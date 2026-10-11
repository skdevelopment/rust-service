//! Agent system types
//!
//! Types for the agentic AI system including settings, memory, and command execution.

use serde::{Deserialize, Serialize};

// =============================================================================
// Provider Types
// =============================================================================

/// Supported AI providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentProvider {
    #[default]
    OpenAI,
    Anthropic,
    #[serde(rename = "xai")]
    Xai,
    Google,
    Mistral,
    DeepSeek,
    Groq,
    OpenRouter,
    Ollama,
    Custom,
}

/// Per-provider API key storage
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderApiKeys {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xai: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groq: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}

// =============================================================================
// Command Approval Types
// =============================================================================

/// Command approval mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ApprovalMode {
    #[default]
    Always,
    Whitelist,
    Yolo,
}

/// Status of a pending command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CommandStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
}

/// A command awaiting user approval
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingCommand {
    pub id: String,
    pub command: String,
    pub reason: String,
    pub created_at: String,
    pub status: CommandStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// =============================================================================
// Search Types
// =============================================================================

/// Search provider options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SearchProvider {
    Tavily,
    Searxng,
    #[default]
    None,
}

/// Search result from web search
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

// =============================================================================
// Conversation Types
// =============================================================================

/// A saved agent conversation
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A message within a conversation (serialized CoreMessage content)
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    /// Role: "user", "assistant", or "tool"
    pub role: String,
    /// JSON-serialized message content
    pub content: String,
    pub created_at: String,
}

/// Conversation with its messages
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationWithMessages {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub messages: Vec<ConversationMessage>,
}

// =============================================================================
// Agent Settings
// =============================================================================

/// Agent configuration settings
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    /// AI provider to use
    #[serde(default)]
    pub provider: AgentProvider,

    /// Model name/identifier
    #[serde(default = "default_model")]
    pub model: String,

    /// Per-provider API key storage
    #[serde(default)]
    pub api_keys: ProviderApiKeys,

    /// Base URL for custom/Ollama providers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Command approval mode
    #[serde(default)]
    pub approval_mode: ApprovalMode,

    /// Whitelisted command patterns (regex)
    #[serde(default = "default_whitelist")]
    pub whitelisted_commands: Vec<String>,

    /// Search provider to use
    #[serde(default)]
    pub search_provider: SearchProvider,

    /// Tavily API key
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tavily_api_key: Option<String>,

    /// SearXNG instance URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_url: Option<String>,

    /// Custom system prompt
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    // ==========================================================================
    // MCP Server Settings
    // ==========================================================================
    /// Whether the MCP HTTP server is enabled
    #[serde(default)]
    pub mcp_server_enabled: bool,

    /// API key for MCP server authentication (auto-generated)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_api_key: Option<String>,

    /// Port for the MCP HTTP server
    #[serde(default = "default_mcp_port")]
    pub mcp_port: u16,

    /// Listen on all network interfaces instead of only 127.0.0.1
    #[serde(default)]
    pub mcp_allow_remote: bool,

    // ==========================================================================
    // MCP Client Settings (connecting to external servers)
    // ==========================================================================
    /// External MCP servers the agent can connect to for additional tools
    #[serde(default)]
    pub mcp_servers: Vec<MCPServerConfig>,
}

/// Transport type for MCP server connections
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MCPTransportType {
    #[default]
    Sse,
    Http,
}

/// Configuration for an external MCP server the agent connects to
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfig {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Server URL
    pub url: String,
    /// Transport type (sse or http)
    #[serde(default)]
    pub transport_type: MCPTransportType,
    /// Whether this server is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Optional API key for authentication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional custom headers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_whitelist() -> Vec<String> {
    vec![
        "^ipconfig".to_string(),
        "^ping ".to_string(),
        "^systeminfo$".to_string(),
        "^tasklist$".to_string(),
        "^hostname$".to_string(),
        "^whoami$".to_string(),
    ]
}

fn default_mcp_port() -> u16 {
    8377
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            provider: AgentProvider::default(),
            model: default_model(),
            api_keys: ProviderApiKeys::default(),
            base_url: None,
            approval_mode: ApprovalMode::default(),
            whitelisted_commands: default_whitelist(),
            search_provider: SearchProvider::default(),
            tavily_api_key: None,
            searxng_url: None,
            system_prompt: None,
            // MCP Server Settings
            mcp_server_enabled: false,
            mcp_api_key: None,
            mcp_port: default_mcp_port(),
            mcp_allow_remote: false,
            // MCP Client Settings
            mcp_servers: Vec::new(),
        }
    }
}

// =============================================================================
// Tool Execution Types
// =============================================================================

/// Command execution result
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

// =============================================================================
// File Attachment Types
// =============================================================================

/// Categories of files for specialized handling
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FileCategory {
    Text,
    Code,
    Document,
    Image,
    Media,
    Binary,
}

impl FileCategory {
    /// Get category from file extension
    pub fn from_extension(filename: &str) -> Self {
        let ext = filename.split('.').next_back().unwrap_or("").to_lowercase();

        let code_exts = [
            "js", "ts", "jsx", "tsx", "py", "rs", "java", "cpp", "c", "h", "hpp", "go", "rb",
            "php", "swift", "kt", "scala", "r", "m", "mm", "cs", "vb", "fs", "hs", "lua", "pl",
            "sh", "bash", "zsh", "fish", "ps1", "cmd", "bat", "sql", "html", "css", "scss", "sass",
            "less", "xml", "yaml", "yml", "toml", "ini", "conf", "config", "json", "md",
            "markdown",
        ];
        if code_exts.contains(&ext.as_str()) {
            return Self::Code;
        }

        let image_exts = [
            "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "raw",
        ];
        if image_exts.contains(&ext.as_str()) {
            return Self::Image;
        }

        let media_exts = [
            "mp3", "mp4", "wav", "avi", "mov", "mkv", "flv", "wmv", "webm", "ogg", "ogv",
        ];
        if media_exts.contains(&ext.as_str()) {
            return Self::Media;
        }

        let doc_exts = [
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt",
        ];
        if doc_exts.contains(&ext.as_str()) {
            return Self::Document;
        }

        Self::Binary
    }

    /// Check if this category should have content auto-extracted
    pub fn should_auto_extract(&self) -> bool {
        matches!(self, Self::Text | Self::Code)
    }
}

/// Source of a file attachment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    Upload,
    Generated,
    Filesystem,
}

/// Unified file attachment metadata
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    pub id: String,
    pub source: FileSource,
    pub original_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub category: FileCategory,
    pub size: u64,
    pub stored_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<u32>,
    pub checksum: String,
    pub uploaded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(flatten)]
    pub metadata: FileAttachmentMetadata,
}

/// Source-specific metadata for file attachments
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct FileAttachmentMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_metadata: Option<UploadMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_metadata: Option<GenerationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesystem_metadata: Option<FilesystemMetadata>,
}

/// Metadata for user-uploaded files
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UploadMetadata {
    pub uploaded_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    pub auto_extracted: bool,
}

/// Metadata for agent-generated files
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerationMetadata {
    pub generated_by: String,
    pub description: String,
    pub tool_call_id: String,
    pub approved: bool,
}

/// Metadata for filesystem-referenced files
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemMetadata {
    pub original_path: String,
    pub accessed_at: String,
    pub auto_read: bool,
}

/// Result of validating a filesystem path
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PathValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sanitized_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub within_sandbox: bool,
}

/// File size limits (in bytes)
pub const FILE_SIZE_SMALL: u64 = 10 * 1024 * 1024; // 10 MB
                                                   // 100 MB
                                                   // 1 GB
                                                   // 1 MB
pub const MAX_CONTENT_EXTRACTION_SIZE: usize = 100 * 1024; // 100 KB

/// Helper to format file size for display
pub fn format_file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let k = 1024_f64;
    let i = (bytes as f64).log(k).floor() as usize;
    let size = bytes as f64 / k.powi(i as i32);
    format!("{:.2} {}", size, UNITS[i.min(UNITS.len() - 1)])
}

/// Helper to compute SHA-256 checksum
pub fn compute_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
