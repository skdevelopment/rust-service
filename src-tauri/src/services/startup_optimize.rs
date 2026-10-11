//! Startup Optimizer Service
//!
//! Enumerates all Windows startup items, sends them to the user's configured
//! LLM for intelligent classification (essential / useful / unnecessary),
//! and optionally disables the unnecessary ones to improve boot time.
//!
//! Falls back to heuristic pattern matching when no AI provider is configured.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::startup::{
    get_registry_startup_items_sync, get_scheduled_startup_tasks_sync,
    get_startup_folder_items_sync, set_startup_item_enabled, StartupItem,
};
use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, ServiceDefinition, ServiceFinding, ServiceOptionSchema,
    ServiceResult,
};

// =============================================================================
// Classification
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
enum Classification {
    Essential,
    Useful,
    Unnecessary,
}

impl Classification {
    fn as_str(&self) -> &'static str {
        match self {
            Classification::Essential => "essential",
            Classification::Useful => "useful",
            Classification::Unnecessary => "unnecessary",
        }
    }
}

// =============================================================================
// AI Classification
// =============================================================================

/// Build the prompt for the LLM to classify startup items.
fn build_classification_prompt(items: &[&StartupItem]) -> String {
    let mut prompt = String::from(
        r#"You are a Windows startup optimization expert. Classify each startup item as exactly one of:
- "essential": System-critical items that should NEVER be disabled (security software, hardware drivers, audio/GPU/touchpad/Bluetooth drivers, OEM hardware utilities, core Windows services)
- "useful": Items the user likely wants but that aren't system-critical (productivity tools they actively use, VPN clients, backup software). When in doubt, classify as "useful" to be conservative.
- "unnecessary": Items safe to disable for faster boot (game launchers, social/media apps that auto-start, cloud sync that can be started manually, software updaters, old utilities)

For each item, consider the name, command path, publisher, and source. Be conservative — only mark items as "unnecessary" when you're confident they aren't needed at startup.

Respond with ONLY a valid JSON object mapping each item number to its classification. Example:
{"1": "essential", "2": "unnecessary", "3": "useful"}

Items to classify:
"#,
    );

    for (i, item) in items.iter().enumerate() {
        prompt.push_str(&format!(
            "\n{}. Name: \"{}\" | Command: \"{}\" | Publisher: \"{}\" | Source: {} | Enabled: {}",
            i + 1,
            item.name,
            item.command,
            item.publisher.as_deref().unwrap_or("Unknown"),
            item.source,
            item.enabled,
        ));
    }

    prompt
}

/// Parse the LLM's JSON response into a classification map.
fn parse_ai_classifications(response: &str, count: usize) -> HashMap<usize, Classification> {
    let mut result = HashMap::new();

    // Extract JSON from response (LLM might wrap it in markdown code blocks)
    let json_str = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(json_str) {
        for (key, value) in &parsed {
            if let Ok(idx) = key.parse::<usize>() {
                if idx >= 1 && idx <= count {
                    let class = match value.to_lowercase().as_str() {
                        "essential" => Classification::Essential,
                        "unnecessary" => Classification::Unnecessary,
                        _ => Classification::Useful,
                    };
                    result.insert(idx - 1, class); // Convert to 0-indexed
                }
            }
        }
    }

    result
}

/// Classify items using the configured LLM. Returns classifications + whether AI was used.
fn classify_with_ai(
    items: &[StartupItem],
    ctx: &mut ServiceCtx,
) -> (Vec<(Classification, usize)>, bool) {
    let item_refs: Vec<&StartupItem> = items.iter().collect();

    if let Some(config) = crate::llm::configured() {
        ctx.log(format!(
            "Using AI classification ({:?} / {})",
            config.provider, config.model
        ));

        let prompt = build_classification_prompt(&item_refs);
        match crate::llm::complete(&config, &prompt, true) {
            Ok(response) => {
                let ai_map = parse_ai_classifications(&response, items.len());
                if ai_map.len() >= items.len() / 2 {
                    ctx.log(format!(
                        "AI classified {}/{} items successfully",
                        ai_map.len(),
                        items.len()
                    ));
                    let classified = (0..items.len())
                        .map(|i| (ai_map.get(&i).copied().unwrap_or(Classification::Useful), i))
                        .collect();
                    return (classified, true);
                }
                ctx.log(format!(
                    "AI returned incomplete results ({}/{}), falling back to heuristics",
                    ai_map.len(),
                    items.len()
                ));
            }
            Err(e) => ctx.log(format!(
                "AI classification failed: {}. Falling back to heuristics.",
                e
            )),
        }
    } else {
        ctx.log("No AI provider configured — using heuristic classification");
    }

    let classified = (0..items.len())
        .map(|i| (classify_item_heuristic(&items[i]), i))
        .collect();
    (classified, false)
}

// =============================================================================
// Heuristic Fallback Classification
// =============================================================================

/// Patterns that indicate an **essential** startup item (never disable).
const ESSENTIAL_PATTERNS: &[&str] = &[
    "windows security",
    "securityhealth",
    "windowsdefender",
    "msascui",
    "windows defender",
    "realtek",
    "rtkngui",
    "rthdvcpl",
    "dolby",
    "nahimic",
    "waves maxxaudio",
    "conexant",
    "idt audio",
    "synaptics",
    "elantech",
    "etdctrl",
    "alps pointing",
    "trackpad",
    "touchpad",
    "precision touchpad",
    "nvidia",
    "nvcontainer",
    "amd radeon",
    "amdrsserv",
    "intel graphics",
    "igfx",
    "bluetooth",
    "lenovo",
    "dell",
    "hp ",
    "asus ",
    "acer ",
    "razer synapse",
    "corsair",
    "logitech",
    "wacom",
    "steelseries",
];

/// Patterns that indicate an **unnecessary** startup item (safe to disable).
const UNNECESSARY_PATTERNS: &[&str] = &[
    "steam",
    "steamwebhelper",
    "epicgameslauncher",
    "epic games",
    "origin",
    "battle.net",
    "gog galaxy",
    "ubisoft",
    "uplay",
    "riotclient",
    "ea app",
    "discord",
    "spotify",
    "skype",
    "telegram",
    "slack",
    "zoom",
    "whatsapp",
    "viber",
    "signal",
    "onedrive",
    "dropbox",
    "google drive",
    "googledrivesync",
    "icloud",
    "box sync",
    "mega",
    "googleupdate",
    "google update",
    "msedge update",
    "adobe update",
    "adobearm",
    "jusched",
    "java update",
    "itunes",
    "ituneshelper",
    "apple push",
    "amazon music",
    "ccleaner",
    "utorrent",
    "bittorrent",
    "teamviewer",
    "anydesk",
    "parsec",
    "wallpaper engine",
];

/// Publishers that default to essential (system / OEM) unless overridden.
const ESSENTIAL_PUBLISHERS: &[&str] = &[
    "microsoft",
    "windows",
    "intel",
    "amd",
    "nvidia",
    "realtek",
    "synaptics",
    "dell",
    "lenovo",
    "hewlett",
    "hp inc",
    "asus",
    "acer",
];

fn classify_item_heuristic(item: &StartupItem) -> Classification {
    let name_lower = item.name.to_lowercase();
    let cmd_lower = item.command.to_lowercase();
    let pub_lower = item.publisher.as_deref().unwrap_or("").to_lowercase();

    if UNNECESSARY_PATTERNS
        .iter()
        .any(|p| name_lower.contains(p) || cmd_lower.contains(p))
    {
        return Classification::Unnecessary;
    }
    if ESSENTIAL_PATTERNS
        .iter()
        .any(|p| name_lower.contains(p) || cmd_lower.contains(p))
    {
        return Classification::Essential;
    }
    if !pub_lower.is_empty() && ESSENTIAL_PUBLISHERS.iter().any(|p| pub_lower.contains(p)) {
        return Classification::Essential;
    }
    Classification::Useful
}

// =============================================================================
// Service Implementation
// =============================================================================

pub struct StartupOptimizeService;

impl Service for StartupOptimizeService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "startup-optimize".to_string(),
            name: "Startup Optimizer".to_string(),
            description:
                "AI-powered analysis of startup programs — classifies and optionally disables unnecessary items to improve boot time"
                    .to_string(),
            category: "maintenance".to_string(),
            estimated_duration_secs: 20,
            required_programs: vec![],
            options: vec![
                ServiceOptionSchema {
                    id: "disable_unnecessary".to_string(),
                    label: "Disable Unnecessary Items".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(false),
                    min: None,
                    max: None,
                    options: None,
                    description: Some(
                        "Actually disable unnecessary startup items (otherwise report only)"
                            .to_string(),
                    ),
                },
                ServiceOptionSchema {
                    id: "include_disabled".to_string(),
                    label: "Include Disabled Items".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(false),
                    min: None,
                    max: None,
                    options: None,
                    description: Some(
                        "Include already-disabled startup items in the report".to_string(),
                    ),
                },
            ],
            icon: "rocket".to_string(),
            exclusive_resources: vec![],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        let disable_unnecessary = options
            .get("disable_unnecessary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let include_disabled = options
            .get("include_disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // -----------------------------------------------------------------
        // Enumerate startup items
        // -----------------------------------------------------------------
        ctx.log("Enumerating startup items...");

        let mut items: Vec<StartupItem> = Vec::new();

        match get_registry_startup_items_sync() {
            Ok(mut v) => items.append(&mut v),
            Err(e) => ctx.log(format!(
                "Warning: failed to get registry startup items: {}",
                e
            )),
        }

        match get_startup_folder_items_sync() {
            Ok(mut v) => items.append(&mut v),
            Err(e) => ctx.log(format!(
                "Warning: failed to get startup folder items: {}",
                e
            )),
        }

        match get_scheduled_startup_tasks_sync() {
            Ok(mut v) => items.append(&mut v),
            Err(e) => ctx.log(format!(
                "Warning: failed to get scheduled startup tasks: {}",
                e
            )),
        }

        ctx.log(format!("Found {} startup items", items.len()));

        // Optionally filter out already-disabled items
        if !include_disabled {
            items.retain(|i| i.enabled);
        }

        if items.is_empty() {
            ctx.log("No startup items found.");
            ctx.findings.push(ServiceFinding {
                severity: FindingSeverity::Success,
                title: "No startup items found".to_string(),
                description: "No startup items were detected on this system.".to_string(),
                recommendation: None,
                data: Some(json!({
                    "type": "startup_optimize",
                    "mode": if disable_unnecessary { "disable" } else { "report" },
                    "aiPowered": false,
                    "totalItems": 0,
                    "essentialCount": 0,
                    "usefulCount": 0,
                    "unnecessaryCount": 0,
                    "disabledThisRun": [],
                    "failedItems": [],
                    "items": [],
                })),
            });
            return ctx.finish(true, None);
        }

        // -----------------------------------------------------------------
        // Classify (AI-powered with heuristic fallback)
        // -----------------------------------------------------------------
        ctx.log("Classifying startup items...");

        let (classified, ai_powered) = classify_with_ai(&items, &mut ctx);

        let essential_count = classified
            .iter()
            .filter(|(c, _)| *c == Classification::Essential)
            .count();
        let useful_count = classified
            .iter()
            .filter(|(c, _)| *c == Classification::Useful)
            .count();
        let unnecessary_count = classified
            .iter()
            .filter(|(c, _)| *c == Classification::Unnecessary)
            .count();
        let unnecessary_enabled: Vec<usize> = classified
            .iter()
            .filter(|(c, i)| *c == Classification::Unnecessary && items[*i].enabled)
            .map(|(_, i)| *i)
            .collect();

        ctx.log(format!(
            "Essential: {}, Useful: {}, Unnecessary: {} ({})",
            essential_count,
            useful_count,
            unnecessary_count,
            if ai_powered {
                "AI-classified"
            } else {
                "heuristic"
            },
        ));

        // -----------------------------------------------------------------
        // Optionally disable
        // -----------------------------------------------------------------
        let mut disabled_items: Vec<String> = Vec::new();
        let mut failed_items: Vec<(String, String)> = Vec::new();

        if disable_unnecessary && !unnecessary_enabled.is_empty() {
            ctx.log("Disabling unnecessary startup items...");

            for &idx in &unnecessary_enabled {
                let item = &items[idx];

                let result = set_startup_item_enabled(&item.id, false);

                match result {
                    Ok(()) => {
                        ctx.log(format!("Disabled: {}", item.name));
                        disabled_items.push(item.name.clone());
                    }
                    Err(e) => {
                        ctx.log(format!("Failed to disable '{}': {}", item.name, e));
                        failed_items.push((item.name.clone(), e));
                    }
                }
            }
        }

        // -----------------------------------------------------------------
        // Build findings
        // -----------------------------------------------------------------

        let items_data: Vec<serde_json::Value> = classified
            .iter()
            .map(|(class, idx)| {
                let item = &items[*idx];
                json!({
                    "id": item.id,
                    "name": item.name,
                    "command": item.command,
                    "source": item.source,
                    "sourceLocation": item.source_location,
                    "enabled": item.enabled,
                    "publisher": item.publisher,
                    "description": item.description,
                    "classification": class.as_str(),
                    "disabledThisRun": disabled_items.contains(&item.name),
                })
            })
            .collect();

        let mode = if disable_unnecessary {
            "disable"
        } else {
            "report"
        };

        let severity =
            if unnecessary_enabled.is_empty() || (disable_unnecessary && failed_items.is_empty()) {
                FindingSeverity::Success
            } else {
                FindingSeverity::Warning
            };

        let title = if disable_unnecessary {
            if disabled_items.is_empty() {
                "No unnecessary items to disable".to_string()
            } else {
                format!(
                    "Disabled {} unnecessary startup item(s)",
                    disabled_items.len()
                )
            }
        } else if unnecessary_enabled.is_empty() {
            "No unnecessary startup items found".to_string()
        } else {
            format!(
                "{} unnecessary startup item(s) found",
                unnecessary_enabled.len()
            )
        };

        let description = format!(
            "{} startup items analyzed{}: {} essential, {} useful, {} unnecessary.",
            classified.len(),
            if ai_powered { " by AI" } else { "" },
            essential_count,
            useful_count,
            unnecessary_count,
        );

        let recommendation = if !disable_unnecessary && !unnecessary_enabled.is_empty() {
            Some("Re-run with \"Disable Unnecessary Items\" enabled to automatically disable these items.".to_string())
        } else {
            None
        };

        ctx.findings.push(ServiceFinding {
            severity,
            title,
            description,
            recommendation,
            data: Some(json!({
                "type": "startup_optimize",
                "mode": mode,
                "aiPowered": ai_powered,
                "totalItems": classified.len(),
                "essentialCount": essential_count,
                "usefulCount": useful_count,
                "unnecessaryCount": unnecessary_count,
                "disabledThisRun": disabled_items,
                "failedItems": failed_items.iter().map(|(n, e)| json!({"name": n, "error": e})).collect::<Vec<_>>(),
                "items": items_data,
            })),
        });

        // Individual warnings for unnecessary enabled items (report mode)
        if !disable_unnecessary {
            for &idx in &unnecessary_enabled {
                let item = &items[idx];
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Warning,
                    title: format!("Unnecessary: {}", item.name),
                    description: format!(
                        "Source: {} | Publisher: {} | Command: {}",
                        item.source,
                        item.publisher.as_deref().unwrap_or("Unknown"),
                        item.command,
                    ),
                    recommendation: Some(
                        "Consider disabling this startup item to improve boot time.".to_string(),
                    ),
                    data: None,
                });
            }
        }

        // Failures
        for (name, error) in &failed_items {
            ctx.findings.push(ServiceFinding {
                severity: FindingSeverity::Error,
                title: format!("Failed to disable: {}", name),
                description: error.clone(),
                recommendation: Some("Try running as administrator.".to_string()),
                data: None,
            });
        }

        ctx.log("Startup optimization complete.");

        ctx.finish(true, None)
    }
}
