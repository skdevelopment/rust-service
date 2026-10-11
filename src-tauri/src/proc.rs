//! Child-process helpers
//!
//! Every non-interactive external program goes through [`command`] so the
//! windowless release build never flashes a console window, and quick queries
//! use [`output_with_timeout`] so a hung tool cannot block its caller forever.
//! Interactive launches (scripts, shortcuts, programs the technician opens)
//! deliberately keep using `std::process::Command` so they get a window.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Default timeout for short system queries (registry, WMI, netsh, ...).
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A `Command` for `program` that will not open a console window.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Like [`Command::output`], but kills the child if it runs past `timeout`.
pub fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Drain both pipes concurrently so a chatty child cannot block on a full pipe.
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("process timed out after {}s", timeout.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let join = |h: Option<std::thread::JoinHandle<Vec<u8>>>| {
        h.and_then(|h| h.join().ok()).unwrap_or_default()
    };
    Ok(Output {
        status,
        stdout: join(stdout),
        stderr: join(stderr),
    })
}

/// Run blocking work (process calls, file scans) from an async command without
/// tying up an async runtime worker. Panics propagate to the caller.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(f).await {
        Ok(value) => value,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    }
}

fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

/// Decode console output from a Windows tool.
///
/// Handles UTF-8, UTF-16LE (sfc, dism, some PowerShell hosts) and the console's
/// OEM code page, which is what most built-in tools use on non-English systems.
pub fn decode_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if looks_like_utf16le(bytes) {
        let start = if bytes.starts_with(&[0xFF, 0xFE]) {
            2
        } else {
            0
        };
        let units: Vec<u16> = bytes[start..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units).replace('\0', "");
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    decode_oem(bytes)
}

fn looks_like_utf16le(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return true;
    }
    if bytes.len() < 4 {
        return false;
    }
    // ASCII text encoded as UTF-16LE has a NUL in (almost) every odd byte.
    let sample = &bytes[..bytes.len().min(512)];
    let odd_nuls = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|&&b| b == 0)
        .count();
    odd_nuls * 2 >= sample.len() / 2
}

#[cfg(windows)]
fn decode_oem(bytes: &[u8]) -> String {
    use winapi::um::stringapiset::MultiByteToWideChar;
    use winapi::um::winnls::CP_OEMCP;

    let Ok(len) = i32::try_from(bytes.len()) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    // SAFETY: both buffers are valid for the lengths passed; the first call only
    // measures, the second writes at most `needed` UTF-16 units into `wide`.
    unsafe {
        let needed = MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr() as *const i8,
            len,
            std::ptr::null_mut(),
            0,
        );
        if needed <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; needed as usize];
        let written = MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr() as *const i8,
            len,
            wide.as_mut_ptr(),
            needed,
        );
        if written <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        String::from_utf16_lossy(&wide[..written as usize])
    }
}

#[cfg(not(windows))]
fn decode_oem(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8() {
        assert_eq!(decode_output("héllo".as_bytes()), "héllo");
    }

    #[test]
    fn decodes_utf16le_with_and_without_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("Scan done".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode_output(&bytes), "Scan done");
        assert_eq!(decode_output(&bytes[2..]), "Scan done");
    }

    #[test]
    fn timeout_kills_child() {
        #[cfg(windows)]
        let mut cmd = {
            let mut c = command("powershell");
            c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = command("sleep");
            c.arg("30");
            c
        };
        let started = Instant::now();
        let err = output_with_timeout(&mut cmd, Duration::from_millis(500)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
