/**
 * Agent system type definitions
 * Types for the agentic AI system including settings and command execution
 */

import type * as B from '@/bindings';
// =============================================================================
// Provider Types
// =============================================================================

/**
 * Supported AI providers
 */
export type AgentProvider = B.AgentProvider;

/**
 * Provider configuration
 */
export interface ProviderConfig {
  id: AgentProvider;
  name: string;
  /** Placeholder text for model input */
  modelPlaceholder: string;
  requiresApiKey: boolean;
  requiresBaseUrl: boolean;
  /** Default base URL for providers that need one */
  defaultBaseUrl?: string;
  /** Help text or documentation link */
  helpText?: string;
}

/**
 * Per-provider API key storage
 */
export type ProviderApiKeys = B.ProviderApiKeys_Serialize;

/**
 * Available providers with their configurations
 */
export const AGENT_PROVIDERS: ProviderConfig[] = [
  {
    id: "openai",
    name: "OpenAI",
    modelPlaceholder: "gpt-4o, gpt-4o-mini, o1, o3-mini...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from platform.openai.com",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    modelPlaceholder: "claude-sonnet-4-0, claude-opus-4-0, claude-3-5-haiku...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from console.anthropic.com",
  },
  {
    id: "xai",
    name: "xAI Grok",
    modelPlaceholder: "grok-4, grok-3, grok-3-mini-fast...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from console.x.ai",
  },
  {
    id: "google",
    name: "Google Gemini",
    modelPlaceholder: "gemini-2.0-flash, gemini-1.5-pro...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from aistudio.google.com",
  },
  {
    id: "mistral",
    name: "Mistral",
    modelPlaceholder: "mistral-large-latest, pixtral-large...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from console.mistral.ai",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    modelPlaceholder: "deepseek-chat, deepseek-reasoner...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from platform.deepseek.com",
  },
  {
    id: "groq",
    name: "Groq",
    modelPlaceholder: "llama-3.3-70b-versatile, mixtral-8x7b...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Get your API key from console.groq.com",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    modelPlaceholder: "anthropic/claude-3.5-sonnet, openai/gpt-4o...",
    requiresApiKey: true,
    requiresBaseUrl: false,
    helpText: "Meta-provider for 100+ models. Get key from openrouter.ai",
  },
  {
    id: "ollama",
    name: "Ollama (Local)",
    modelPlaceholder: "llama3.2, mistral, deepseek-coder...",
    requiresApiKey: false,
    requiresBaseUrl: true,
    defaultBaseUrl: "http://localhost:11434",
    helpText: "Run models locally. Install from ollama.com",
  },
  {
    id: "custom",
    name: "Custom OpenAI-Compatible",
    modelPlaceholder: "Enter model name...",
    requiresApiKey: true,
    requiresBaseUrl: true,
    helpText: "Works with any OpenAI-compatible API endpoint",
  },
];

// =============================================================================
// MCP Client Types
// =============================================================================

/**
 * Configuration for an external MCP server the agent can connect to
 */
export type MCPServerConfig = B.MCPServerConfig_Serialize;

// =============================================================================
// Command Approval Types
// =============================================================================

/**
 * Command approval mode
 */
export type ApprovalMode = B.ApprovalMode;

/**
 * A command awaiting user approval
 */
export type PendingCommand = B.PendingCommand_Serialize;

// =============================================================================
// Search Types
// =============================================================================

/**
 * Search provider options
 */
export type SearchProvider = B.SearchProvider;

// =============================================================================
// Conversation Types
// =============================================================================

/**
 * A saved agent conversation
 */
export type Conversation = B.Conversation;

// =============================================================================
// Agent Settings
// =============================================================================

/**
 * Agent configuration settings
 */
export type AgentSettings = B.AgentSettings_Serialize;

/**
 * Default agent settings
 */
export const DEFAULT_AGENT_SETTINGS: AgentSettings = {
  provider: 'openai',
  model: 'gpt-4o-mini',
  apiKeys: {},
  baseUrl: undefined,
  approvalMode: 'always',
  whitelistedCommands: [
    '^ipconfig',
    '^ping ',
    '^systeminfo$',
    '^tasklist$',
    '^hostname$',
    '^whoami$',
  ],
  searchProvider: 'none',
  tavilyApiKey: undefined,
  searxngUrl: undefined,
  systemPrompt: undefined,
  // MCP Server Settings
  mcpServerEnabled: false,
  mcpApiKey: undefined,
  mcpPort: 8377,
  mcpAllowRemote: false,
  // MCP Client Settings
  mcpServers: [],
};

// =============================================================================
// Chat Types
// =============================================================================

/**
 * Role of a message in conversation
 */
export type MessageRole = "user" | "assistant" | "system" | "tool";

// =============================================================================
// File System Types
// =============================================================================

export type Instrument = B.Instrument;
