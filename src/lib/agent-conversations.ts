/**
 * Agent conversation persistence
 *
 * A conversation is stored as its model history (the SDK `ModelMessage`s,
 * exactly what is sent to the provider). The chat UI is rebuilt from it on
 * load, so what the technician sees always matches what the model saw.
 */

import { generateId, type ModelMessage } from 'ai';
import { commands } from '@/bindings';
import { normalizeHistory } from '@/lib/agent-chat';
import { describeToolCall } from '@/lib/agent-tools';
import type { MessagePart } from '@/components/agent/ChatMessage';
import type { AgentActivity } from '@/types/agent-activity';
import type { FileAttachment } from '@/types/file-attachment';

/** UI message shape rendered by the chat */
export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  createdAt: string;
  parts?: MessagePart[];
  attachments?: FileAttachment[];
}

/** Prefix of history messages that carry service-run updates for the agent */
export const SERVICE_UPDATE_PREFIX = '[SERVICE';

/** Human-readable text for a tool result value */
export function formatToolOutput(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    const v = value as { output?: unknown; message?: unknown; stderr?: unknown; error?: unknown };
    if (typeof v.output === 'string') {
      return typeof v.stderr === 'string' && v.stderr ? `${v.output}\n${v.stderr}`.trim() : v.output;
    }
    if (typeof v.message === 'string') return v.message;
    if (typeof v.error === 'string') return v.error;
  }
  try {
    return JSON.stringify(value, null, 2) ?? '';
  } catch {
    return String(value);
  }
}

/** Read a ToolResultOutput as { text, isError } */
export function readToolResultOutput(output: unknown): { text: string; isError: boolean } {
  const o = output as { type?: string; value?: unknown } | undefined;
  switch (o?.type) {
    case 'text':
      return { text: String(o.value ?? ''), isError: false };
    case 'error-text':
      return { text: String(o.value ?? ''), isError: true };
    case 'json':
      return { text: formatToolOutput(o.value), isError: false };
    case 'error-json':
      return { text: formatToolOutput(o.value), isError: true };
    default:
      return { text: formatToolOutput(output), isError: false };
  }
}

function userText(content: unknown): string {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .filter((p): p is { type: 'text'; text: string } => p?.type === 'text' && typeof p.text === 'string')
      .map(p => p.text)
      .join('\n');
  }
  return '';
}

/**
 * Rebuild chat messages from model history. Consecutive assistant/tool
 * messages (one multi-step turn) become a single assistant chat message.
 */
export function historyToUiMessages(history: ModelMessage[], createdAt: string): Message[] {
  const results = new Map<string, { text: string; isError: boolean }>();
  for (const m of history) {
    if (m.role === 'tool') {
      for (const part of m.content) {
        if (part.type === 'tool-result') results.set(part.toolCallId, readToolResultOutput(part.output));
      }
    }
  }

  const ui: Message[] = [];
  let current: Message | null = null;
  for (const m of history) {
    if (m.role === 'user') {
      current = null;
      const text = userText(m.content);
      if (text.startsWith(SERVICE_UPDATE_PREFIX)) {
        ui.push({ id: generateId(), role: 'system', content: text.split('\n')[0], createdAt });
      } else {
        ui.push({ id: generateId(), role: 'user', content: text, createdAt });
      }
    } else if (m.role === 'assistant') {
      if (!current) {
        current = { id: generateId(), role: 'assistant', content: '', createdAt, parts: [] };
        ui.push(current);
      }
      const parts = current.parts!;
      const content = typeof m.content === 'string' ? [{ type: 'text' as const, text: m.content }] : m.content;
      for (const part of content) {
        if (part.type === 'text' && part.text) {
          current.content += part.text;
          parts.push({ type: 'text', content: part.text });
        } else if (part.type === 'tool-call') {
          const { type, details } = describeToolCall(part.toolName, part.input);
          const result = results.get(part.toolCallId);
          parts.push({
            type: 'tool',
            activity: {
              id: part.toolCallId,
              timestamp: createdAt,
              type,
              status: result?.isError ? 'error' : 'success',
              output: result?.text,
              error: result?.isError ? result.text : undefined,
              ...details,
            } as AgentActivity,
          });
        }
      }
    }
    // tool messages are folded into the activities above
  }
  return ui;
}

/** Load a saved conversation: its history and the rebuilt chat messages */
export async function loadConversationHistory(conversationId: string) {
  const data = await commands.getConversation(conversationId);
  const raw = data.messages.map(m => {
    let content: unknown;
    try {
      content = JSON.parse(m.content);
    } catch {
      content = m.content;
    }
    return { role: m.role, content };
  });
  const history = normalizeHistory(raw);
  return {
    title: data.title,
    history,
    messages: historyToUiMessages(history, data.updatedAt),
  };
}

/** Save the model history of a conversation (replaces what was stored) */
export async function saveConversationHistory(conversationId: string, history: ModelMessage[]) {
  const now = new Date().toISOString();
  await commands.saveConversationMessages(
    conversationId,
    history.map(m => ({
      id: generateId(),
      conversationId,
      role: m.role,
      content: JSON.stringify(m.content),
      createdAt: now,
    })),
  );
}
