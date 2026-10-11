//! Mount a Windows 11 ISO and start Setup.
//!
//! Supported PCs launch `setup.exe` from the root of the image.
//! Unsupported PCs launch `sources\setupprep.exe /Product Server`. That switch
//! is the Flyby11 / Flyoobe compatibility path: Setup uses the server product
//! checks, which do not block on TPM 2.0, Secure Boot, or the CPU generation.
//! The installed edition is still the Windows 11 client edition on the ISO.
//! The ISO stays mounted after Setup starts, because Setup reads it from that drive.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::assess::cpu_features;
use super::ps;
use super::Windows11UpgradeRoute;

/// Windows 11 starts at build 22000. 24H2, which requires POPCNT, starts at 26100.
pub(super) const MIN_WINDOWS_11_BUILD: u32 = 22_000;
pub(super) const POPCNT_REQUIRED_BUILD: u32 = 26_100;
const MOUNT_TIMEOUT: Duration = Duration::from_secs(120);

/// What was launched, so the page can tell the technician where to continue.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11UpgradeStart {
    pub drive: String,
    pub media_build: u32,
    pub setup_path: String,
    pub arguments: String,
}

struct SetupPlan {
    relative: &'static str,
    arguments: &'static str,
}

fn setup_plan(route: Windows11UpgradeRoute, advanced: bool) -> SetupPlan {
    match route {
        Windows11UpgradeRoute::Supported => SetupPlan {
            relative: "setup.exe",
            arguments: "",
        },
        Windows11UpgradeRoute::Unsupported if advanced => SetupPlan {
            relative: r"sources\setupprep.exe",
            arguments: "/Product Server /Compat IgnoreWarning /MigrateDrivers All",
        },
        Windows11UpgradeRoute::Unsupported => SetupPlan {
            relative: r"sources\setupprep.exe",
            arguments: "/Product Server",
        },
    }
}

pub(super) fn is_windows10_filename(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("win10") || name.contains("windows 10") || name.contains("windows10")
}

/// `build == 0` means the ISO's version could not be read. Current media is
/// 24H2 or newer, so a missing version still refuses a CPU without POPCNT.
pub(super) fn media_block_reason(
    build: u32,
    has_popcnt: Option<bool>,
    has_sse42: Option<bool>,
) -> Option<String> {
    if build > 0 && build < MIN_WINDOWS_11_BUILD {
        return Some(format!(
            "This ISO reports build {build}. Choose a Windows 11 ISO."
        ));
    }
    let needs_popcnt = build == 0 || build >= POPCNT_REQUIRED_BUILD;
    if needs_popcnt && (has_popcnt == Some(false) || has_sse42 == Some(false)) {
        return Some(
            "This PC's processor does not support POPCNT and SSE4.2. Windows 11 24H2 and newer will not install, and the compatibility path cannot bypass that.".into(),
        );
    }
    None
}

pub(super) fn start_sync(
    iso_path: &str,
    route: Windows11UpgradeRoute,
    advanced: bool,
) -> Result<Windows11UpgradeStart, String> {
    #[cfg(not(windows))]
    {
        let _ = (iso_path, route, advanced);
        return Err("Windows 11 upgrade is only available on Windows.".into());
    }
    #[cfg(windows)]
    {
        start_windows(iso_path, route, advanced)
    }
}

#[cfg(windows)]
fn start_windows(
    iso_path: &str,
    route: Windows11UpgradeRoute,
    advanced: bool,
) -> Result<Windows11UpgradeStart, String> {
    if matches!(
        super::assess::native_machine(),
        super::assess::MachineKind::X86
    ) {
        return Err("Windows 11 installation media is 64-bit only. This PC is 32-bit.".into());
    }
    let path = validate_iso(iso_path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if is_windows10_filename(file_name) {
        return Err("This file looks like a Windows 10 ISO. Choose a Windows 11 ISO.".into());
    }

    let quoted = ps::quote(&path.to_string_lossy())?;
    let script = format!(
        r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'Stop'
$image = $null
try {{
  $image = Mount-DiskImage -ImagePath {quoted} -PassThru
  $letter = @(Get-Volume -DiskImage $image | Where-Object {{ $_.DriveLetter }} | Select-Object -First 1 -ExpandProperty DriveLetter)
  if (-not $letter) {{ throw 'Windows mounted the ISO without assigning a drive letter.' }}
  $root = "$($letter):\"
  $setup = Join-Path $root 'setup.exe'
  $prep = Join-Path $root 'sources\setupprep.exe'
  $build = 0
  if (Test-Path -LiteralPath $setup) {{
    $build = [int](Get-Item -LiteralPath $setup).VersionInfo.FileBuildPart
  }}
  @{{ ok = $true; drive = [string]$letter; build = $build; hasSetup = [bool](Test-Path -LiteralPath $setup); hasPrep = [bool](Test-Path -LiteralPath $prep) }} | ConvertTo-Json -Compress
}} catch {{
  if ($null -ne $image) {{
    Dismount-DiskImage -ImagePath {quoted} -ErrorAction SilentlyContinue | Out-Null
  }}
  @{{ ok = $false; error = $_.Exception.Message }} | ConvertTo-Json -Compress
}}
"#
    );
    // Any failure below, including a killed mount script, dismounts. Setup
    // keeps the image only after launch succeeds.
    let mut image = MountedIso {
        path: &path,
        keep: false,
    };
    let stdout = ps::run(&script, MOUNT_TIMEOUT)?;
    let mounted: MountReport = serde_json::from_str(ps::json_object(&stdout))
        .map_err(|_| "Windows did not report the mounted ISO.".to_string())?;
    if !mounted.ok {
        return Err(mounted
            .error
            .unwrap_or_else(|| "The ISO could not be mounted.".into()));
    }
    let drive = mounted
        .drive
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .ok_or_else(|| "Windows mounted the ISO without assigning a drive letter.".to_string())?;
    let root = PathBuf::from(format!("{drive}:\\"));
    if !mounted.has_setup || !mounted.has_prep {
        return Err(
            "The mounted image does not contain Windows Setup. Choose a Windows 11 ISO.".into(),
        );
    }

    let (has_popcnt, has_sse42) = cpu_features();
    if let Some(reason) = media_block_reason(mounted.build, has_popcnt, has_sse42) {
        return Err(reason);
    }

    let plan = setup_plan(route, advanced);
    let setup_path = root.join(plan.relative);
    if !setup_path.is_file() {
        return Err("Windows Setup was not found on the mounted ISO.".into());
    }
    launch_elevated(&setup_path, plan.arguments)?;
    image.keep = true;

    log::info!(
        "Windows 11 {} setup started from media build {} on {}:",
        match route {
            Windows11UpgradeRoute::Supported => "standard",
            Windows11UpgradeRoute::Unsupported => "compatibility",
        },
        mounted.build,
        drive
    );
    Ok(Windows11UpgradeStart {
        drive: format!("{drive}:"),
        media_build: mounted.build,
        setup_path: setup_path.to_string_lossy().to_string(),
        arguments: plan.arguments.to_string(),
    })
}

#[cfg(windows)]
fn validate_iso(iso_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(iso_path.trim());
    if !path.is_absolute() {
        return Err("Choose the full path of a Windows 11 ISO.".into());
    }
    if !path.is_file() {
        return Err("That ISO does not exist.".into());
    }
    let name = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if !name.eq_ignore_ascii_case("iso") {
        return Err("Choose a file that ends in .iso.".into());
    }
    Ok(path)
}

#[cfg(windows)]
struct MountedIso<'a> {
    path: &'a Path,
    keep: bool,
}

#[cfg(windows)]
impl Drop for MountedIso<'_> {
    fn drop(&mut self) {
        if !self.keep {
            dismount(self.path);
        }
    }
}

#[cfg(windows)]
fn dismount(iso: &Path) {
    let Ok(quoted) = ps::quote(&iso.to_string_lossy()) else {
        return;
    };
    let script = format!(
        "$ProgressPreference = 'SilentlyContinue'; Dismount-DiskImage -ImagePath {quoted} | Out-Null"
    );
    let _ = ps::run(&script, Duration::from_secs(30));
}

#[cfg(windows)]
fn launch_elevated(exe: &Path, args: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_SHOWNORMAL;

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let file = wide(exe.as_os_str());
    let verb = wide(std::ffi::OsStr::new("runas"));
    let params = wide(std::ffi::OsStr::new(args));
    let directory = exe.parent().map(|dir| wide(dir.as_os_str()));
    // ShellExecute returns a value greater than 32 when the process starts.
    // 5 is the code Windows uses when the administrator prompt is declined.
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            if args.is_empty() {
                std::ptr::null()
            } else {
                params.as_ptr()
            },
            directory
                .as_ref()
                .map(|dir| dir.as_ptr())
                .unwrap_or(std::ptr::null()),
            SW_SHOWNORMAL,
        ) as isize
    };
    if code > 32 {
        Ok(())
    } else if code == 5 {
        Err("Setup did not start. Approve the administrator prompt and try again.".into())
    } else {
        Err(format!("Windows could not start Setup (code {code})."))
    }
}

#[cfg(windows)]
#[derive(Debug, serde::Deserialize)]
struct MountReport {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    drive: String,
    #[serde(default)]
    build: u32,
    #[serde(default, rename = "hasSetup")]
    has_setup: bool,
    #[serde(default, rename = "hasPrep")]
    has_prep: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_route_uses_normal_setup() {
        let plan = setup_plan(Windows11UpgradeRoute::Supported, true);
        assert_eq!(plan.relative, "setup.exe");
        assert!(plan.arguments.is_empty());
    }

    #[test]
    fn unsupported_route_uses_the_server_product_switch() {
        let plan = setup_plan(Windows11UpgradeRoute::Unsupported, false);
        assert_eq!(plan.relative, r"sources\setupprep.exe");
        assert_eq!(plan.arguments, "/Product Server");
    }

    #[test]
    fn advanced_mode_adds_the_flyby11_compatibility_flags() {
        let plan = setup_plan(Windows11UpgradeRoute::Unsupported, true);
        assert!(plan.arguments.contains("/Product Server"));
        assert!(plan.arguments.contains("/Compat IgnoreWarning"));
        assert!(plan.arguments.contains("/MigrateDrivers All"));
    }

    #[test]
    fn windows_10_names_are_rejected() {
        assert!(is_windows10_filename("Win10_22H2_English_x64.iso"));
        assert!(is_windows10_filename("Windows 10.iso"));
        assert!(!is_windows10_filename("Windows11_Client_x64_en-us.iso"));
    }

    #[test]
    fn old_media_and_missing_popcnt_are_blocked() {
        assert!(media_block_reason(19041, Some(true), Some(true))
            .unwrap()
            .contains("Windows 11"));
        assert!(media_block_reason(22631, Some(false), Some(false)).is_none());
        assert!(media_block_reason(26100, Some(false), Some(true)).is_some());
        assert!(media_block_reason(0, Some(false), Some(true)).is_some());
        assert!(media_block_reason(26300, Some(true), Some(true)).is_none());
    }
}
