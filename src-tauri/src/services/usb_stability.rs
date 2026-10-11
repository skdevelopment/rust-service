//! USB Stability Test Service
//!
//! Non-destructive USB drive testing: sequential read/write benchmarks,
//! data integrity verification, random I/O latency, and fake-capacity detection.
//! Uses temporary files that are cleaned up after the test.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use sysinfo::Disks;

use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, SelectOption, ServiceDefinition, ServiceFinding,
    ServiceOptionSchema, ServiceResult,
};

// =============================================================================
// Constants
// =============================================================================

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunks
const RANDOM_IO_BLOCK: usize = 4096; // 4 KB random I/O blocks
const RANDOM_IO_ITERATIONS: usize = 100;
const PATTERN_BYTE_A: u8 = 0xAA;
const PATTERN_BYTE_B: u8 = 0x55;
const TEMP_DIR_NAME: &str = "_rustservice_usb_test";
const BYTES_PER_MB: f64 = 1_048_576.0;
const BYTES_PER_GB: f64 = 1_073_741_824.0;
/// Free space required on top of the test file (filesystem metadata etc.)
const SPACE_HEADROOM: u64 = 10 * 1024 * 1024;

// =============================================================================
// Service Implementation
// =============================================================================

pub struct UsbStabilityService;

impl Service for UsbStabilityService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "usb-stability".to_string(),
            name: "USB Stability Test".to_string(),
            description:
                "Tests USB drive speed, integrity, and reliability with non-destructive benchmarks"
                    .to_string(),
            category: "diagnostics".to_string(),
            estimated_duration_secs: 120,
            required_programs: vec![],
            options: vec![
                ServiceOptionSchema {
                    id: "target_drive".to_string(),
                    label: "Target USB Drive".to_string(),
                    option_type: OptionType::UsbDrive,
                    default_value: serde_json::json!(""),
                    min: None,
                    max: None,
                    options: None,
                    description: Some(
                        "Select the USB drive to test. Leave empty to auto-detect.".to_string(),
                    ),
                },
                ServiceOptionSchema {
                    id: "test_intensity".to_string(),
                    label: "Test Intensity".to_string(),
                    option_type: OptionType::Select,
                    default_value: serde_json::json!("standard"),
                    min: None,
                    max: None,
                    options: Some(vec![
                        SelectOption {
                            value: "quick".to_string(),
                            label: "Quick (256 MB)".to_string(),
                        },
                        SelectOption {
                            value: "standard".to_string(),
                            label: "Standard (512 MB)".to_string(),
                        },
                        SelectOption {
                            value: "thorough".to_string(),
                            label: "Thorough (1 GB)".to_string(),
                        },
                    ]),
                    description: Some("Amount of data to write for benchmarking".to_string()),
                },
                ServiceOptionSchema {
                    id: "verify_integrity".to_string(),
                    label: "Data Integrity Check".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: serde_json::json!(true),
                    min: None,
                    max: None,
                    options: None,
                    description: Some(
                        "Verify written data byte-by-byte to detect corruption or fake drives"
                            .to_string(),
                    ),
                },
            ],
            icon: "usb".to_string(),
            exclusive_resources: vec!["disk-heavy".to_string()],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        ctx.log("Starting USB Stability Test...");
        let opts = TestOptions::from_json(options);

        // Phase 1: drive detection & validation
        ctx.log("Phase 1: Detecting USB drives...");
        let drive = match select_drive(&opts.target_drive, opts.test_size_bytes) {
            Ok(d) => d,
            Err((title, msg, rec, code)) => {
                ctx.log(format!("ERROR: {}", msg));
                ctx.finding(
                    FindingSeverity::Error,
                    title,
                    msg.clone(),
                    Some(rec.to_string()),
                    Some(json!({ "type": "usb_error", "error": code })),
                );
                return ctx.finish(false, Some(msg));
            }
        };
        ctx.log(format!(
            "Selected drive: {} ({}) — {:.1} GB total, {:.1} GB free, {}",
            drive.mount_point,
            drive.label,
            drive.total_space as f64 / BYTES_PER_GB,
            drive.available_space as f64 / BYTES_PER_GB,
            drive.fs_type
        ));

        let temp = match TempDir::create(PathBuf::from(&drive.mount_point).join(TEMP_DIR_NAME)) {
            Ok(t) => t,
            Err(e) => {
                let msg = format!(
                    "Failed to create temp directory on {}: {}",
                    drive.mount_point, e
                );
                ctx.log(format!("ERROR: {}", msg));
                return ctx.finish(false, Some(msg));
            }
        };
        let test_file = temp.path.join("stability_test.bin");

        // Phase 2: sequential write
        ctx.log(format!(
            "Phase 2: Sequential write test ({} MB)...",
            opts.test_size_mb()
        ));
        let write = match write_test(&mut ctx, &test_file, opts.test_size_bytes) {
            Ok(t) => t,
            Err(e) => return fail_phase(ctx, temp, "Write Test Failed", e, "write_failed",
                "The USB drive may be write-protected, corrupted, or failing. Try a different USB port or drive."),
        };
        report_speed(&mut ctx, &write, false, opts.test_size_mb());

        // Phase 3: sequential read
        ctx.log(format!(
            "Phase 3: Sequential read test ({} MB)...",
            opts.test_size_mb()
        ));
        let read = match read_test(&mut ctx, &test_file, opts.test_size_bytes) {
            Ok(t) => t,
            Err(e) => {
                return fail_phase(
                    ctx,
                    temp,
                    "Read Test Failed",
                    e,
                    "read_failed",
                    "The USB drive may be corrupted or disconnected during the test.",
                )
            }
        };
        report_speed(&mut ctx, &read, true, opts.test_size_mb());

        // Phase 4: integrity
        let integrity = if opts.verify_integrity {
            ctx.log("Phase 4: Data integrity verification...");
            let result = verify_test(&mut ctx, &test_file, opts.test_size_bytes);
            report_integrity(&mut ctx, &result, opts.test_size_bytes);
            Some(result)
        } else {
            ctx.log("Phase 4: Skipped (integrity check disabled)");
            None
        };

        // Phase 5: random I/O
        ctx.log("Phase 5: Random I/O latency test (100 random 4KB reads)...");
        match random_io_test(&test_file, opts.test_size_bytes) {
            Ok(latency) => report_random_io(&mut ctx, latency),
            Err(e) => {
                ctx.log(format!("Warning: Random I/O test failed: {}", e));
                ctx.finding(
                    FindingSeverity::Warning,
                    "Random I/O Test Failed",
                    e,
                    None,
                    Some(json!({ "type": "usb_error", "error": "random_io_failed" })),
                );
            }
        }

        // Phase 6: capacity spot-check
        ctx.log("Phase 6: Capacity verification...");
        capacity_check(&mut ctx, &drive, opts.test_size_bytes);

        temp.cleanup(&mut ctx);

        let integrity_pass = integrity.as_ref().is_none_or(|r| r.pass);
        summarize(
            ctx,
            &drive,
            &opts,
            &write,
            &read,
            integrity.as_ref(),
            integrity_pass,
        )
    }
}

// =============================================================================
// Options & drive selection
// =============================================================================

struct TestOptions {
    target_drive: String,
    intensity: String,
    verify_integrity: bool,
    test_size_bytes: u64,
}

impl TestOptions {
    fn from_json(options: &serde_json::Value) -> Self {
        let intensity = options
            .get("test_intensity")
            .and_then(|v| v.as_str())
            .unwrap_or("standard")
            .to_string();
        let test_size_bytes = match intensity.as_str() {
            "quick" => 256 * 1024 * 1024,
            "thorough" => 1024 * 1024 * 1024,
            _ => 512 * 1024 * 1024,
        };
        Self {
            target_drive: options
                .get("target_drive")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            intensity,
            verify_integrity: options
                .get("verify_integrity")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            test_size_bytes,
        }
    }

    fn test_size_mb(&self) -> u64 {
        self.test_size_bytes / (1024 * 1024)
    }
}

struct Drive {
    mount_point: String,
    label: String,
    fs_type: String,
    total_space: u64,
    available_space: u64,
}

type DriveError = (&'static str, String, &'static str, &'static str);

fn same_mount(a: &str, b: &str) -> bool {
    a.trim_end_matches('\\')
        .eq_ignore_ascii_case(b.trim_end_matches('\\'))
}

fn select_drive(target: &str, test_size_bytes: u64) -> Result<Drive, DriveError> {
    let disks = Disks::new_with_refreshed_list();
    let removable: Vec<_> = disks
        .list()
        .iter()
        .filter(|d| d.is_removable() && d.total_space() > 0)
        .collect();

    if removable.is_empty() {
        return Err((
            "No USB Drives Found",
            "No removable USB drives were detected on this system. Please connect a USB drive and try again.".to_string(),
            "Insert a USB drive and ensure it is recognized by Windows before running this test.",
            "no_drives",
        ));
    }

    let disk = if target.is_empty() {
        removable.first().copied()
    } else {
        removable
            .iter()
            .find(|d| same_mount(&d.mount_point().to_string_lossy(), target))
            .copied()
    }
    .ok_or_else(|| {
        (
            "Drive Not Found",
            format!(
                "Drive '{}' not found or is not a removable USB drive",
                target
            ),
            "Check that the USB drive is properly connected and try again.",
            "drive_not_found",
        )
    })?;

    let name = disk.name().to_string_lossy().to_string();
    let drive = Drive {
        mount_point: disk.mount_point().to_string_lossy().to_string(),
        label: if name.is_empty() {
            "Removable Disk".to_string()
        } else {
            name
        },
        fs_type: disk.file_system().to_string_lossy().to_string(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
    };

    if drive.available_space < test_size_bytes + SPACE_HEADROOM {
        return Err((
            "Insufficient Free Space",
            format!(
                "Insufficient free space on {}. Need {:.0} MB free, only {:.0} MB available.",
                drive.mount_point,
                (test_size_bytes + SPACE_HEADROOM) as f64 / BYTES_PER_MB,
                drive.available_space as f64 / BYTES_PER_MB
            ),
            "Free up space on the USB drive or use a lower test intensity.",
            "insufficient_space",
        ));
    }

    Ok(drive)
}

/// Temporary test directory, removed when dropped (on every exit path).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn create(path: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// Remove now and log the outcome.
    fn cleanup(self, ctx: &mut ServiceCtx) {
        match fs::remove_dir_all(&self.path) {
            Ok(()) => ctx.log("Cleanup: Temporary test files removed"),
            Err(e) => ctx.log(format!("Warning: Failed to clean up temp files: {}", e)),
        }
        std::mem::forget(self);
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn fail_phase(
    mut ctx: ServiceCtx,
    temp: TempDir,
    title: &str,
    error: String,
    code: &str,
    recommendation: &str,
) -> ServiceResult {
    ctx.log(format!("ERROR: {}: {}", title, error));
    temp.cleanup(&mut ctx);
    ctx.finding(
        FindingSeverity::Critical,
        title,
        error.clone(),
        Some(recommendation.to_string()),
        Some(json!({ "type": "usb_error", "error": code })),
    );
    ctx.finish(false, Some(error))
}

// =============================================================================
// Phases
// =============================================================================

struct Throughput {
    bytes: u64,
    duration: Duration,
}

impl Throughput {
    fn mbps(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs > 0.0 {
            self.bytes as f64 / BYTES_PER_MB / secs
        } else {
            0.0
        }
    }
}

fn pattern_byte(index: usize) -> u8 {
    if index.is_multiple_of(2) {
        PATTERN_BYTE_A
    } else {
        PATTERN_BYTE_B
    }
}

fn check_cancel(ctx: &ServiceCtx) -> Result<(), String> {
    if ctx.is_cancelled() {
        Err("Cancelled by user".to_string())
    } else {
        Ok(())
    }
}

/// Log at each 10% step of `done` out of `total`.
fn log_progress(ctx: &mut ServiceCtx, label: &str, done: u64, total: u64, last_step: &mut u64) {
    let step = done * 10 / total.max(1);
    if step > *last_step {
        *last_step = step;
        ctx.log(format!(
            "  {} progress: {}% ({} MB)",
            label,
            step * 10,
            done / (1024 * 1024)
        ));
    }
}

fn write_test(ctx: &mut ServiceCtx, path: &Path, size: u64) -> Result<Throughput, String> {
    let chunk: Vec<u8> = (0..CHUNK_SIZE).map(pattern_byte).collect();
    let total_chunks = size as usize / CHUNK_SIZE;
    let start = Instant::now();
    let mut written = 0u64;
    let mut last_step = 0;

    let mut file =
        fs::File::create(path).map_err(|e| format!("Failed to create test file: {}", e))?;
    for i in 0..total_chunks {
        check_cancel(ctx)?;
        file.write_all(&chunk)
            .map_err(|e| format!("Write error at chunk {}: {}", i, e))?;
        written += CHUNK_SIZE as u64;
        log_progress(ctx, "Write", written, size, &mut last_step);
    }
    file.sync_all()
        .map_err(|e| format!("Failed to sync: {}", e))?;

    Ok(Throughput {
        bytes: written,
        duration: start.elapsed(),
    })
}

fn read_test(ctx: &mut ServiceCtx, path: &Path, size: u64) -> Result<Throughput, String> {
    let start = Instant::now();
    let mut read = 0u64;
    let mut last_step = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];

    let mut file =
        fs::File::open(path).map_err(|e| format!("Failed to open test file for reading: {}", e))?;
    loop {
        check_cancel(ctx)?;
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        read += n as u64;
        log_progress(ctx, "Read", read, size, &mut last_step);
    }

    Ok(Throughput {
        bytes: read,
        duration: start.elapsed(),
    })
}

struct Integrity {
    pass: bool,
    errors: u64,
    first_error_offset: Option<u64>,
}

fn verify_test(ctx: &mut ServiceCtx, path: &Path, size: u64) -> Integrity {
    let result = (|| -> Result<Integrity, String> {
        let mut file = fs::File::open(path)
            .map_err(|e| format!("Failed to open test file for verification: {}", e))?;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut offset = 0u64;
        let mut errors = 0u64;
        let mut first_error_offset = None;
        let mut last_step = 0;

        loop {
            check_cancel(ctx)?;
            let n = file
                .read(&mut buf)
                .map_err(|e| format!("Read error during verification: {}", e))?;
            if n == 0 {
                break;
            }
            // Chunks are CHUNK_SIZE (even) long, so the pattern restarts per chunk.
            for (i, &byte) in buf[..n].iter().enumerate() {
                if byte != pattern_byte(i) {
                    errors += 1;
                    first_error_offset.get_or_insert(offset + i as u64);
                }
            }
            offset += n as u64;
            log_progress(ctx, "Verify", offset, size, &mut last_step);
        }

        Ok(Integrity {
            pass: errors == 0,
            errors,
            first_error_offset,
        })
    })();

    match result {
        Ok(r) => {
            if r.pass {
                ctx.log("Integrity check PASSED — all bytes verified correctly");
            } else {
                ctx.log(format!(
                    "Integrity check FAILED — {} byte errors detected (first at offset {})",
                    r.errors,
                    r.first_error_offset.unwrap_or(0)
                ));
            }
            r
        }
        Err(e) => {
            ctx.log(format!("ERROR: Integrity verification failed: {}", e));
            Integrity {
                pass: false,
                errors: 0,
                first_error_offset: None,
            }
        }
    }
}

struct Latency {
    avg_ms: f64,
    min_ms: f64,
    max_ms: f64,
}

fn random_io_test(path: &Path, size: u64) -> Result<Latency, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("Failed to open file for random I/O: {}", e))?;
    let mut buf = vec![0u8; RANDOM_IO_BLOCK];
    let mut latencies = Vec::with_capacity(RANDOM_IO_ITERATIONS);

    // Deterministic pseudo-random offsets (LCG) so runs are comparable.
    let mut seed: u64 = 12345;
    for _ in 0..RANDOM_IO_ITERATIONS {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = seed % size.saturating_sub(RANDOM_IO_BLOCK as u64).max(1);

        let start = Instant::now();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Seek error: {}", e))?;
        file.read_exact(&mut buf)
            .map_err(|e| format!("Random read error: {}", e))?;
        latencies.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(Latency {
        avg_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
        min_ms: latencies.iter().cloned().fold(f64::INFINITY, f64::min),
        max_ms: latencies.iter().cloned().fold(0.0_f64, f64::max),
    })
}

fn capacity_check(ctx: &mut ServiceCtx, drive: &Drive, expected_used: u64) {
    let disks = Disks::new_with_refreshed_list();
    let Some(disk) = disks
        .list()
        .iter()
        .find(|d| same_mount(&d.mount_point().to_string_lossy(), &drive.mount_point))
    else {
        ctx.log("Warning: Could not re-detect drive for capacity check");
        return;
    };

    let used_by_test = drive.available_space.saturating_sub(disk.available_space());
    // Allow 5% tolerance + 10MB overhead for filesystem metadata
    let tolerance = (expected_used as f64 * 0.05) as u64 + SPACE_HEADROOM;
    let discrepancy = used_by_test.abs_diff(expected_used);
    let ok = discrepancy <= tolerance;

    let (expected_mb, used_mb, diff_mb) = (
        expected_used / 1_048_576,
        used_by_test / 1_048_576,
        discrepancy / 1_048_576,
    );
    ctx.log(format!(
        "Capacity check: expected ~{} MB used, actual ~{} MB used (discrepancy: {} MB)",
        expected_mb, used_mb, diff_mb
    ));

    if ok {
        ctx.finding(
            FindingSeverity::Success,
            "Capacity Verification: PASS",
            format!(
                "Reported capacity matches actual storage. Wrote {} MB, space used: {} MB.",
                expected_mb, used_mb
            ),
            None,
            Some(json!({
                "type": "usb_capacity",
                "pass": true,
                "expectedMb": expected_mb,
                "actualUsedMb": used_mb,
                "discrepancyMb": diff_mb,
            })),
        );
    } else {
        ctx.log("WARNING: Significant capacity discrepancy — possible fake drive!");
        ctx.finding(
            FindingSeverity::Critical,
            "⚠ FAKE DRIVE SUSPECTED",
            format!(
                "Capacity mismatch detected! Expected ~{} MB used but only ~{} MB was actually stored. This drive may report a larger capacity than its actual storage.",
                expected_mb, used_mb
            ),
            Some("Do NOT use this drive for important data. It likely has less real storage than advertised. Consider returning it.".to_string()),
            Some(json!({
                "type": "usb_capacity",
                "pass": false,
                "expectedMb": expected_mb,
                "actualUsedMb": used_mb,
                "discrepancyMb": diff_mb,
                "fakeDriveSuspected": true,
            })),
        );
    }
}

// =============================================================================
// Findings
// =============================================================================

fn report_speed(ctx: &mut ServiceCtx, t: &Throughput, is_read: bool, size_mb: u64) {
    let speed = t.mbps();
    let secs = t.duration.as_secs_f64();
    let (verb, kind, bytes_key) = if is_read {
        ("Read", "usb_read_speed", "bytesRead")
    } else {
        ("Write", "usb_write_speed", "bytesWritten")
    };
    ctx.log(format!(
        "{} complete: {:.1} MB/s ({:.1}s for {} MB)",
        verb, speed, secs, size_mb
    ));

    let rating = rate_speed(speed, is_read);
    let mut data = json!({
        "type": kind,
        "speedMbps": speed,
        "durationSecs": secs,
        "rating": rating.label,
    });
    data[bytes_key] = json!(t.bytes);

    ctx.finding(
        rating.severity,
        format!("Sequential {}: {:.1} MB/s", verb, speed),
        format!(
            "{} {} MB in {:.1} seconds. {}",
            if is_read { "Read" } else { "Wrote" },
            size_mb,
            secs,
            rating.description
        ),
        rating.recommendation,
        Some(data),
    );
}

fn report_integrity(ctx: &mut ServiceCtx, r: &Integrity, size: u64) {
    let size_mb = size / (1024 * 1024);
    let severity = if r.pass {
        FindingSeverity::Success
    } else if r.errors < 100 {
        FindingSeverity::Error
    } else {
        FindingSeverity::Critical
    };
    ctx.finding(
        severity,
        if r.pass {
            "Data Integrity: PASS".to_string()
        } else {
            format!("Data Integrity: FAIL ({} errors)", r.errors)
        },
        if r.pass {
            format!("All {} MB of written data verified byte-by-byte with zero corruption.", size_mb)
        } else {
            format!(
                "{} byte-level errors detected across {} MB. First error at offset {}. This indicates flash cell failure or a counterfeit drive.",
                r.errors,
                size_mb,
                r.first_error_offset.unwrap_or(0)
            )
        },
        (!r.pass).then(|| "This drive should NOT be used for important data. Replace it immediately — data stored on this drive may become corrupt.".to_string()),
        Some(json!({
            "type": "usb_integrity",
            "pass": r.pass,
            "errorCount": r.errors,
            "firstErrorOffset": r.first_error_offset,
            "bytesVerified": size,
        })),
    );
}

fn report_random_io(ctx: &mut ServiceCtx, l: Latency) {
    ctx.log(format!(
        "Random I/O: avg {:.2}ms, min {:.2}ms, max {:.2}ms",
        l.avg_ms, l.min_ms, l.max_ms
    ));

    let (severity, description) = if l.avg_ms > 50.0 {
        (FindingSeverity::Error, "Very slow random access — the drive controller may be failing or severely bottlenecked.")
    } else if l.avg_ms > 10.0 {
        (
            FindingSeverity::Warning,
            "Elevated random access latency. Typical for USB 2.0 or older flash drives.",
        )
    } else if l.avg_ms > 2.0 {
        (
            FindingSeverity::Info,
            "Normal random access latency for a USB flash drive.",
        )
    } else {
        (
            FindingSeverity::Success,
            "Excellent random access latency — consistent with USB 3.0+ drives.",
        )
    };

    ctx.finding(
        severity,
        format!("Random I/O: {:.2}ms avg latency", l.avg_ms),
        format!(
            "100 random 4KB reads — avg {:.2}ms, min {:.2}ms, max {:.2}ms. {}",
            l.avg_ms, l.min_ms, l.max_ms, description
        ),
        (l.avg_ms > 50.0).then(|| {
            "Consider replacing this drive. High latency indicates controller issues.".to_string()
        }),
        Some(json!({
            "type": "usb_random_io",
            "avgMs": l.avg_ms,
            "minMs": l.min_ms,
            "maxMs": l.max_ms,
            "iterations": RANDOM_IO_ITERATIONS,
            "blockSize": RANDOM_IO_BLOCK,
        })),
    );
}

fn summarize(
    mut ctx: ServiceCtx,
    drive: &Drive,
    opts: &TestOptions,
    write: &Throughput,
    read: &Throughput,
    integrity: Option<&Integrity>,
    integrity_pass: bool,
) -> ServiceResult {
    let worst = |sev: FindingSeverity| ctx.findings.iter().any(|f| f.severity == sev);
    let (has_critical, has_errors, has_warnings) = (
        worst(FindingSeverity::Critical),
        worst(FindingSeverity::Error),
        worst(FindingSeverity::Warning),
    );

    let (overall_status, overall_severity) = if has_critical {
        ("FAIL", FindingSeverity::Critical)
    } else if has_errors {
        ("ISSUES DETECTED", FindingSeverity::Error)
    } else if has_warnings {
        ("PASS WITH WARNINGS", FindingSeverity::Warning)
    } else {
        ("PASS", FindingSeverity::Success)
    };

    let total_secs = ctx.elapsed_ms() as f64 / 1000.0;
    ctx.log(format!(
        "USB Stability Test complete: {} (took {:.1}s)",
        overall_status, total_secs
    ));

    ctx.findings.insert(
        0,
        ServiceFinding {
            severity: overall_severity,
            title: format!("USB Stability Test: {}", overall_status),
            description: format!(
                "Tested {} ({}) — {} on {}. Write: {:.1} MB/s, Read: {:.1} MB/s. Integrity: {}.",
                drive.mount_point,
                drive.label,
                format_bytes(drive.total_space),
                drive.fs_type,
                write.mbps(),
                read.mbps(),
                match integrity {
                    None => "skipped",
                    Some(_) if integrity_pass => "PASS",
                    Some(_) => "FAIL",
                }
            ),
            recommendation: None,
            data: Some(json!({
                "type": "usb_summary",
                "drivePath": drive.mount_point,
                "volumeLabel": drive.label,
                "fileSystem": drive.fs_type,
                "totalSpaceBytes": drive.total_space,
                "availableSpaceBytes": drive.available_space,
                "testSizeMb": opts.test_size_mb(),
                "intensity": opts.intensity,
                "writeSpeedMbps": write.mbps(),
                "readSpeedMbps": read.mbps(),
                "writeDurationSecs": write.duration.as_secs_f64(),
                "readDurationSecs": read.duration.as_secs_f64(),
                "integrityPass": integrity_pass,
                "integrityErrors": integrity.map(|i| i.errors).unwrap_or(0),
                "integrityChecked": integrity.is_some(),
                "overallStatus": overall_status,
                "totalDurationSecs": total_secs,
            })),
        },
    );

    ctx.finish(!has_critical && !has_errors, None)
}

// =============================================================================
// Helper Types & Functions
// =============================================================================

struct SpeedRating {
    severity: FindingSeverity,
    label: String,
    description: String,
    recommendation: Option<String>,
}

fn rate_speed(speed_mbps: f64, is_read: bool) -> SpeedRating {
    let direction = if is_read { "read" } else { "write" };

    if speed_mbps >= 100.0 {
        SpeedRating {
            severity: FindingSeverity::Success,
            label: "Excellent".to_string(),
            description: format!("Excellent {} speed — consistent with USB 3.0+.", direction),
            recommendation: None,
        }
    } else if speed_mbps >= 50.0 {
        SpeedRating {
            severity: FindingSeverity::Success,
            label: "Good".to_string(),
            description: format!("Good {} speed — typical for USB 3.0 drives.", direction),
            recommendation: None,
        }
    } else if speed_mbps >= 20.0 {
        SpeedRating {
            severity: FindingSeverity::Info,
            label: "Average".to_string(),
            description: format!(
                "Average {} speed — possibly USB 2.0 or a slower USB 3.0 drive.",
                direction
            ),
            recommendation: Some(
                "Try a USB 3.0 port if available for better performance.".to_string(),
            ),
        }
    } else if speed_mbps >= 5.0 {
        SpeedRating {
            severity: FindingSeverity::Warning,
            label: "Slow".to_string(),
            description: format!("Slow {} speed — typical of USB 2.0 connections or aging flash memory.", direction),
            recommendation: Some("Ensure the drive is in a USB 3.0 port. If already in USB 3.0, the drive's flash memory may be worn.".to_string()),
        }
    } else {
        SpeedRating {
            severity: FindingSeverity::Error,
            label: "Very Slow".to_string(),
            description: format!("Very slow {} speed — indicates a failing drive or severely bottlenecked connection.", direction),
            recommendation: Some("This drive may be failing. Consider replacing it.".to_string()),
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / BYTES_PER_GB;
    if gb >= 1000.0 {
        format!("{:.1} TB", gb / 1024.0)
    } else {
        format!("{:.1} GB", gb)
    }
}
