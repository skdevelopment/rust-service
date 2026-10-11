/**
 * Agent Session
 *
 * Owns the agent conversation for the whole app, above the tab switcher, so
 * switching tabs never interrupts a running turn, orphans an approval or
 * drops service-run updates.
 *
 * Every turn goes through one serialized dispatch queue. A turn streams the
 * model's response, appends the SDK's own `response.messages` to the history
 * (so tool results — including tool errors and invalid calls — are always
 * paired with their calls), then handles calls to approval tools:
 * auto-approved ones run immediately; the rest suspend the turn until the
 * technician approves or denies each of them.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { generateId, type ModelMessage, type ToolResultPart } from 'ai';
import { commands } from '@/bindings';
import { useSettings } from '@/components/settings-context';
import { isAgentConfigured, streamChat } from '@/lib/agent-chat';
import {
  buildToolSet,
  describeToolCall,
  needsApproval,
  runTool,
  setExternalTools,
  type ToolRunContext,
} from '@/lib/agent-tools';
import {
  formatToolOutput,
  loadConversationHistory,
  saveConversationHistory,
  SERVICE_UPDATE_PREFIX,
  type Message,
} from '@/lib/agent-conversations';
import { AgentHeartbeat, type HeartbeatStatus } from '@/lib/agent-heartbeat';
import {
  connectMCPServers,
  disconnectAll as disconnectMCPServers,
  getMCPTools,
  type MCPManagerState,
} from '@/lib/mcp-manager';
import type { MessagePart } from '@/components/agent/ChatMessage';
import type { AgentActivity } from '@/types/agent-activity';
import type { Conversation } from '@/types/agent';
import type { FileAttachment } from '@/types/file-attachment';
import type { ServiceReport, ServiceResult, ServiceRunState } from '@/types/service';

// =============================================================================
// Types
// =============================================================================

export type AgentStatus = 'idle' | 'running' | 'awaiting_approval';

export interface ActiveServiceRun {
  reportId: string;
  lastResultCount: number;
}

interface ToolCallInfo {
  toolCallId: string;
  toolName: string;
  input: unknown;
}

/** A turn waiting for the technician to approve or deny tool calls */
interface PendingTurn {
  msgId: string;
  waiting: Map<string, ToolCallInfo>;
  results: ToolResultPart[];
}

interface AgentSessionValue {
  messages: Message[];
  status: AgentStatus;
  heartbeat: HeartbeatStatus;
  isConfigured: boolean;
  conversationId: string | null;
  activeServiceRun: ActiveServiceRun | null;
  mcpState: MCPManagerState;
  /** Send a user message (ignored unless idle) */
  send: (text: string, attachments?: FileAttachment[]) => void;
  approve: (toolCallId: string) => void;
  reject: (toolCallId: string) => void;
  /** Stop the running turn; pending approvals are cancelled */
  stop: () => void;
  /** Stop and start a fresh conversation */
  clear: () => void;
  loadConversation: (conversation: Conversation) => Promise<void>;
  /** Re-run the last turn after a stall */
  restart: () => void;
  dismissServiceRun: () => void;
}

const AgentSessionContext = createContext<AgentSessionValue | null>(null);

/** Auto-continue rounds per turn (each round is one streamText call) */
const MAX_ROUNDS = 25;

const EMPTY_MCP: MCPManagerState = { servers: [], toolCount: 0, isConnecting: false, errors: [] };

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function okResult(call: ToolCallInfo, value: unknown): ToolResultPart {
  return {
    type: 'tool-result',
    toolCallId: call.toolCallId,
    toolName: call.toolName,
    output: { type: 'json', value: (value ?? null) as never },
  };
}

function errorResult(call: ToolCallInfo, message: string): ToolResultPart {
  return {
    type: 'tool-result',
    toolCallId: call.toolCallId,
    toolName: call.toolName,
    output: { type: 'error-text', value: message },
  };
}

/** Concise text summary of a service result for the agent */
function formatServiceResult(result: ServiceResult): string {
  const lines = [
    `Status: ${result.success ? 'SUCCESS' : 'FAILED'}`,
    ...(result.error ? [`Error: ${result.error}`] : []),
    `Duration: ${((result.durationMs ?? 0) / 1000).toFixed(1)}s`,
    `Findings (${result.findings.length}):`,
  ];
  for (const f of result.findings.slice(0, 10)) {
    lines.push(`  [${f.severity.toUpperCase()}] ${f.title}: ${f.description}`);
    if (f.recommendation) lines.push(`    -> ${f.recommendation}`);
  }
  if (result.findings.length > 10) lines.push(`  ... and ${result.findings.length - 10} more findings`);
  return lines.join('\n');
}

// =============================================================================
// Provider
// =============================================================================

export function AgentSessionProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  // --- UI state (with refs so async turn code never reads stale values) ---
  const [messages, setMessagesState] = useState<Message[]>([]);
  const messagesRef = useRef<Message[]>([]);
  const setMessages = useCallback((update: Message[] | ((prev: Message[]) => Message[])) => {
    const next = typeof update === 'function' ? update(messagesRef.current) : update;
    messagesRef.current = next;
    setMessagesState(next);
  }, []);

  const [status, setStatusState] = useState<AgentStatus>('idle');
  const statusRef = useRef<AgentStatus>('idle');
  const setStatus = useCallback((s: AgentStatus) => {
    statusRef.current = s;
    setStatusState(s);
  }, []);

  const [heartbeat, setHeartbeat] = useState<HeartbeatStatus>('idle');
  const heartbeatRef = useRef<AgentHeartbeat | null>(null);
  if (!heartbeatRef.current) {
    heartbeatRef.current = new AgentHeartbeat();
    heartbeatRef.current.onStatusChange(setHeartbeat);
  }

  const [conversationId, setConversationIdState] = useState<string | null>(null);
  const conversationIdRef = useRef<string | null>(null);
  const setConversationId = useCallback((id: string | null) => {
    conversationIdRef.current = id;
    setConversationIdState(id);
  }, []);

  const [activeServiceRun, setActiveServiceRunState] = useState<ActiveServiceRun | null>(null);
  const activeRunRef = useRef<ActiveServiceRun | null>(null);
  const setActiveServiceRun = useCallback((run: ActiveServiceRun | null) => {
    activeRunRef.current = run;
    setActiveServiceRunState(run);
  }, []);

  const [mcpState, setMcpState] = useState<MCPManagerState>(EMPTY_MCP);

  // --- agent internals ---
  const historyRef = useRef<ModelMessage[]>([]);
  const pendingRef = useRef<PendingTurn | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  const serviceUpdatesRef = useRef<string[]>([]);
  const chainRef = useRef<Promise<void>>(Promise.resolve());
  const epochRef = useRef(0);

  /**
   * Run jobs one at a time. `stop()` drops jobs queued before it; a job that
   * awaits before starting a turn checks `alive()` so a Stop in between wins.
   */
  const dispatch = useCallback((job: (alive: () => boolean) => Promise<void>) => {
    const epoch = epochRef.current;
    const alive = () => epoch === epochRef.current;
    chainRef.current = chainRef.current
      .then(() => (alive() ? job(alive) : undefined))
      .catch(err => console.error('[Agent] Job failed:', err));
  }, []);

  // ---------------------------------------------------------------------------
  // Message helpers
  // ---------------------------------------------------------------------------

  const updateMessage = useCallback((msgId: string, fn: (m: Message) => Message) => {
    setMessages(prev => prev.map(m => (m.id === msgId ? fn(m) : m)));
  }, [setMessages]);

  const appendText = useCallback((msgId: string, text: string) => {
    updateMessage(msgId, m => {
      const parts = [...(m.parts ?? [])];
      const last = parts[parts.length - 1];
      if (last?.type === 'text') {
        parts[parts.length - 1] = { ...last, content: (last.content ?? '') + text };
      } else {
        parts.push({ type: 'text', content: text });
      }
      return { ...m, content: m.content + text, parts };
    });
  }, [updateMessage]);

  const updateActivity = useCallback((activityId: string, updates: Partial<AgentActivity>) => {
    setMessages(prev =>
      prev.map(m => {
        if (!m.parts?.some(p => p.activity?.id === activityId)) return m;
        return {
          ...m,
          parts: m.parts.map(p =>
            p.activity?.id === activityId
              ? { ...p, activity: { ...p.activity, ...updates } as AgentActivity }
              : p,
          ),
        };
      }),
    );
  }, [setMessages]);

  const addActivity = useCallback((msgId: string, call: ToolCallInfo, status: AgentActivity['status']) => {
    const { type, details } = describeToolCall(call.toolName, call.input);
    const part: MessagePart = {
      type: 'tool',
      activity: {
        id: call.toolCallId,
        timestamp: new Date().toISOString(),
        type,
        status,
        ...details,
      } as AgentActivity,
    };
    updateMessage(msgId, m => ({ ...m, parts: [...(m.parts ?? []), part] }));
  }, [updateMessage]);

  /** Mark still-running activities of a message as stopped */
  const markUnfinished = useCallback((msgId: string, reason: string) => {
    updateMessage(msgId, m => ({
      ...m,
      parts: m.parts?.map(p =>
        p.activity && (p.activity.status === 'running' || p.activity.status === 'pending_approval')
          ? { ...p, activity: { ...p.activity, status: 'error', error: reason } as AgentActivity }
          : p,
      ),
    }));
  }, [updateMessage]);

  // ---------------------------------------------------------------------------
  // Persistence
  // ---------------------------------------------------------------------------

  const persist = useCallback(async () => {
    const id = conversationIdRef.current;
    if (!id || historyRef.current.length === 0) return;
    try {
      await saveConversationHistory(id, historyRef.current);
    } catch (err) {
      console.error('[Agent] Failed to save conversation:', err);
    }
  }, []);

  // ---------------------------------------------------------------------------
  // Tool execution
  // ---------------------------------------------------------------------------

  const toolContext = useCallback((toolCallId: string): ToolRunContext => ({
    toolCallId,
    onServiceRunStarted: reportId => setActiveServiceRun({ reportId, lastResultCount: 0 }),
    onServiceRunCancelled: () => setActiveServiceRun(null),
  }), [setActiveServiceRun]);

  /** Run an approved (or auto-approved) call and record its result */
  const executeCall = useCallback(async (call: ToolCallInfo): Promise<ToolResultPart> => {
    updateActivity(call.toolCallId, { status: 'running' });
    try {
      const value = await runTool(call.toolName, call.input, toolContext(call.toolCallId));
      const extra: Partial<AgentActivity> = {};
      if (call.toolName === 'generate_file' && value && typeof value === 'object') {
        const v = value as { path?: string; size?: number; fileId?: string };
        Object.assign(extra, { path: v.path, size: v.size, fileId: v.fileId });
      }
      if (call.toolName === 'run_service_queue' && value && typeof value === 'object') {
        Object.assign(extra, { reportId: (value as { report_id?: string }).report_id });
      }
      updateActivity(call.toolCallId, { status: 'success', output: formatToolOutput(value), ...extra });
      return okResult(call, value);
    } catch (err) {
      const message = errorMessage(err);
      updateActivity(call.toolCallId, { status: 'error', error: message, output: message });
      return errorResult(call, message);
    }
  }, [toolContext, updateActivity]);

  // ---------------------------------------------------------------------------
  // Turn loop
  // ---------------------------------------------------------------------------

  type StreamOutcome =
    | { kind: 'aborted' }
    | { kind: 'done'; pending: ToolCallInfo[]; finishReason: string };

  /** One streamText call: render it, then append its messages to the history */
  const streamOnce = useCallback(async (msgId: string, signal: AbortSignal): Promise<StreamOutcome> => {
    const agent = settingsRef.current.agent;
    setExternalTools(getMCPTools());
    const tools = buildToolSet(agent);
    const result = await streamChat({
      messages: historyRef.current,
      settings: agent,
      tools,
      abortSignal: signal,
      maxSteps: activeRunRef.current ? 30 : 10,
    });

    const calls = new Map<string, ToolCallInfo>();
    const answered = new Set<string>();

    for await (const part of result.fullStream) {
      heartbeatRef.current!.ping();
      if (signal.aborted || part.type === 'abort') return { kind: 'aborted' };
      switch (part.type) {
        case 'text-delta':
          appendText(msgId, part.text);
          break;
        case 'tool-call': {
          const call = { toolCallId: part.toolCallId, toolName: part.toolName, input: part.input };
          calls.set(call.toolCallId, call);
          if (!part.providerExecuted) addActivity(msgId, call, 'running');
          break;
        }
        case 'tool-result':
          answered.add(part.toolCallId);
          updateActivity(part.toolCallId, { status: 'success', output: formatToolOutput(part.output) });
          break;
        case 'tool-error': {
          // Includes invalid calls (unknown tool, arguments failing the schema):
          // the SDK sends the error back to the model, so nothing waits on it.
          answered.add(part.toolCallId);
          const message = errorMessage(part.error);
          updateActivity(part.toolCallId, { status: 'error', error: message, output: message });
          break;
        }
        case 'error':
          throw part.error;
        default:
          break;
      }
    }
    if (signal.aborted) return { kind: 'aborted' };

    const response = await result.response;
    historyRef.current = [...historyRef.current, ...response.messages];

    const pending = [...calls.values()].filter(c => !answered.has(c.toolCallId));
    return { kind: 'done', pending, finishReason: await result.finishReason };
  }, [addActivity, appendText, updateActivity]);

  /** Deliver buffered service updates as a new turn (only when idle) */
  const deliverServiceUpdatesRef = useRef<() => void>(() => {});

  const runTurn = useCallback(async (reuseMsgId?: string) => {
    const controller = new AbortController();
    abortRef.current = controller;
    setStatus('running');
    heartbeatRef.current!.start();

    let msgId = reuseMsgId && messagesRef.current.some(m => m.id === reuseMsgId) ? reuseMsgId : '';
    if (!msgId) {
      msgId = generateId();
      setMessages(prev => [
        ...prev,
        { id: msgId, role: 'assistant', content: '', createdAt: new Date().toISOString(), parts: [] },
      ]);
    }

    let suspended = false;
    try {
      for (let round = 0; round < MAX_ROUNDS; round++) {
        const outcome = await streamOnce(msgId, controller.signal);
        if (outcome.kind === 'aborted') {
          markUnfinished(msgId, 'Stopped');
          appendText(msgId, '\n\n*[Stopped]*');
          break;
        }

        if (outcome.pending.length === 0) {
          // The step limit can end a round while the model is still working.
          if (outcome.finishReason === 'tool-calls') continue;
          break;
        }

        const mode = settingsRef.current.agent.approvalMode;
        const results: ToolResultPart[] = [];
        const waiting = new Map<string, ToolCallInfo>();
        for (const call of outcome.pending) {
          if (controller.signal.aborted) {
            results.push(errorResult(call, 'Stopped by the user.'));
            updateActivity(call.toolCallId, { status: 'error', error: 'Stopped' });
          } else if (await needsApproval(call.toolName, call.input, mode)) {
            waiting.set(call.toolCallId, call);
            updateActivity(call.toolCallId, { status: 'pending_approval' });
          } else {
            results.push(await executeCall(call));
          }
        }

        if (waiting.size > 0 && !controller.signal.aborted) {
          pendingRef.current = { msgId, waiting, results };
          suspended = true;
          setStatus('awaiting_approval');
          return;
        }

        for (const call of waiting.values()) {
          results.push(errorResult(call, 'Stopped by the user.'));
          updateActivity(call.toolCallId, { status: 'error', error: 'Stopped' });
        }
        historyRef.current = [...historyRef.current, { role: 'tool', content: results }];
        if (controller.signal.aborted) break;
      }
    } catch (err) {
      console.error('[Agent] Turn failed:', err);
      markUnfinished(msgId, errorMessage(err));
      appendText(msgId, `\n\n*[Error: ${errorMessage(err)}]*`);
    } finally {
      heartbeatRef.current!.stop();
      if (abortRef.current === controller) abortRef.current = null;
      if (!suspended) {
        // Drop the placeholder if the turn produced nothing at all.
        setMessages(prev => prev.filter(m => m.id !== msgId || (m.parts?.length ?? 0) > 0 || m.content));
        setStatus('idle');
      }
      await persist();
      if (!suspended) deliverServiceUpdatesRef.current();
    }
  }, [appendText, executeCall, markUnfinished, persist, setMessages, setStatus, streamOnce, updateActivity]);

  /** Approve or deny one call of the suspended turn; resume when all are decided */
  const resolveCall = useCallback(async (toolCallId: string, approved: boolean) => {
    const pending = pendingRef.current;
    const call = pending?.waiting.get(toolCallId);
    if (!pending || !call) return;
    pending.waiting.delete(toolCallId);

    if (approved) {
      pending.results.push(await executeCall(call));
    } else {
      updateActivity(toolCallId, { status: 'error', error: 'Denied', output: 'User denied this action.' });
      pending.results.push(errorResult(call, 'User denied this action.'));
    }

    if (pending.waiting.size === 0 && pendingRef.current === pending) {
      pendingRef.current = null;
      historyRef.current = [...historyRef.current, { role: 'tool', content: pending.results }];
      await runTurn(pending.msgId);
    }
  }, [executeCall, runTurn, updateActivity]);

  // ---------------------------------------------------------------------------
  // Service supervision
  // ---------------------------------------------------------------------------

  deliverServiceUpdatesRef.current = () => {
    if (statusRef.current !== 'idle' || serviceUpdatesRef.current.length === 0) return;
    const updates = serviceUpdatesRef.current.splice(0);
    const content = updates.join('\n\n---\n\n');
    historyRef.current = [...historyRef.current, { role: 'user', content }];
    setMessages(prev => [
      ...prev,
      { id: generateId(), role: 'system', content: content.split('\n')[0], createdAt: new Date().toISOString() },
    ]);
    setStatus('running');
    dispatch(() => runTurn());
  };

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const add = async <T,>(event: string, handler: (payload: T) => void) => {
      const un = await listen<T>(event, e => handler(e.payload));
      if (disposed) un();
      else unlisteners.push(un);
    };

    add<ServiceRunState>('service-state-changed', state => {
      const run = activeRunRef.current;
      const report = state.currentReport;
      if (!run || report?.id !== run.reportId) return;
      if (report.results.length <= run.lastResultCount) return;
      const fresh = report.results.slice(run.lastResultCount);
      setActiveServiceRun({ ...run, lastResultCount: report.results.length });
      serviceUpdatesRef.current.push(
        `${SERVICE_UPDATE_PREFIX} UPDATE — ${fresh.length} service(s) completed]\n\n` +
          fresh.map(r => `${r.serviceId} completed:\n${formatServiceResult(r)}`).join('\n\n'),
      );
      deliverServiceUpdatesRef.current();
    });

    add<ServiceReport>('service-completed', report => {
      const run = activeRunRef.current;
      if (!run || report.id !== run.reportId) return;
      setActiveServiceRun(null);
      const duration = report.totalDurationMs ? `${(report.totalDurationMs / 1000).toFixed(1)}s` : 'unknown';
      serviceUpdatesRef.current.push(
        `${SERVICE_UPDATE_PREFIX} RUN COMPLETE] Report ID: ${report.id}\nStatus: ${report.status}\n` +
          `Services run: ${report.results.length}\nDuration: ${duration}\n\n` +
          'All services have finished. Review the results with get_service_report and get_report_statistics, ' +
          'then write the analysis, summary and health score.',
      );
      deliverServiceUpdatesRef.current();
    });

    return () => {
      disposed = true;
      unlisteners.forEach(un => un());
    };
  }, [setActiveServiceRun]);

  // ---------------------------------------------------------------------------
  // MCP client connections
  // ---------------------------------------------------------------------------

  const mcpServers = settings.agent?.mcpServers;
  const mcpKey = JSON.stringify((mcpServers ?? []).map(s => [s.id, s.url, s.enabled, s.apiKey, s.transportType]));
  useEffect(() => {
    let cancelled = false;
    const servers = mcpServers ?? [];
    if (servers.some(s => s.enabled && s.url)) {
      setMcpState(prev => ({ ...prev, isConnecting: true }));
      connectMCPServers(servers)
        .then(({ state }) => !cancelled && setMcpState(state))
        .catch(err => {
          console.error('[MCP] Connection error:', err);
          if (!cancelled) setMcpState(prev => ({ ...prev, isConnecting: false }));
        });
    } else {
      disconnectMCPServers().then(() => !cancelled && setMcpState(EMPTY_MCP));
    }
    return () => {
      cancelled = true;
      disconnectMCPServers();
    };
    // Reconnect only when the server configuration actually changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mcpKey]);

  // ---------------------------------------------------------------------------
  // Public actions
  // ---------------------------------------------------------------------------

  const send = useCallback((text: string, attachments?: FileAttachment[]) => {
    if (statusRef.current !== 'idle' || (!text.trim() && !attachments?.length)) return;
    if (!isAgentConfigured(settingsRef.current.agent)) return;
    setStatus('running');

    let modelText = text;
    if (attachments?.length) {
      modelText += '\n\n' + attachments
        .map(att => {
          let ctx = `[File: ${att.originalName} (${att.category}, ${att.size} bytes)]`;
          if (att.content) {
            ctx += `\nContent:\n${att.content.substring(0, 2000)}${att.content.length > 2000 ? '\n... (truncated)' : ''}`;
          }
          return ctx;
        })
        .join('\n\n');
    }

    setMessages(prev => [
      ...prev,
      { id: generateId(), role: 'user', content: text, createdAt: new Date().toISOString(), attachments },
    ]);
    historyRef.current = [...historyRef.current, { role: 'user', content: modelText }];

    dispatch(async alive => {
      if (!conversationIdRef.current) {
        try {
          const title = text.length > 50 ? `${text.substring(0, 50)}...` : text || 'New Chat';
          const conv = await commands.createConversation(title);
          setConversationId(conv.id);
        } catch (err) {
          console.error('[Agent] Failed to create conversation:', err);
        }
      }
      if (alive()) await runTurn();
    });
  }, [dispatch, runTurn, setConversationId, setMessages, setStatus]);

  const approve = useCallback((id: string) => {
    if (statusRef.current !== 'awaiting_approval') return;
    dispatch(() => resolveCall(id, true));
  }, [dispatch, resolveCall]);

  const reject = useCallback((id: string) => {
    if (statusRef.current !== 'awaiting_approval') return;
    dispatch(() => resolveCall(id, false));
  }, [dispatch, resolveCall]);

  const stop = useCallback(() => {
    epochRef.current++;
    abortRef.current?.abort();
    serviceUpdatesRef.current = [];

    const pending = pendingRef.current;
    if (pending) {
      pendingRef.current = null;
      for (const call of pending.waiting.values()) {
        pending.results.push(errorResult(call, 'Stopped by the user before approval.'));
      }
      historyRef.current = [...historyRef.current, { role: 'tool', content: pending.results }];
      markUnfinished(pending.msgId, 'Stopped');
      setStatus('idle');
      persist();
    } else if (!abortRef.current) {
      // A turn was queued but had not started yet; it has just been dropped.
      setStatus('idle');
    }
    heartbeatRef.current!.stop();
  }, [markUnfinished, persist, setStatus]);

  const clear = useCallback(() => {
    stop();
    // Wait for the aborted turn to unwind before resetting.
    dispatch(async () => {
      historyRef.current = [];
      pendingRef.current = null;
      setMessages([]);
      setActiveServiceRun(null);
      setConversationId(null);
      setStatus('idle');
    });
  }, [dispatch, setActiveServiceRun, setConversationId, setMessages, setStatus, stop]);

  const loadConversation = useCallback(async (conversation: Conversation) => {
    stop();
    const data = await loadConversationHistory(conversation.id);
    dispatch(async () => {
      historyRef.current = data.history;
      pendingRef.current = null;
      setMessages(data.messages);
      setActiveServiceRun(null);
      setConversationId(conversation.id);
      setStatus('idle');
    });
  }, [dispatch, setActiveServiceRun, setConversationId, setMessages, setStatus, stop]);

  const restart = useCallback(() => {
    stop();
    const last = historyRef.current[historyRef.current.length - 1];
    if (!last || last.role === 'assistant') return;
    setStatus('running');
    dispatch(() => runTurn());
  }, [dispatch, runTurn, setStatus, stop]);

  const dismissServiceRun = useCallback(() => setActiveServiceRun(null), [setActiveServiceRun]);

  const isConfigured = isAgentConfigured(settings.agent);

  const value = useMemo<AgentSessionValue>(() => ({
    messages,
    status,
    heartbeat,
    isConfigured,
    conversationId,
    activeServiceRun,
    mcpState,
    send,
    approve,
    reject,
    stop,
    clear,
    loadConversation,
    restart,
    dismissServiceRun,
  }), [messages, status, heartbeat, isConfigured, conversationId, activeServiceRun, mcpState,
    send, approve, reject, stop, clear, loadConversation, restart, dismissServiceRun]);

  return <AgentSessionContext.Provider value={value}>{children}</AgentSessionContext.Provider>;
}

export function useAgentSession(): AgentSessionValue {
  const ctx = useContext(AgentSessionContext);
  if (!ctx) throw new Error('useAgentSession must be used within an AgentSessionProvider');
  return ctx;
}
