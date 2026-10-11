/**
 * Agent Chat Service
 *
 * Handles AI streaming with multi-provider support using Vercel AI SDK.
 * Supports: OpenAI, Anthropic, xAI, Google, Mistral, DeepSeek, Groq, OpenRouter, Ollama, Custom
 */

import {
  streamText,
  stepCountIs,
  type LanguageModel,
  type ModelMessage,
  type ToolSet,
} from "ai";
import { createOpenAI } from "@ai-sdk/openai";
import { createAnthropic } from "@ai-sdk/anthropic";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createMistral } from "@ai-sdk/mistral";
import { createXai } from "@ai-sdk/xai";
import { createGroq } from "@ai-sdk/groq";
import {
  AGENT_PROVIDERS,
  type AgentSettings,
  type AgentProvider,
  type ProviderApiKeys,
} from "@/types/agent";
import { commands } from '@/bindings';
import { enabledTools } from '@/lib/agent-tools';

// =============================================================================
// Provider Configuration
// =============================================================================

/**
 * Base URLs for OpenAI-compatible providers
 */
const PROVIDER_BASE_URLS: Partial<Record<AgentProvider, string>> = {
  deepseek: "https://api.deepseek.com",
  openrouter: "https://openrouter.ai/api/v1",
};

/**
 * Whether the selected provider has everything it needs to chat
 */
export function isAgentConfigured(settings: AgentSettings | undefined): boolean {
  if (!settings || !settings.model?.trim()) return false;
  const info = AGENT_PROVIDERS.find(p => p.id === settings.provider);
  const key = settings.apiKeys?.[settings.provider as keyof ProviderApiKeys];
  if ((info?.requiresApiKey ?? true) && !key?.trim()) return false;
  if (info?.requiresBaseUrl && !settings.baseUrl?.trim()) return false;
  return true;
}

/**
 * Create a language model instance based on provider settings
 */
export function createProviderModel(settings: AgentSettings): LanguageModel {
  const { provider, model, apiKeys, baseUrl } = settings;
  const apiKey = apiKeys?.[provider as keyof ProviderApiKeys] || "";

  switch (provider) {
    case "openai": {
      const openai = createOpenAI({ apiKey });
      return openai(model);
    }

    case "anthropic": {
      const anthropic = createAnthropic({ apiKey });
      return anthropic(model);
    }

    case "xai": {
      const xai = createXai({ apiKey });
      return xai(model);
    }

    case "google": {
      const google = createGoogleGenerativeAI({ apiKey });
      return google(model);
    }

    case "mistral": {
      const mistral = createMistral({ apiKey });
      return mistral(model);
    }

    case "groq": {
      const groq = createGroq({ apiKey });
      return groq(model);
    }

    case "deepseek": {
      // DeepSeek uses OpenAI-compatible API (Chat Completions)
      const deepseek = createOpenAI({
        apiKey,
        baseURL: PROVIDER_BASE_URLS.deepseek,
      });
      return deepseek.chat(model);
    }

    case "openrouter": {
      // OpenRouter uses OpenAI-compatible API but doesn't support the Responses API
      // Use .chat() to force Chat Completions API instead
      const openrouter = createOpenAI({
        apiKey,
        baseURL: PROVIDER_BASE_URLS.openrouter,
      });
      return openrouter.chat(model);
    }

    case "ollama": {
      // Ollama uses OpenAI-compatible API (Chat Completions)
      const ollamaUrl = baseUrl || "http://localhost:11434/v1";
      const ollama = createOpenAI({
        apiKey: "ollama", // Ollama doesn't require a real API key
        baseURL: ollamaUrl,
      });
      return ollama.chat(model);
    }

    case "custom": {
      // Custom OpenAI-compatible endpoint (Chat Completions)
      if (!baseUrl) {
        throw new Error("Custom provider requires a base URL");
      }
      const custom = createOpenAI({
        apiKey,
        baseURL: baseUrl,
      });
      return custom.chat(model);
    }

    default:
      throw new Error(`Unsupported provider: ${provider}`);
  }
}

// =============================================================================
// Chat Streaming
// =============================================================================

export interface StreamChatOptions {
  messages: ModelMessage[];
  settings: AgentSettings;
  tools?: ToolSet;
  systemPrompt?: string;
  abortSignal?: AbortSignal;
  /** Max tool steps before stopping (default: 10, use higher for service supervision) */
  maxSteps?: number;
}

/** The streamText result: consume `fullStream`, then read `response`. */
export type StreamChatResult = ReturnType<typeof streamText>;

/**
 * Default system prompt for the agent
 */
const DEFAULT_SYSTEM_PROMPT = `You are ServiceAgent, an autonomous AI assistant built into a Windows desktop repair toolkit called RustService. You help computer repair technicians diagnose and fix Windows computers.

## Environment
- **OS**: Windows 10/11 (you are running ON the target machine)
- **Shell**: PowerShell 5.1+ (use PowerShell syntax ONLY, never bash/Linux)
- **Context**: You are a portable tool running from a USB drive or local install
- **User**: A computer repair technician who needs fast, accurate system work

## Core Behavior

You are an AGENTIC assistant. This means:
1. **Complete tasks fully** - Don't stop after one step. Keep going until the goal is achieved.
2. **Chain tools together** - Multi-step tasks should flow: diagnose → analyze → fix → verify.
3. **Be autonomous** - Make decisions and act. Don't ask "should I proceed?" - just do it.
4. **Learn from errors** - If a command fails, analyze the error and try a different approach immediately.
5. **Explain as you go** - Brief explanations before each action, detailed analysis after results.

## Tool Execution Strategy

You can use **multiple tools per step** when they are independent and don't depend on each other's results (e.g., reading two files at once, checking disk health and network status simultaneously).

However, always **analyze results before deciding next actions** — don't blindly chain steps without reviewing output. For dependent operations, execute sequentially across steps so you can adapt your approach based on results.

## Thinking Process

For every task, follow this pattern:
1. **Assess** - What do I need to accomplish? What information do I need first?
2. **Plan** - What's the sequence of steps? Prioritize gathering info before making changes.
3. **Execute** - Run ONE command, analyze the result.
4. **Evaluate** - Did it work? What did I learn? What's next?
5. **Report** - Summarize findings clearly when the task is complete.

## PowerShell Quick Reference
\`\`\`powershell
# System info
Get-ComputerInfo | Select-Object CsName, OsName, OsArchitecture, OsBuildNumber
systeminfo | Select-String "OS Name|Total Physical Memory|System Boot Time"

# Disk health
Get-PhysicalDisk | Select-Object FriendlyName, MediaType, HealthStatus, Size
Get-Volume | Select-Object DriveLetter, FileSystemLabel, SizeRemaining, Size | Format-Table -AutoSize

# Processes & services
Get-Process | Sort-Object CPU -Descending | Select-Object -First 15 Name, CPU, WorkingSet64
Get-Service | Where-Object {$_.Status -eq "Running"} | Select-Object -First 20

# Network
Get-NetIPAddress -AddressFamily IPv4 | Select-Object InterfaceAlias, IPAddress
Test-NetConnection -ComputerName 8.8.8.8 -InformationLevel Quiet

# Files
Get-ChildItem -Path "$env:USERPROFILE\\Downloads" -File | Sort-Object LastWriteTime -Descending | Select-Object Name, @{N="Size(MB)";E={[math]::Round($_.Length/1MB,2)}}, LastWriteTime -First 15 | Format-Table -AutoSize

# Temp cleanup
Get-ChildItem "$env:TEMP" -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
\`\`\`

## Response Formatting
- Use **markdown tables** when presenting structured data (files, processes, disks, etc.)
- Use **code blocks** for commands and outputs
- Use **bold** for important findings
- Keep explanations concise but informative
- Use emoji sparingly for visual categorization (📁 folders, ⚠️ warnings, ✅ success)

## IMPORTANT RULES
- ALWAYS use tools when asked to perform actions - don't just describe what you would do
- Keep going until the task is DONE - one tool call is rarely enough
- If a command fails with a syntax error, FIX IT and retry immediately
- When showing file listings or system data, format as a clean markdown table
- Never apologize excessively - just fix the issue and move on

## SERVICE AUTOMATION

You can run diagnostic and maintenance services on the system. Services are modular tools (disk checks, malware scans, system file repair, etc.) that execute sequentially in a queue.

### Running Services
1. **Discover**: Use \`list_services\` to see available services and \`list_service_presets\` for preset bundles
2. **Validate**: Use \`check_service_requirements\` to verify required programs are installed
3. **Propose**: Describe the queue you'll run and WHY each service is included
4. **Execute**: Use \`run_service_queue\` to start the run (user must approve)
5. **Supervise**: You'll receive updates as each service completes — monitor for issues
6. **Intervene**: If a service fails or reports critical findings, use \`pause_service\` and investigate
7. **Report**: After completion, review all findings, write analysis, and export the report

### Supervision Protocol
When monitoring a running service queue:
- After each service completes, you'll receive its results as a service update
- Briefly acknowledge each result (1-2 sentences)
- If severity is **error** or **critical**: explain the issue, consider pausing to investigate
- If a service fails entirely: note it and decide whether remaining services should continue
- After all services complete: do a comprehensive review

### Report Writing
After a service run completes:
1. Call \`get_report_statistics\` for an overview
2. Review each service's findings with \`get_service_report\`
3. Write per-service analysis with \`set_service_analysis\` (plain language, actionable)
4. Edit findings if needed with \`edit_finding\` (make customer-friendly)
5. Write an executive summary with \`set_report_summary\`
6. Set a health score with \`set_health_score\` (0-100)
7. Export a text copy with \`export_report\` if the technician wants a file (printable PDFs come from the Reports page)
8. Summarize the key findings for the technician

### Symptom-to-Service Mapping
When users describe problems, map their symptoms to appropriate services:
- **Slow/laggy**: disk-space, bleachbit, sfc, dism, drivecleanup
- **Crashes/BSOD**: chkdsk, sfc, dism, driver-audit, smartctl
- **Virus/malware**: kvrt-scan, adwcleaner, stinger, bleachbit
- **Network issues**: ping-test, speedtest, network-config
- **Battery problems**: battery-report, energy-report
- **Disk issues**: disk-space, smartctl, chkdsk, bleachbit, drivecleanup
- **General checkup**: ping-test, disk-space, sfc, dism, driver-audit, smartctl, network-config`;

/** Tool list for the system prompt, generated from the tool registry */
function toolsSection(settings: AgentSettings): string {
  const lines = enabledTools(settings).map(t =>
    `- **${t.name}**: ${t.summary}${t.approval === 'auto' ? '' : ' (requires approval)'}`,
  );
  return `\n\n## Available Tools\n${lines.join('\n')}`;
}

/**
 * Stream a chat response from the AI provider
 */
export async function streamChat(options: StreamChatOptions): Promise<StreamChatResult> {
  const { messages, settings, tools, abortSignal, maxSteps = 10 } = options;

  let systemPrompt = (options.systemPrompt || settings.systemPrompt || DEFAULT_SYSTEM_PROMPT) + toolsSection(settings);

  try {
    const instruments = await commands.listInstruments();
    if (instruments.length > 0) {
      systemPrompt += `\n\n## AVAILABLE CUSTOM INSTRUMENTS\nYou can run these special tools by name using 'run_instrument':\n`;
      for (const i of instruments) {
        systemPrompt += `- **${i.name}** (.${i.extension}): ${i.description}\n`;
      }
    }
  } catch (error) {
    console.warn("Failed to load instruments for the system prompt:", error);
  }

  return streamText({
    model: createProviderModel(settings),
    system: systemPrompt,
    messages,
    // Only pass tools if there are any defined - some models don't support tools
    ...(tools && Object.keys(tools).length > 0 ? { tools } : {}),
    stopWhen: stepCountIs(maxSteps),
    abortSignal,
  });
}

// =============================================================================
// History normalization (conversations saved by older versions)
// =============================================================================

type ToolOutput =
  | { type: 'text' | 'error-text'; value: string }
  | { type: 'json' | 'error-json'; value: unknown };

/** Convert any stored tool result shape into the SDK's ToolResultOutput. */
function normalizeToolOutput(part: Record<string, unknown>): ToolOutput {
  const out = part.output as Record<string, unknown> | undefined;
  if (out && typeof out === 'object' && typeof out.type === 'string') {
    const t = out.type;
    if (t === 'text' || t === 'error-text') {
      return { type: t, value: typeof out.value === 'string' ? out.value : JSON.stringify(out.value ?? '') };
    }
    if (t === 'json' || t === 'error-json') return { type: t, value: out.value ?? null };
  }
  // Legacy: { result, isError } or a raw value in `output`
  const raw = part.result ?? part.output;
  const isError = part.isError === true
    || (raw !== null && typeof raw === 'object' && (raw as { status?: unknown }).status === 'error');
  if (typeof raw === 'string') return { type: isError ? 'error-text' : 'text', value: raw };
  return { type: isError ? 'error-json' : 'json', value: raw ?? null };
}

/**
 * Make a stored history safe to send to the SDK: legacy tool-call `args`
 * become `input`, tool results get a valid `output`, and tool calls left
 * without a result (e.g. the app closed mid-approval) get an error result.
 */
export function normalizeHistory(messages: unknown[]): ModelMessage[] {
  const out: ModelMessage[] = [];
  const answered = new Set<string>();
  for (const m of messages) {
    if (m && typeof m === 'object' && (m as { role?: unknown }).role === 'tool') {
      for (const p of ((m as { content?: unknown[] }).content ?? []) as Array<Record<string, unknown>>) {
        if (typeof p.toolCallId === 'string') answered.add(p.toolCallId);
      }
    }
  }

  for (const raw of messages) {
    if (!raw || typeof raw !== 'object') continue;
    const msg = raw as { role?: string; content?: unknown };
    if (msg.role === 'assistant' && Array.isArray(msg.content)) {
      const content = (msg.content as Array<Record<string, unknown>>).map(part =>
        part.type === 'tool-call'
          ? { type: 'tool-call' as const, toolCallId: String(part.toolCallId), toolName: String(part.toolName), input: part.input ?? part.args ?? {} }
          : part,
      );
      out.push({ role: 'assistant', content } as unknown as ModelMessage);
      const unanswered = content.filter(
        (p): p is { type: 'tool-call'; toolCallId: string; toolName: string; input: unknown } =>
          p.type === 'tool-call' && !answered.has(p.toolCallId as string),
      );
      if (unanswered.length > 0) {
        out.push({
          role: 'tool',
          content: unanswered.map(c => ({
            type: 'tool-result' as const,
            toolCallId: c.toolCallId,
            toolName: c.toolName,
            output: { type: 'error-text' as const, value: 'Not run: the conversation ended before this action was approved.' },
          })),
        });
      }
    } else if (msg.role === 'tool' && Array.isArray(msg.content)) {
      out.push({
        role: 'tool',
        content: (msg.content as Array<Record<string, unknown>>)
          .filter(p => p.type === 'tool-result')
          .map(p => ({
            type: 'tool-result' as const,
            toolCallId: String(p.toolCallId),
            toolName: String(p.toolName),
            output: normalizeToolOutput(p),
          })),
      } as ModelMessage);
    } else if (msg.role === 'user' || msg.role === 'assistant' || msg.role === 'system') {
      out.push(msg as ModelMessage);
    }
  }
  return out;
}
