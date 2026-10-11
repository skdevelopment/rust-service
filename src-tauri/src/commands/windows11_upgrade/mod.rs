//! Windows 11 in-place upgrade.
//!
//! The technician picks a supported or unsupported route. Both routes use a
//! Windows 11 ISO, either downloaded from Microsoft's consumer servers or
//! chosen from disk. Supported hardware starts `setup.exe`. Unsupported
//! hardware starts `sources\setupprep.exe /Product Server`, the compatibility
//! path used by Flyoobe / Flyby11. Setup's window does the actual install.

mod assess;
mod catalog;
mod ps;
mod setup;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::pin;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::lock::lock_mutex;
use crate::proc;

pub use assess::Windows11UpgradeAssessment;
pub use catalog::Windows11IsoCatalog;
pub use setup::Windows11UpgradeStart;

const PROGRESS_EVENT: &str = "windows11-iso-progress";
const MIN_ISO_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ISO_BYTES: u64 = 15 * 1024 * 1024 * 1024;
const SPACE_HEADROOM: u64 = 100 * 1024 * 1024;
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static RECEIVED: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static FINISHED_PATH: Mutex<Option<String>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Supported uses normal Setup. Unsupported uses the server-product compatibility path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Windows11UpgradeRoute {
    /// `setup.exe` with Windows' own hardware checks.
    Supported,
    /// `sources\setupprep.exe /Product Server` for PCs that fail those checks.
    Unsupported,
}

/// A Windows 11 ISO saved on this PC.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11IsoFile {
    pub path: String,
    pub bytes: u64,
    pub file_name: String,
}

/// Bytes received while an ISO is downloading.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11DownloadProgress {
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

/// Download state, including a download that finished while the page was closed.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11DownloadStatus {
    pub running: bool,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub finished_path: Option<String>,
    pub error: Option<String>,
}

/// Read the PC and suggest the supported or unsupported route.
#[tauri::command]
#[specta::specta]
pub async fn assess_windows11_upgrade() -> Result<Windows11UpgradeAssessment, String> {
    proc::blocking(assess::assess_sync).await
}

/// Languages for the current Windows 11 ISO on Microsoft's servers.
#[tauri::command]
#[specta::specta]
pub async fn list_windows11_iso_languages() -> Result<Windows11IsoCatalog, String> {
    catalog::list_languages().await
}

/// Download that language to `dest_path`. Progress is emitted on `windows11-iso-progress`.
#[tauri::command]
#[specta::specta]
pub async fn download_windows11_iso(
    app: AppHandle,
    language: String,
    dest_path: String,
) -> Result<Windows11IsoFile, String> {
    let guard = DownloadGuard::try_begin()?;
    let result = download_inner(&app, &language, &dest_path).await;
    match &result {
        Ok(file) => guard.finish_ok(&file.path),
        Err(error) => guard.finish_err(error),
    }
    result
}

/// Stop the ISO download. The partial file is deleted.
#[tauri::command(async)]
#[specta::specta]
pub fn cancel_windows11_iso_download() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Current or most recent ISO download.
#[tauri::command(async)]
#[specta::specta]
pub fn windows11_iso_download_status() -> Windows11DownloadStatus {
    let total = TOTAL.load(Ordering::Relaxed);
    Windows11DownloadStatus {
        running: RUNNING.load(Ordering::SeqCst),
        received_bytes: RECEIVED.load(Ordering::Relaxed),
        total_bytes: if total == 0 { None } else { Some(total) },
        finished_path: lock_mutex(&FINISHED_PATH).clone(),
        error: lock_mutex(&LAST_ERROR).clone(),
    }
}

/// Mount `iso_path` and start Windows Setup for the chosen route.
#[tauri::command]
#[specta::specta]
pub async fn start_windows11_upgrade(
    iso_path: String,
    route: Windows11UpgradeRoute,
    advanced: bool,
) -> Result<Windows11UpgradeStart, String> {
    proc::blocking(move || setup::start_sync(&iso_path, route, advanced)).await
}

struct DownloadGuard;

impl DownloadGuard {
    fn try_begin() -> Result<Self, String> {
        if RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A Windows 11 ISO download is already running.".into());
        }
        CANCEL.store(false, Ordering::SeqCst);
        RECEIVED.store(0, Ordering::Relaxed);
        TOTAL.store(0, Ordering::Relaxed);
        *lock_mutex(&FINISHED_PATH) = None;
        *lock_mutex(&LAST_ERROR) = None;
        Ok(Self)
    }

    fn finish_ok(&self, path: &str) {
        *lock_mutex(&FINISHED_PATH) = Some(path.to_string());
        *lock_mutex(&LAST_ERROR) = None;
    }

    fn finish_err(&self, error: &str) {
        *lock_mutex(&FINISHED_PATH) = None;
        *lock_mutex(&LAST_ERROR) = Some(error.to_string());
    }
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

async fn download_inner(
    app: &AppHandle,
    language: &str,
    dest_path: &str,
) -> Result<Windows11IsoFile, String> {
    let language = language.trim();
    if language.is_empty() {
        return Err("Choose a language first.".into());
    }
    let dest = PathBuf::from(dest_path.trim());
    validate_destination(&dest)?;
    not_canceled()?;

    // The handshake has its own request timeout. This only lets Cancel win
    // while that request is still in flight.
    let resolved = poll_cancel(catalog::resolve_iso(language), None).await??;
    log::info!("Windows 11 ISO download starting ({})", resolved.file_name);
    not_canceled()?;
    let url = reqwest::Url::parse(&resolved.url)
        .map_err(|_| "Microsoft returned a download link that is not a URL.".to_string())?;

    let client = reqwest::Client::builder()
        .user_agent(catalog::BROWSER_USER_AGENT)
        .connect_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Could not start the download: {e}"))?;
    let mut response = poll_cancel(
        client
            .get(url)
            .header(reqwest::header::REFERER, catalog::REFERER)
            .send(),
        Some(STALL_TIMEOUT),
    )
    .await?
    .map_err(|e| {
        format!(
            "Could not download the ISO: {}",
            catalog::redact_urls(&e.to_string())
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Microsoft returned HTTP {status} for the ISO."));
    }

    let total = response.content_length();
    if let Some(total) = total {
        if !(MIN_ISO_BYTES..=MAX_ISO_BYTES).contains(&total) {
            return Err(format!(
                "Microsoft offered a {} file. A Windows 11 ISO is several GB.",
                format_bytes(total)
            ));
        }
        if let Some(parent) = dest.parent() {
            if let Some(free) = assess::volume_free_bytes(parent) {
                if free < total.saturating_add(SPACE_HEADROOM) {
                    return Err(format!(
                        "The destination drive has {} free. This ISO needs {}.",
                        format_bytes(free),
                        format_bytes(total)
                    ));
                }
            }
        }
        TOTAL.store(total, Ordering::Relaxed);
    }

    let mut partial_path = dest.clone().into_os_string();
    partial_path.push(".partial");
    let partial = PartialFile::create(PathBuf::from(partial_path))?;
    let file = tokio::fs::File::create(&partial.path)
        .await
        .map_err(|e| format!("Could not create the ISO file: {e}"))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    let mut received: u64 = 0;
    let mut saw_body = false;
    let mut last_emit = Instant::now();
    let mut last_byte = Instant::now();

    loop {
        // Keep the same read future across cancel/stall polls. Dropping it on
        // every short timeout would throw away an in-flight chunk.
        let next_chunk = response.chunk();
        pin!(next_chunk);
        let chunk = loop {
            tokio::select! {
                biased;
                chunk = &mut next_chunk => break chunk,
                _ = tokio::time::sleep(Duration::from_millis(400)) => {
                    not_canceled()?;
                    if last_byte.elapsed() >= STALL_TIMEOUT {
                        return Err(
                            "The download stalled. Check the network and try again.".into(),
                        );
                    }
                }
            }
        };
        let chunk = match chunk {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                return Err(format!(
                    "The download stopped: {}",
                    catalog::redact_urls(&error.to_string())
                ));
            }
        };
        // Chunks arrive faster than 400ms on a healthy link, so the sleep
        // branch above never runs and would never notice Cancel.
        not_canceled()?;
        if chunk.is_empty() {
            continue;
        }
        if !saw_body {
            saw_body = true;
            if looks_like_error_page(&chunk) {
                return Err("Microsoft returned a page instead of an ISO.".into());
            }
        }
        writer
            .write_all(&chunk)
            .await
            .map_err(|e| format!("Could not write the ISO: {e}"))?;
        received = received.saturating_add(chunk.len() as u64);
        if let Some(total) = total {
            if received > total {
                return Err("The download was larger than Microsoft announced.".into());
            }
        } else if received > MAX_ISO_BYTES {
            return Err("The download is larger than a Windows 11 ISO.".into());
        }
        RECEIVED.store(received, Ordering::Relaxed);
        last_byte = Instant::now();
        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            emit_progress(app, received, total);
            last_emit = Instant::now();
        }
    }

    if let Some(total) = total {
        if received != total {
            return Err("The download ended before the ISO was complete.".into());
        }
    }
    if received < MIN_ISO_BYTES {
        return Err(format!(
            "The download is only {}. A Windows 11 ISO is several GB.",
            format_bytes(received)
        ));
    }

    writer
        .flush()
        .await
        .map_err(|e| format!("Could not finish writing the ISO: {e}"))?;
    let file = writer.into_inner();
    file.sync_all()
        .await
        .map_err(|e| format!("Could not finish writing the ISO: {e}"))?;
    drop(file);

    if !is_iso9660(&partial.path) {
        return Err("The downloaded file is not a CD or DVD image.".into());
    }
    commit_partial(&partial, &dest)?;
    emit_progress(app, received, total.or(Some(received)));

    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Windows11.iso")
        .to_string();
    log::info!("Windows 11 ISO saved ({})", format_bytes(received));
    Ok(Windows11IsoFile {
        path: dest.to_string_lossy().to_string(),
        bytes: received,
        file_name,
    })
}

fn emit_progress(app: &AppHandle, received: u64, total: Option<u64>) {
    let _ = app.emit(
        PROGRESS_EVENT,
        Windows11DownloadProgress {
            received_bytes: received,
            total_bytes: total,
        },
    );
}

fn not_canceled() -> Result<(), String> {
    if CANCEL.load(Ordering::SeqCst) {
        Err("Download canceled.".into())
    } else {
        Ok(())
    }
}

/// Poll `fut` without dropping it on the 400ms tick. `stall_after` limits how
/// long the future may stay unfinished. Cancel still applies either way.
async fn poll_cancel<T>(
    fut: impl Future<Output = T>,
    stall_after: Option<Duration>,
) -> Result<T, String> {
    pin!(fut);
    let started = Instant::now();
    loop {
        tokio::select! {
            biased;
            result = &mut fut => return Ok(result),
            _ = tokio::time::sleep(Duration::from_millis(400)) => {
                not_canceled()?;
                if stall_after.is_some_and(|limit| started.elapsed() >= limit) {
                    return Err(
                        "The download stalled. Check the network and try again.".into(),
                    );
                }
            }
        }
    }
}

fn validate_destination(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Choose a full path for the ISO.".into());
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err("The file name needs to end in .iso.".into());
    };
    if name.len() > 180 || !name.to_ascii_lowercase().ends_with(".iso") {
        return Err("The file name needs to end in .iso.".into());
    }
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Err("Choose a folder for the ISO.".into());
    };
    if !parent.is_dir() {
        return Err("The destination folder does not exist.".into());
    }
    Ok(())
}

struct PartialFile {
    path: PathBuf,
    /// When set, drop deletes the partial file. Cleared once the bytes are a
    /// finished ISO we must not throw away, including when the final rename fails.
    armed: AtomicBool,
}

impl PartialFile {
    fn create(path: PathBuf) -> Result<Self, String> {
        Ok(Self {
            path,
            armed: AtomicBool::new(true),
        })
    }

    fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        if self.armed.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn commit_partial(partial: &PartialFile, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        std::fs::remove_file(dest)
            .map_err(|e| format!("Could not replace the existing ISO: {e}"))?;
    }
    if let Err(error) = std::fs::rename(&partial.path, dest) {
        partial.disarm();
        return Err(format!(
            "The ISO downloaded but could not be moved to {}. It is still at {}. ({error})",
            dest.display(),
            partial.path.display()
        ));
    }
    partial.disarm();
    Ok(())
}

fn looks_like_error_page(chunk: &[u8]) -> bool {
    let start = chunk
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(0);
    let body = &chunk[start..];
    body.starts_with(b"<") || body.starts_with(b"{")
}

pub(super) fn is_iso9660(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    if file.seek(SeekFrom::Start(0x8001)).is_err() {
        return false;
    }
    let mut magic = [0u8; 5];
    file.read_exact(&mut magic).is_ok() && magic == *b"CD001"
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_pages_are_not_treated_as_iso_bytes() {
        assert!(looks_like_error_page(b"\n<html>"));
        assert!(looks_like_error_page(b"{\"error\":true}"));
        assert!(!looks_like_error_page(&[0, 0, 1, 2, 3, 4]));
    }

    #[test]
    fn iso9660_magic_is_read_at_the_volume_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.iso");
        let mut file = std::fs::File::create(&path).unwrap();
        use std::io::Write;
        file.write_all(&vec![0u8; 0x8001]).unwrap();
        file.write_all(b"CD001").unwrap();
        assert!(is_iso9660(&path));
        assert!(!is_iso9660(dir.path()));
    }
}
