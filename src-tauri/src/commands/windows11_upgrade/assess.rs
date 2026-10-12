//! Read-only check of the PC that is about to be upgraded.
//!
//! The numbers match Flyoobe's `Data/Upgrade.ini`: 4 GB RAM, 25 GB free on the
//! Windows drive, 64-bit, TPM 2.0, Secure Boot. An unknown TPM or Secure Boot
//! result stays neutral. Windows Setup still makes the final call.

use serde::{Deserialize, Serialize};

use super::Windows11UpgradeRoute;

pub(super) const MINIMUM_RAM_GB: f64 = 4.0;
pub(super) const RECOMMENDED_FREE_SPACE_GB: f64 = 25.0;
const BYTES_PER_GB: f64 = 1_073_741_824.0;

/// What the upgrade page shows before the technician picks a route.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11UpgradeAssessment {
    pub windows_name: String,
    pub edition: String,
    pub display_version: String,
    pub build: u32,
    /// Update Build Revision from the registry, when Windows has one.
    pub update_build: Option<u32>,
    pub is_windows10: bool,
    pub is_windows11: bool,
    pub is_64_bit: bool,
    pub architecture: String,
    pub memory_gb: f64,
    pub free_space_gb: Option<f64>,
    pub has_tpm20: Option<bool>,
    pub secure_boot_enabled: Option<bool>,
    pub processor_name: String,
    /// `None` on ARM, where these x86 instructions do not apply.
    pub has_popcnt: Option<bool>,
    pub has_sse42: Option<bool>,
    pub minimum_ram_gb: f64,
    pub recommended_free_space_gb: f64,
    pub suggested_route: Windows11UpgradeRoute,
    /// Sentences from the same check that picked `suggested_route`. Empty when
    /// that route is supported.
    pub suggestion_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteSuggestion {
    pub route: Windows11UpgradeRoute,
    pub reasons: Vec<String>,
}

pub(super) fn suggested_route(
    is_64_bit: bool,
    memory_gb: f64,
    has_tpm20: Option<bool>,
    secure_boot_enabled: Option<bool>,
) -> RouteSuggestion {
    let mut reasons = Vec::new();
    if !is_64_bit {
        reasons.push("This PC is 32-bit.".to_string());
    }
    if memory_gb > 0.0 && memory_gb < MINIMUM_RAM_GB {
        reasons.push(format!("Memory is {memory_gb:.1} GB."));
    }
    if has_tpm20 == Some(false) {
        reasons.push("TPM 2.0 was not found.".to_string());
    }
    if secure_boot_enabled == Some(false) {
        reasons.push("Secure Boot is off.".to_string());
    }
    let route = if reasons.is_empty() {
        Windows11UpgradeRoute::Supported
    } else {
        Windows11UpgradeRoute::Unsupported
    };
    RouteSuggestion { route, reasons }
}

pub(super) fn edition_label(product: &str, windows11: bool) -> String {
    let product = product.trim();
    if windows11 {
        if let Some(rest) = product.strip_prefix("Windows 10") {
            return format!("Windows 11{rest}");
        }
    }
    if product.is_empty() {
        if windows11 {
            "Windows 11".to_string()
        } else {
            "Windows".to_string()
        }
    } else {
        product.to_string()
    }
}

/// `spec` is the TPM `SpecVersion` string, for example `2.0, 0, 1.16`.
pub(super) fn spec_is_tpm20(spec: &str) -> bool {
    spec.split(',').any(|part| part.trim().starts_with("2.0"))
}

pub(super) fn assess_sync() -> Result<Windows11UpgradeAssessment, String> {
    #[cfg(windows)]
    {
        assess_windows()
    }
    #[cfg(not(windows))]
    {
        Err("Windows 11 upgrade is only available on Windows.".into())
    }
}

#[cfg(windows)]
fn assess_windows() -> Result<Windows11UpgradeAssessment, String> {
    let build = read_sz(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuild",
    )
    .and_then(|value| value.parse().ok())
    .unwrap_or(0);
    let update_build = read_dword(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "UBR");
    let display_version = read_sz(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "DisplayVersion",
    )
    .unwrap_or_default();
    let product = read_sz(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "ProductName",
    )
    .unwrap_or_default();
    let is_windows11 = build >= 22_000;
    let is_windows10 = (10_240..22_000).contains(&build);
    let windows_name = if is_windows11 {
        "Windows 11"
    } else if is_windows10 {
        "Windows 10"
    } else {
        "Windows"
    };

    let arch = native_machine();
    let is_64_bit = !matches!(arch, MachineKind::X86);
    let memory_gb = memory_gb();
    let free_space_gb = system_free_gb();
    let has_tpm20 = read_tpm20();
    let secure_boot_enabled = read_secure_boot();
    let (has_popcnt, has_sse42) = cpu_features();
    let suggestion = suggested_route(is_64_bit, memory_gb, has_tpm20, secure_boot_enabled);

    Ok(Windows11UpgradeAssessment {
        windows_name: windows_name.to_string(),
        edition: edition_label(&product, is_windows11),
        display_version,
        build,
        update_build,
        is_windows10,
        is_windows11,
        is_64_bit,
        architecture: arch.label().to_string(),
        memory_gb,
        free_space_gb,
        has_tpm20,
        secure_boot_enabled,
        processor_name: read_sz(
            r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            "ProcessorNameString",
        )
        .unwrap_or_default(),
        has_popcnt,
        has_sse42,
        minimum_ram_gb: MINIMUM_RAM_GB,
        recommended_free_space_gb: RECOMMENDED_FREE_SPACE_GB,
        suggested_route: suggestion.route,
        suggestion_reasons: suggestion.reasons,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MachineKind {
    X64,
    Arm64,
    X86,
}

impl MachineKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "ARM64",
            Self::X86 => "32-bit",
        }
    }
}

#[cfg(windows)]
pub(super) fn native_machine() -> MachineKind {
    // IsWow64Process2 reports the native machine even when this process is
    // running under emulation (x64 on ARM64). It has shipped with Windows 10.
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn IsWow64Process2(
            process: isize,
            process_machine: *mut u16,
            native_machine: *mut u16,
        ) -> i32;
    }
    const AMD64: u16 = 0x8664;
    const ARM64: u16 = 0xAA64;
    const I386: u16 = 0x014c;

    let mut process_machine = 0u16;
    let mut native = 0u16;
    let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut process_machine, &mut native) };
    if ok != 0 {
        return match native {
            ARM64 => MachineKind::Arm64,
            I386 => MachineKind::X86,
            AMD64 => MachineKind::X64,
            _ => MachineKind::X64,
        };
    }
    match std::env::consts::ARCH {
        "aarch64" => MachineKind::Arm64,
        "x86" => MachineKind::X86,
        _ => MachineKind::X64,
    }
}

#[cfg(not(windows))]
pub(super) fn native_machine() -> MachineKind {
    MachineKind::X64
}

#[cfg(windows)]
fn memory_gb() -> f64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() as f64 / BYTES_PER_GB
}

#[cfg(windows)]
fn system_free_gb() -> Option<f64> {
    let root = std::env::var("SystemRoot").ok()?;
    Some(volume_free_bytes(std::path::Path::new(&root))? as f64 / BYTES_PER_GB)
}

/// `canonicalize` on Windows returns `\\?\C:\...`. Disk mounts are `C:\`.
fn windows_path_key(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw.into_owned()
    };
    stripped.trim_end_matches(['\\', '/']).to_ascii_uppercase()
}

/// Free bytes on the volume that holds `path`.
#[cfg(windows)]
pub(super) fn volume_free_bytes(path: &std::path::Path) -> Option<u64> {
    let anchor = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.parent()
            .filter(|parent| parent.exists())
            .and_then(|parent| parent.canonicalize().ok())
            .unwrap_or_else(|| path.to_path_buf())
    };
    let anchor_text = windows_path_key(&anchor);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for disk in disks.list() {
        let mount = windows_path_key(disk.mount_point());
        if mount.is_empty() || !anchor_text.starts_with(&mount) {
            continue;
        }
        if best.as_ref().is_none_or(|(len, _)| mount.len() > *len) {
            best = Some((mount.len(), disk.available_space()));
        }
    }
    best.map(|(_, bytes)| bytes)
}

#[cfg(not(windows))]
pub(super) fn volume_free_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

#[cfg(windows)]
fn read_tpm20() -> Option<bool> {
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
$tpm = Get-CimInstance -Namespace 'root/CIMV2/Security/MicrosoftTpm' -ClassName Win32_Tpm
if ($null -eq $tpm) {
  @{ status = 'absent'; spec = '' } | ConvertTo-Json -Compress
} else {
  @{ status = 'ok'; spec = [string]$tpm.SpecVersion } | ConvertTo-Json -Compress
}
"#;
    let stdout = super::ps::run(script, crate::proc::QUERY_TIMEOUT).ok()?;
    let parsed: TpmReport = serde_json::from_str(super::ps::json_object(&stdout)).ok()?;
    match parsed.status.as_str() {
        "ok" => Some(spec_is_tpm20(&parsed.spec)),
        "absent" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct TpmReport {
    status: String,
    #[serde(default)]
    spec: String,
}

#[cfg(windows)]
fn read_secure_boot() -> Option<bool> {
    let value = read_dword(
        r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
        "UEFISecureBootEnabled",
    )?;
    Some(value == 1)
}

/// POPCNT is CPUID leaf 1, ECX bit 23. SSE4.2 is ECX bit 20.
/// Windows 11 24H2 refuses to boot without both, and no setup switch skips that.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(super) fn cpu_features() -> (Option<bool>, Option<bool>) {
    #[cfg(target_arch = "x86_64")]
    let result = unsafe { std::arch::x86_64::__cpuid(1) };
    #[cfg(target_arch = "x86")]
    let result = unsafe { std::arch::x86::__cpuid(1) };
    let popcnt = result.ecx & (1 << 23) != 0;
    let sse42 = result.ecx & (1 << 20) != 0;
    (Some(popcnt), Some(sse42))
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub(super) fn cpu_features() -> (Option<bool>, Option<bool>) {
    (None, None)
}

#[cfg(windows)]
fn read_sz(subkey: &str, value: &str) -> Option<String> {
    with_key(subkey, |key| {
        let name = wide(value);
        let mut size = 0u32;
        let mut kind = 0u32;
        let status = unsafe {
            winapi::um::winreg::RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        // A size query returns ERROR_SUCCESS, or ERROR_MORE_DATA is not used
        // when the data pointer is null: Windows returns success and the size.
        if status != 0 || kind != winapi::um::winnt::REG_SZ || size < 2 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize / 2) + 1];
        let status = unsafe {
            winapi::um::winreg::RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                buf.as_mut_ptr() as *mut u8,
                &mut size,
            )
        };
        if status != 0 || kind != winapi::um::winnt::REG_SZ {
            return None;
        }
        let chars = (size as usize / 2).saturating_sub(1);
        let text = String::from_utf16_lossy(&buf[..chars.min(buf.len())]);
        let text = text.trim().trim_matches('\0').trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

#[cfg(windows)]
fn read_dword(subkey: &str, value: &str) -> Option<u32> {
    with_key(subkey, |key| {
        let name = wide(value);
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = 0u32;
        let status = unsafe {
            winapi::um::winreg::RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                &mut data as *mut u32 as *mut u8,
                &mut size,
            )
        };
        if status != 0 || kind != winapi::um::winnt::REG_DWORD {
            None
        } else {
            Some(data)
        }
    })
}

#[cfg(windows)]
fn with_key<T>(
    subkey: &str,
    f: impl FnOnce(winapi::shared::minwindef::HKEY) -> Option<T>,
) -> Option<T> {
    let name = wide(subkey);
    let mut key = std::ptr::null_mut();
    let access = winapi::um::winnt::KEY_READ | winapi::um::winnt::KEY_WOW64_64KEY;
    let status = unsafe {
        winapi::um::winreg::RegOpenKeyExW(
            winapi::um::winreg::HKEY_LOCAL_MACHINE,
            name.as_ptr(),
            0,
            access,
            &mut key,
        )
    };
    if status != 0 || key.is_null() {
        return None;
    }
    struct Close(winapi::shared::minwindef::HKEY);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe {
                winapi::um::winreg::RegCloseKey(self.0);
            }
        }
    }
    let _close = Close(key);
    f(key)
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tpm_20_spec_accepts_the_windows_version_string() {
        assert!(spec_is_tpm20("2.0, 0, 1.16"));
        assert!(!spec_is_tpm20("1.2, 2, 3"));
        assert!(!spec_is_tpm20(""));
    }

    #[test]
    fn suggestion_stays_supported_when_firmware_checks_are_unknown() {
        let suggestion = suggested_route(true, 16.0, None, None);
        assert_eq!(suggestion.route, Windows11UpgradeRoute::Supported);
        assert!(suggestion.reasons.is_empty());
    }

    #[test]
    fn suggestion_switches_when_a_known_requirement_is_missing() {
        let tpm = suggested_route(true, 16.0, Some(false), Some(true));
        assert_eq!(tpm.route, Windows11UpgradeRoute::Unsupported);
        assert_eq!(tpm.reasons, vec!["TPM 2.0 was not found.".to_string()]);

        let memory = suggested_route(true, 3.5, Some(true), Some(true));
        assert_eq!(memory.route, Windows11UpgradeRoute::Unsupported);
        assert_eq!(memory.reasons, vec!["Memory is 3.5 GB.".to_string()]);

        let bits = suggested_route(false, 16.0, Some(true), Some(false));
        assert_eq!(bits.route, Windows11UpgradeRoute::Unsupported);
        assert_eq!(
            bits.reasons,
            vec![
                "This PC is 32-bit.".to_string(),
                "Secure Boot is off.".to_string()
            ]
        );
    }

    #[test]
    fn unknown_memory_does_not_count_as_too_little() {
        let suggestion = suggested_route(true, 0.0, Some(true), Some(true));
        assert_eq!(suggestion.route, Windows11UpgradeRoute::Supported);
        assert!(suggestion.reasons.is_empty());
    }

    #[test]
    fn extended_windows_paths_match_drive_mounts() {
        assert_eq!(
            windows_path_key(std::path::Path::new(r"\\?\C:\Windows")),
            r"C:\WINDOWS"
        );
        assert_eq!(windows_path_key(std::path::Path::new(r"C:\")), "C:");
        assert!(windows_path_key(std::path::Path::new(r"\\?\C:\Windows")).starts_with("C:"));
    }

    #[test]
    fn windows_11_product_name_replaces_the_old_windows_10_prefix() {
        assert_eq!(edition_label("Windows 10 Pro", true), "Windows 11 Pro");
        assert_eq!(edition_label("Windows 11 Home", true), "Windows 11 Home");
        assert_eq!(edition_label("", false), "Windows");
    }
}
