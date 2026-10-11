//! Ping Test Service
//!
//! Tests network connectivity by pinging specified hosts.

use serde_json::json;

use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, ServiceDefinition, ServiceFinding, ServiceOptionSchema,
    ServiceResult,
};

// =============================================================================
// Service Implementation
// =============================================================================

pub struct PingTestService;

impl Service for PingTestService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "ping-test".to_string(),
            name: "Ping Test".to_string(),
            description: "Tests network connectivity by pinging specified hosts".to_string(),
            category: "diagnostics".to_string(),
            estimated_duration_secs: 10,
            required_programs: vec![],
            options: vec![
                ServiceOptionSchema {
                    id: "target".to_string(),
                    label: "Target Host".to_string(),
                    option_type: OptionType::String,
                    default_value: json!("8.8.8.8"),
                    min: None,
                    max: None,
                    options: None,
                    description: Some("IP address or hostname to ping".to_string()),
                },
                ServiceOptionSchema {
                    id: "count".to_string(),
                    label: "Ping Count".to_string(),
                    option_type: OptionType::Number,
                    default_value: json!(4),
                    min: Some(1.0),
                    max: Some(100.0),
                    options: None,
                    description: Some("Number of ping requests to send".to_string()),
                },
            ],
            icon: "wifi".to_string(),
            exclusive_resources: vec![],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        let mut success = true;
        let mut error: Option<String> = None;

        let target = options
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("8.8.8.8");
        let count = options.get("count").and_then(|v| v.as_u64()).unwrap_or(4) as u32;

        ctx.log(format!(
            "Starting ping test to {} ({} pings)",
            target, count
        ));

        // Run Windows ping command
        let output =
            ctx.output(crate::proc::command("ping").args(["-n", &count.to_string(), target]));

        match output {
            Ok(output) => {
                let stdout = crate::proc::decode_output(&output.stdout);
                let stderr = crate::proc::decode_output(&output.stderr);

                // Log each line
                for line in stdout.lines() {
                    if !line.trim().is_empty() {
                        ctx.log(line);
                    }
                }

                if !stderr.is_empty() {
                    ctx.log(format!("Error: {}", stderr));
                }

                // Locale-independent parse (shared with the Network Diagnostics page)
                let parsed =
                    crate::commands::network_diagnostics::parse_ping_output(target, &stdout, count);
                let avg_latency: Option<f64> = parsed.avg_ms.map(f64::from);
                let packet_loss: Option<u32> =
                    (!parsed.replies.is_empty()).then(|| parsed.packet_loss_percent.round() as u32);

                // Generate findings
                if parsed.packets_received > 0 {
                    if let Some(avg) = avg_latency {
                        let severity = if avg < 50.0 {
                            FindingSeverity::Success
                        } else if avg < 100.0 {
                            FindingSeverity::Info
                        } else if avg < 200.0 {
                            FindingSeverity::Warning
                        } else {
                            FindingSeverity::Error
                        };

                        ctx.findings.push(ServiceFinding {
                            severity,
                            title: format!("Average Latency: {:.0}ms", avg),
                            description: format!(
                                "Ping to {} completed with average latency of {:.0}ms",
                                target, avg
                            ),
                            recommendation: if avg > 100.0 {
                                Some("High latency detected. Check network connection.".to_string())
                            } else {
                                None
                            },
                            data: Some(json!({
                                "avgLatency": avg,
                                "target": target,
                                "type": "latency"
                            })),
                        });
                    }

                    if let Some(loss) = packet_loss {
                        let severity = if loss == 0 {
                            FindingSeverity::Success
                        } else if loss < 10 {
                            FindingSeverity::Warning
                        } else {
                            FindingSeverity::Error
                        };

                        ctx.findings.push(ServiceFinding {
                            severity,
                            title: format!("Packet Loss: {}%", loss),
                            description: format!(
                                "{} out of {} packets lost",
                                (loss * count) / 100,
                                count
                            ),
                            recommendation: if loss > 0 {
                                Some("Packet loss detected. Network may be unstable.".to_string())
                            } else {
                                None
                            },
                            data: Some(json!({
                                "packetLoss": loss,
                                "type": "packet_loss"
                            })),
                        });
                    }

                    ctx.log("Ping test completed successfully");
                } else {
                    success = false;
                    error = Some("Ping command failed".to_string());
                    ctx.findings.push(ServiceFinding {
                        severity: FindingSeverity::Error,
                        title: "Ping Failed".to_string(),
                        description: format!("Unable to reach {}", target),
                        recommendation: Some(
                            "Check network connection and ensure host is reachable.".to_string(),
                        ),
                        data: None,
                    });
                }
            }
            Err(e) => {
                success = false;
                error = Some(format!("Failed to execute ping command: {}", e));
                ctx.log(format!("Error: {}", e));
            }
        }

        ctx.finish(success, error)
    }
}
