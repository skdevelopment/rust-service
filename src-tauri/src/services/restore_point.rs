//! System Restore Point Service
//!
//! Creates a Windows system restore point before maintenance work.

use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, ServiceDefinition, ServiceOptionSchema, ServiceResult,
};

pub struct RestorePointService;

impl Service for RestorePointService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "restore-point".to_string(),
            name: "System Restore Point".to_string(),
            description: "Creates a system restore point before maintenance work".to_string(),
            category: "maintenance".to_string(),
            estimated_duration_secs: 30,
            required_programs: vec![],
            options: vec![ServiceOptionSchema {
                id: "description".to_string(),
                label: "Restore Point Description".to_string(),
                option_type: OptionType::String,
                default_value: serde_json::json!("RustService Pre-Service Restore Point"),
                min: None,
                max: None,
                options: None,
                description: Some("Description for the restore point".to_string()),
            }],
            icon: "shield-check".to_string(),
            exclusive_resources: vec![],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        let description = options
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("RustService Pre-Service Restore Point")
            .to_string();

        ctx.log(format!("Creating restore point: {}", description));

        match crate::commands::restore_points::create_restore_point_blocking(&description) {
            Ok(msg) => {
                ctx.log(format!("Restore point created: {}", msg));
                ctx.finding(
                    FindingSeverity::Success,
                    "Restore Point Created",
                    format!("Successfully created restore point: {}", description),
                    None,
                    Some(serde_json::json!({
                        "type": "restore_point_result",
                        "description": description,
                        "success": true
                    })),
                );
                ctx.finish(true, None)
            }
            Err(e) => {
                let lower = e.to_lowercase();
                let is_access_denied = lower.contains("access")
                    || lower.contains("privilege")
                    || lower.contains("administrator");

                ctx.log(format!("Failed to create restore point: {}", e));
                ctx.finding(
                    if is_access_denied {
                        FindingSeverity::Warning
                    } else {
                        FindingSeverity::Error
                    },
                    if is_access_denied {
                        "Insufficient Privileges"
                    } else {
                        "Restore Point Failed"
                    },
                    e.clone(),
                    Some(if is_access_denied {
                        "Run RustService as administrator to create restore points".to_string()
                    } else {
                        "Check that System Protection is enabled for the system drive".to_string()
                    }),
                    Some(serde_json::json!({
                        "type": "restore_point_result",
                        "success": false
                    })),
                );
                ctx.finish(false, Some(e))
            }
        }
    }
}
