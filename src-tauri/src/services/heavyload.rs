//! HeavyLoad Stress Test Service
//!
//! Runs HeavyLoad.exe for CPU, memory, disk, and GPU stress testing.
//! Executes headlessly with configurable duration and reports completion status.

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

pub struct HeavyLoadService;

impl Service for HeavyLoadService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "heavyload".to_string(),
            name: "Stress Test".to_string(),
            description: "Run CPU, memory, and disk stress tests using HeavyLoad".to_string(),
            category: "diagnostics".to_string(),
            estimated_duration_secs: 300, // 5 minutes default
            required_programs: vec!["heavyload".to_string()],
            options: vec![
                ServiceOptionSchema {
                    id: "duration_minutes".to_string(),
                    label: "Duration (minutes)".to_string(),
                    option_type: OptionType::Number,
                    default_value: json!(5),
                    min: Some(1.0),
                    max: Some(60.0),
                    options: None,
                    description: Some("How long to run the stress test".to_string()),
                },
                ServiceOptionSchema {
                    id: "stress_cpu".to_string(),
                    label: "Stress CPU".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(true),
                    min: None,
                    max: None,
                    options: None,
                    description: Some("Test all CPU cores under load".to_string()),
                },
                ServiceOptionSchema {
                    id: "stress_memory".to_string(),
                    label: "Stress Memory".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(true),
                    min: None,
                    max: None,
                    options: None,
                    description: Some("Test RAM allocation and stability".to_string()),
                },
                ServiceOptionSchema {
                    id: "stress_disk".to_string(),
                    label: "Stress Disk".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(false),
                    min: None,
                    max: None,
                    options: None,
                    description: Some("Test disk I/O performance".to_string()),
                },
            ],
            icon: "weight".to_string(),
            exclusive_resources: vec!["cpu-stress".to_string()],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        // Parse options
        let duration_minutes = options
            .get("duration_minutes")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as i32;
        let stress_cpu = options
            .get("stress_cpu")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let stress_memory = options
            .get("stress_memory")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let stress_disk = options
            .get("stress_disk")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        ctx.log("Starting HeavyLoad stress test...");

        // Validate options
        if !stress_cpu && !stress_memory && !stress_disk {
            ctx.log("ERROR: At least one stress type must be enabled");
            ctx.findings.push(ServiceFinding {
                severity: FindingSeverity::Error,
                title: "Invalid Configuration".to_string(),
                description: "At least one of CPU, Memory, or Disk stress must be enabled."
                    .to_string(),
                recommendation: Some("Enable at least one stress test option.".to_string()),
                data: Some(json!({"type": "error", "reason": "no_stress_selected"})),
            });
            return ctx.finish(false, Some("No stress test type selected".to_string()));
        }

        // Get executable path
        let exe_path = match get_program_exe_path("heavyload".to_string()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                ctx.log("ERROR: HeavyLoad.exe not found");
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Error,
                    title: "HeavyLoad Not Found".to_string(),
                    description: "HeavyLoad executable was not found.".to_string(),
                    recommendation: Some(
                        "Download HeavyLoad from JAM Software and place it in the programs folder."
                            .to_string(),
                    ),
                    data: Some(json!({"type": "error", "reason": "exe_not_found"})),
                });
                return ctx.finish(false, Some("HeavyLoad.exe not found".to_string()));
            }
            Err(e) => {
                ctx.log(format!("ERROR: Failed to locate HeavyLoad: {}", e));
                return ctx.finish(false, Some(e.to_string()));
            }
        };

        ctx.log(format!("Found HeavyLoad at: {}", exe_path));

        // Build command arguments
        let mut args: Vec<String> = Vec::new();

        if stress_cpu {
            args.push("/CPU".to_string());
        }
        if stress_memory {
            args.push("/MEMORY".to_string());
        }
        if stress_disk {
            args.push("/FILE".to_string());
        }

        args.push("/DURATION".to_string());
        args.push(duration_minutes.to_string());
        args.push("/AUTOEXIT".to_string());
        args.push("/NOGUI".to_string());
        args.push("/START".to_string());

        // Build description of what we're testing
        let mut test_types: Vec<&str> = Vec::new();
        if stress_cpu {
            test_types.push("CPU");
        }
        if stress_memory {
            test_types.push("Memory");
        }
        if stress_disk {
            test_types.push("Disk");
        }
        let test_desc = test_types.join(", ");

        ctx.log(format!(
            "Running {} stress test for {} minute(s)...",
            test_desc, duration_minutes
        ));
        ctx.log("System will be under heavy load. Do not use the computer during this test.");

        // Execute HeavyLoad
        let output = match ctx.output(crate::proc::command(&exe_path).args(&args)) {
            Ok(output) => output,
            Err(e) => {
                ctx.log(format!("ERROR: Failed to execute HeavyLoad: {}", e));
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Error,
                    title: "Execution Failed".to_string(),
                    description: format!("Failed to run HeavyLoad: {}", e),
                    recommendation: Some(
                        "Ensure HeavyLoad is accessible and not corrupted.".to_string(),
                    ),
                    data: Some(json!({"type": "error", "reason": "execution_failed"})),
                });
                return ctx.finish(false, Some(format!("Execution failed: {}", e)));
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let actual_duration = std::time::Duration::from_millis(ctx.elapsed_ms());

        ctx.log(format!("HeavyLoad completed with exit code: {}", exit_code));
        ctx.log(format!(
            "Actual test duration: {:.1} minutes",
            actual_duration.as_secs_f64() / 60.0
        ));

        // Determine result
        let success = exit_code == 0;
        let (severity, status) = if success {
            (FindingSeverity::Success, "Stress test passed")
        } else {
            (
                FindingSeverity::Error,
                "Stress test failed or was interrupted",
            )
        };

        ctx.findings.push(ServiceFinding {
            severity,
            title: status.to_string(),
            description: format!(
                "System completed {} stress test for {:.1} minutes. {}",
                test_desc,
                actual_duration.as_secs_f64() / 60.0,
                if success {
                    "No stability issues detected."
                } else {
                    "The system may have stability issues under load."
                }
            ),
            recommendation: if !success {
                Some(
                    "Consider checking system temperatures, RAM stability, and power supply."
                        .to_string(),
                )
            } else {
                None
            },
            data: Some(json!({
                "type": "heavyload_result",
                "stress_cpu": stress_cpu,
                "stress_memory": stress_memory,
                "stress_disk": stress_disk,
                "requested_duration_minutes": duration_minutes,
                "actual_duration_seconds": actual_duration.as_secs(),
                "exit_code": exit_code,
                "success": success,
            })),
        });

        ctx.log("Stress test complete");

        ctx.finish(success, None)
    }
}
