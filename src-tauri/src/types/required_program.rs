//! Required program definitions for services
//!
//! Defines known external programs that services may require.
//! Each program has a stable ID, display info, and expected exe name(s).

use serde::{Deserialize, Serialize};

/// A program required by one or more services
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RequiredProgramDef {
    /// Stable identifier used in service definitions (e.g. "bleachbit")
    pub id: String,
    /// Display name
    pub name: String,
    /// Brief description
    pub description: String,
    /// Expected executable filename(s) to search for (e.g. ["bleachbit.exe", "bleachbit_console.exe"])
    pub exe_names: Vec<String>,
    /// Download/info URL (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Direct unattended download URL (exe or zip). None means install click errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

/// Status of a required program (for frontend display)
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RequiredProgramStatus {
    /// Program definition
    pub definition: RequiredProgramDef,
    /// Whether the program was found
    pub found: bool,
    /// Detected or configured path (if found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether using a custom path override
    pub is_custom: bool,
}
