//! FurMark GPU Stress Test Service
//!
//! Executes FurMark GPU stress test with configurable duration and resolution.
//! Parses console output for FPS metrics, GPU temperature, and usage stats.

use std::path::Path;

use regex::Regex;
use serde_json::json;

use crate::commands::required_programs::get_program_exe_path;
use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, ServiceDefinition, ServiceFinding, ServiceOptionSchema,
    ServiceResult,
};

// =============================================================================
// Service Implementation
// =============================================================================

pub struct FurmarkService;

impl Service for FurmarkService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "furmark".to_string(),
            name: "GPU Stress Test (FurMark)".to_string(),
            description: "Stress test GPU with FurMark to check stability and thermals".to_string(),
            category: "diagnostics".to_string(),
            estimated_duration_secs: 120,
            required_programs: vec!["furmark".to_string()],
            options: vec![
                ServiceOptionSchema {
                    id: "duration_seconds".to_string(),
                    label: "Duration (seconds)".to_string(),
                    option_type: OptionType::Number,
                    default_value: json!(60),
                    min: Some(10.0),
                    max: Some(600.0),
                    options: None,
                    description: Some("How long to run the stress test".to_string()),
                },
                ServiceOptionSchema {
                    id: "width".to_string(),
                    label: "Width".to_string(),
                    option_type: OptionType::Number,
                    default_value: json!(1920),
                    min: Some(640.0),
                    max: Some(3840.0),
                    options: None,
                    description: Some("Render resolution width".to_string()),
                },
                ServiceOptionSchema {
                    id: "height".to_string(),
                    label: "Height".to_string(),
                    option_type: OptionType::Number,
                    default_value: json!(1080),
                    min: Some(480.0),
                    max: Some(2160.0),
                    options: None,
                    description: Some("Render resolution height".to_string()),
                },
            ],
            icon: "flame".to_string(),
            exclusive_resources: vec!["cpu-stress".to_string()],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        ctx.log("Starting FurMark GPU stress test...");

        // Get FurMark executable path
        let exe_path = match get_program_exe_path("furmark".to_string()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                ctx.log("ERROR: FurMark executable not found");
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Error,
                    title: "FurMark Not Found".to_string(),
                    description: "FurMark executable was not found.".to_string(),
                    recommendation: Some(
                        "Download FurMark from Geeks3D and place it in the programs folder."
                            .to_string(),
                    ),
                    data: Some(json!({"type": "error", "reason": "exe_not_found"})),
                });
                return ctx.finish(false, Some("FurMark executable not found".to_string()));
            }
            Err(e) => {
                ctx.log(format!("ERROR: Failed to locate FurMark: {}", e));
                return ctx.finish(false, Some(e.to_string()));
            }
        };

        ctx.log(format!("Found FurMark at: {}", exe_path));

        // Parse options
        let duration = options
            .get("duration_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(60) as u32;
        let width = options
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(1920) as u32;
        let height = options
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(1080) as u32;

        ctx.log(format!(
            "Running {}x{} stress test for {} seconds...",
            width, height, duration
        ));

        // Build command arguments
        let args = vec![
            "--demo".to_string(),
            "furmark-gl".to_string(),
            "--width".to_string(),
            width.to_string(),
            "--height".to_string(),
            height.to_string(),
            "--max-time".to_string(),
            duration.to_string(),
        ];

        // Get working directory
        let working_dir = Path::new(&exe_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Execute FurMark
        let output = match ctx.output(
            crate::proc::command(&exe_path)
                .args(&args)
                .current_dir(&working_dir),
        ) {
            Ok(output) => output,
            Err(e) => {
                ctx.log(format!("ERROR: Failed to execute FurMark: {}", e));
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Error,
                    title: "Execution Failed".to_string(),
                    description: format!("Failed to run FurMark: {}", e),
                    recommendation: Some(
                        "Ensure FurMark is accessible and not corrupted.".to_string(),
                    ),
                    data: Some(json!({"type": "error", "reason": "execution_failed"})),
                });
                return ctx.finish(false, Some(format!("Execution failed: {}", e)));
            }
        };

        let stdout = crate::proc::decode_output(&output.stdout).to_string();
        let stderr = crate::proc::decode_output(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        ctx.log(format!("FurMark completed with exit code: {}", exit_code));

        if exit_code != 0 {
            ctx.log("FurMark exited with non-zero code");
            ctx.findings.push(ServiceFinding {
                severity: FindingSeverity::Error,
                title: "Test Failed".to_string(),
                description: format!("FurMark exited with code {}", exit_code),
                recommendation: Some(
                    "Check if GPU drivers are up to date and FurMark is compatible.".to_string(),
                ),
                data: Some(json!({
                    "type": "error",
                    "reason": "non_zero_exit",
                    "exitCode": exit_code,
                    "stderr": stderr.chars().take(500).collect::<String>(),
                })),
            });
            return ctx.finish(false, Some(format!("Exit code: {}", exit_code)));
        }

        // Parse output
        let parsed = parse_furmark_output(&stdout);

        // Log results
        if let Some(ref renderer) = parsed.renderer {
            ctx.log(format!("GPU: {}", renderer));
        }
        if let Some(avg) = parsed.fps_avg {
            ctx.log(format!("Average FPS: {}", avg));
        }
        if let Some(frames) = parsed.frames {
            ctx.log(format!("Total frames: {}", frames));
        }

        // Log GPU metrics if available
        if let Some(gpu) = parsed.gpus.first() {
            if let Some(temp) = gpu.max_temperature_c {
                ctx.log(format!("Max GPU temperature: {}°C", temp));
            }
            if let Some(usage) = gpu.max_usage_percent {
                ctx.log(format!("Max GPU usage: {}%", usage));
            }
        }

        // Determine severity based on results
        let (severity, title, description) = if let Some(gpu) = parsed.gpus.first() {
            let temp = gpu.max_temperature_c.unwrap_or(0);
            if temp >= 95 {
                (
                    FindingSeverity::Error,
                    "GPU Overheating".to_string(),
                    format!(
                        "GPU reached {}°C during stress test - this is dangerously high!",
                        temp
                    ),
                )
            } else if temp >= 85 {
                (
                    FindingSeverity::Warning,
                    "High GPU Temperature".to_string(),
                    format!("GPU reached {}°C - consider improving cooling.", temp),
                )
            } else {
                (
                    FindingSeverity::Success,
                    "GPU Stress Test Passed".to_string(),
                    format!(
                        "GPU stable at {}°C with {} avg FPS.",
                        temp,
                        parsed.fps_avg.unwrap_or(0)
                    ),
                )
            }
        } else if parsed.fps_avg.is_some() {
            (
                FindingSeverity::Success,
                "Stress Test Complete".to_string(),
                format!(
                    "FurMark completed with {} avg FPS.",
                    parsed.fps_avg.unwrap_or(0)
                ),
            )
        } else {
            (
                FindingSeverity::Info,
                "Test Complete".to_string(),
                "FurMark stress test completed.".to_string(),
            )
        };

        ctx.findings.push(ServiceFinding {
            severity,
            title,
            description,
            recommendation: if parsed
                .gpus
                .first()
                .and_then(|g| g.max_temperature_c)
                .unwrap_or(0)
                >= 85
            {
                Some("Clean dust from GPU heatsink/fans or improve case airflow.".to_string())
            } else {
                None
            },
            data: Some(json!({
                "type": "furmark_result",
                "demo": parsed.demo,
                "renderer": parsed.renderer,
                "api": parsed.api,
                "resolution": parsed.resolution,
                "frames": parsed.frames,
                "durationMs": parsed.duration_ms,
                "fps": parsed.fps,
                "gpus": parsed.gpus,
            })),
        });

        ctx.log("FurMark stress test complete");

        ctx.finish(true, None)
    }
}

// =============================================================================
// Output Parsing
// =============================================================================

#[derive(Debug, Clone, Default, serde::Serialize, specta::Type)]
struct FurmarkGpuInfo {
    index: u32,
    name: String,
    id: String,
    max_temperature_c: Option<u32>,
    max_usage_percent: Option<u32>,
    max_core_clock_mhz: Option<u32>,
    min_core_clock_mhz: Option<u32>,
}

#[derive(Debug, Default, serde::Serialize, specta::Type)]
struct FurmarkParsedOutput {
    demo: Option<String>,
    renderer: Option<String>,
    api: Option<String>,
    resolution: Option<Resolution>,
    frames: Option<u32>,
    duration_ms: Option<u32>,
    fps: Option<FpsStats>,
    fps_avg: Option<u32>,
    gpus: Vec<FurmarkGpuInfo>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
struct Resolution {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
struct FpsStats {
    min: u32,
    avg: u32,
    max: u32,
}

fn parse_furmark_output(output: &str) -> FurmarkParsedOutput {
    let mut result = FurmarkParsedOutput::default();
    let mut current_gpu: Option<FurmarkGpuInfo> = None;

    // Compile regex patterns
    let re_demo = Regex::new(r"(?i)^-\s*demo\s*:\s*(.+)$").ok();
    let re_renderer = Regex::new(r"(?i)^-\s*renderer\s*:\s*(.+)$").ok();
    let re_api = Regex::new(r"(?i)^-\s*3D API\s*:\s*(.+)$").ok();
    let re_resolution = Regex::new(r"(?i)^-\s*resolution\s*:\s*(\d+)x(\d+)$").ok();
    let re_frames = Regex::new(r"(?i)^-\s*frames\s*:\s*(\d+)$").ok();
    let re_duration = Regex::new(r"(?i)^-\s*duration\s*:\s*(\d+)\s*ms").ok();
    let re_fps = Regex::new(r"(?i)^-\s*FPS.*?:\s*(\d+)\s*/\s*(\d+)\s*/\s*(\d+)").ok();
    let re_gpu_header = Regex::new(r"^-\s*GPU\s+(\d+):\s*(.+?)\s*\[(.+?)\]").ok();
    let re_gpu_temp = Regex::new(r"\.max temperature:\s*(\d+)").ok();
    let re_gpu_usage = Regex::new(r"\.max usage:\s*(\d+)%").ok();
    let re_gpu_core_max = Regex::new(r"(?i)\.max core clock:\s*(\d+)\s*MHz").ok();
    let re_gpu_core_min = Regex::new(r"(?i)\.min core clock:\s*(\d+)\s*MHz").ok();

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Check for GPU header
        if let Some(ref re) = re_gpu_header {
            if let Some(caps) = re.captures(line) {
                // Push previous GPU if any
                if let Some(gpu) = current_gpu.take() {
                    result.gpus.push(gpu);
                }
                current_gpu = Some(FurmarkGpuInfo {
                    index: caps.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0)),
                    name: caps.get(2).map_or("", |m| m.as_str()).trim().to_string(),
                    id: caps.get(3).map_or("", |m| m.as_str()).trim().to_string(),
                    ..Default::default()
                });
                continue;
            }
        }

        // GPU metrics (lines starting with .)
        if let Some(ref mut gpu) = current_gpu {
            if line.starts_with('.') {
                if let Some(ref re) = re_gpu_temp {
                    if let Some(caps) = re.captures(line) {
                        gpu.max_temperature_c = caps.get(1).and_then(|m| m.as_str().parse().ok());
                    }
                }
                if let Some(ref re) = re_gpu_usage {
                    if let Some(caps) = re.captures(line) {
                        gpu.max_usage_percent = caps.get(1).and_then(|m| m.as_str().parse().ok());
                    }
                }
                if let Some(ref re) = re_gpu_core_max {
                    if let Some(caps) = re.captures(line) {
                        gpu.max_core_clock_mhz = caps.get(1).and_then(|m| m.as_str().parse().ok());
                    }
                }
                if let Some(ref re) = re_gpu_core_min {
                    if let Some(caps) = re.captures(line) {
                        gpu.min_core_clock_mhz = caps.get(1).and_then(|m| m.as_str().parse().ok());
                    }
                }
                continue;
            }
        }

        // General metrics
        if let Some(ref re) = re_demo {
            if let Some(caps) = re.captures(line) {
                result.demo = caps.get(1).map(|m| m.as_str().trim().to_string());
                continue;
            }
        }
        if let Some(ref re) = re_renderer {
            if let Some(caps) = re.captures(line) {
                result.renderer = caps.get(1).map(|m| m.as_str().trim().to_string());
                continue;
            }
        }
        if let Some(ref re) = re_api {
            if let Some(caps) = re.captures(line) {
                result.api = caps.get(1).map(|m| m.as_str().trim().to_string());
                continue;
            }
        }
        if let Some(ref re) = re_resolution {
            if let Some(caps) = re.captures(line) {
                let width = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                let height = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                result.resolution = Some(Resolution { width, height });
                continue;
            }
        }
        if let Some(ref re) = re_frames {
            if let Some(caps) = re.captures(line) {
                result.frames = caps.get(1).and_then(|m| m.as_str().parse().ok());
                continue;
            }
        }
        if let Some(ref re) = re_duration {
            if let Some(caps) = re.captures(line) {
                result.duration_ms = caps.get(1).and_then(|m| m.as_str().parse().ok());
                continue;
            }
        }
        if let Some(ref re) = re_fps {
            if let Some(caps) = re.captures(line) {
                let min = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                let avg = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                let max = caps
                    .get(3)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);
                result.fps = Some(FpsStats { min, avg, max });
                result.fps_avg = Some(avg);
            }
        }
    }

    // Push last GPU if still open
    if let Some(gpu) = current_gpu {
        result.gpus.push(gpu);
    }

    result
}
