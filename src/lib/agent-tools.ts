/**
 * Agent Tools
 *
 * The single registry of everything the agent can do. Each tool is defined
 * once here — its schema, how it is approved, how it is shown in the chat and
 * sidebar, and how it runs — and every other part of the agent reads from it.
 *
 * Approval:
 * - `auto`: read-only or report-editing tools; the SDK executes them directly.
 * - `always`: needs the technician's approval unless the mode is YOLO.
 * - `command`: like `always`, but whitelisted commands run automatically in
 *   whitelist mode (checked by the backend, which rejects chained commands).
 *
 * Approval tools have no `execute` in the SDK tool set; the agent session runs
 * them through `runTool` once they are approved.
 */

import { tool, type ToolSet } from 'ai';
import { z } from 'zod';
import { commands } from '@/bindings';
import type { AgentSettings, ApprovalMode } from '@/types/agent';
import type { ActivityType } from '@/types/agent-activity';

// =============================================================================
// Types
// =============================================================================

export type ToolApproval = 'auto' | 'always' | 'command';

/** Context passed to tools that need to report back to the agent session. */
export interface ToolRunContext {
  toolCallId: string;
  /** Called when a service run the agent started has begun. */
  onServiceRunStarted?: (reportId: string) => void;
  /** Called when the agent cancels the service run it is supervising. */
  onServiceRunCancelled?: () => void;
}

export interface AgentToolDef {
  name: string;
  /** Short label for the sidebar and activity cards */
  label: string;
  /** One-line description for the sidebar */
  summary: string;
  /** Description sent to the model */
  description: string;
  input: z.ZodTypeAny;
  approval: ToolApproval;
  activity: ActivityType;
  /** Extra fields shown on the activity card */
  details?: (args: Record<string, unknown>) => Record<string, unknown>;
  /** Run the tool. Throw to report an error to the model. */
  run: (args: never, ctx: ToolRunContext) => Promise<unknown>;
  /** Whether the tool is offered with these settings (default: always) */
  enabled?: (settings: AgentSettings) => boolean;
}

/** Define a tool with typed arguments (the registry stores them erased). */
function defineTool<S extends z.ZodTypeAny>(def: Omit<AgentToolDef, 'input' | 'run' | 'details'> & {
  input: S;
  run: (args: z.infer<S>, ctx: ToolRunContext) => Promise<unknown>;
  details?: (args: z.infer<S>) => Record<string, unknown>;
}): AgentToolDef {
  return def as unknown as AgentToolDef;
}

// =============================================================================
// Shared schemas & helpers
// =============================================================================

const windowsPath = z
  .string()
  .trim()
  .regex(/^[a-zA-Z]:[\\/]|^\\\\/, 'Must be an absolute Windows path, e.g. C:\\Users\\Public\\file.txt');

const srcDest = z.object({
  src: windowsPath.describe('Source path'),
  dest: windowsPath.describe('Destination path'),
});

const reason = (what: string) => z.object({ reason: z.string().describe(what) });

const severity = z.enum(['info', 'success', 'warning', 'error', 'critical']);

const fileName = (p: unknown) => (typeof p === 'string' ? p.split(/[/\\]/).pop() || '' : '');

function compact(value: string, max = 120): string {
  const flat = value.replace(/\s+/g, ' ').trim();
  return flat.length > max ? `${flat.slice(0, max - 3)}...` : flat;
}

/** Shape a command result for the model. Non-zero exits are reported, not thrown. */
function commandOutput(res: { status: string; output?: string | null; error?: string | null }) {
  return {
    status: res.status === 'executed' ? 'success' : 'failed',
    output: res.output ?? '',
    ...(res.error ? { stderr: res.error } : {}),
  };
}

// =============================================================================
// Tools
// =============================================================================

export const AGENT_TOOLS: AgentToolDef[] = [
  // --------------------------------------------------------------- commands
  defineTool({
    name: 'execute_command',
    label: 'Commands',
    summary: 'Execute PowerShell',
    activity: 'ran_command',
    approval: 'command',
    description: `Execute a PowerShell command on the Windows system. The user approves it before it runs (unless the approval mode allows it).

IMPORTANT POWERSHELL RULES:
- Use full cmdlet names: Get-ChildItem (not ls/dir), Get-Process (not ps), Select-Object (not select)
- Chain with semicolons (;) not &&
- Pipe to Format-Table -AutoSize or Select-Object for readable output
- Quote paths with spaces: "$env:USERPROFILE\\Downloads"
- Use -ErrorAction SilentlyContinue when checking things that may not exist
- Prefer structured output over raw strings`,
    input: z.object({
      command: z.string().trim().min(1).describe('The PowerShell command to execute'),
      reason: z.string().describe('Brief explanation of what this command does and why'),
    }),
    details: a => ({ command: a.command }),
    run: async a => commandOutput(await commands.executeAgentCommand(a.command, a.reason || 'Agent approved')),
  }),
  defineTool({
    name: 'run_instrument',
    label: 'Run Instrument',
    summary: 'Execute a custom script',
    activity: 'ran_command',
    approval: 'always',
    description: 'Run a custom instrument (script from data/instruments) by name. The user approves it first.',
    input: z.object({
      name: z.string().min(1).describe('Name of the instrument (see list_instruments)'),
      args: z.string().optional().describe('Arguments to pass'),
    }),
    details: a => ({ command: `Instrument: ${a.name}${a.args ? ` ${a.args}` : ''}` }),
    run: async a => commandOutput(await commands.runAgentInstrument(a.name, a.args ?? null)),
  }),

  // ------------------------------------------------------------ filesystem
  defineTool({
    name: 'read_file',
    label: 'Read File',
    summary: 'Open file contents',
    activity: 'read_file',
    approval: 'auto',
    description: 'Read a text file with optional line numbers and pagination. Use for logs, configs, scripts, or any text file.',
    input: z.object({
      path: z.string().min(1).describe('Full absolute path to the file'),
      offset: z.number().int().min(0).optional().describe('Line to start from (0-indexed)'),
      limit: z.number().int().min(1).optional().describe('Maximum number of lines to read'),
      lineNumbers: z.boolean().optional().describe('Include line numbers (default: true)'),
    }),
    details: a => ({ path: a.path, filename: fileName(a.path) }),
    run: a => commands.agentReadFile(a.path, a.offset ?? null, a.limit ?? null, a.lineNumbers ?? true),
  }),
  defineTool({
    name: 'grep',
    label: 'Grep',
    summary: 'Search text across files',
    activity: 'searched',
    approval: 'auto',
    description: 'Search for a regex pattern across files in a directory. Returns matching lines with file paths and line numbers.',
    input: z.object({
      pattern: z.string().min(1).describe('Regex pattern to search for'),
      path: z.string().optional().describe('Directory to search (default: working directory)'),
      filePattern: z.string().optional().describe('Glob filter for file names, e.g. "*.log"'),
      maxResults: z.number().int().min(1).optional().describe('Maximum results (default: 50)'),
    }),
    details: a => ({ query: a.pattern }),
    run: async a => {
      const results = await commands.agentGrep(a.pattern, a.path ?? null, a.filePattern ?? null, a.maxResults ?? 50);
      return { results, count: results.length };
    },
  }),
  defineTool({
    name: 'glob',
    label: 'Glob',
    summary: 'Find files by pattern',
    activity: 'searched',
    approval: 'auto',
    description: 'Find files matching a glob pattern (relative to path), newest first, e.g. "*.log" or "**/*.txt".',
    input: z.object({
      pattern: z.string().min(1).describe('Glob pattern relative to path'),
      path: z.string().optional().describe('Base directory (default: working directory)'),
      limit: z.number().int().min(1).optional().describe('Maximum results (default: 100)'),
    }),
    details: a => ({ query: a.pattern }),
    run: async a => {
      const files = await commands.agentGlob(a.pattern, a.path ?? null, a.limit ?? 100);
      return { files, count: files.length };
    },
  }),
  defineTool({
    name: 'list_dir',
    label: 'List Directory',
    summary: 'List folder contents',
    activity: 'list_dir',
    approval: 'auto',
    description: 'List files and directories at a path with name, type, and size.',
    input: z.object({
      path: z.string().min(1).describe('Absolute path to list, e.g. C:\\Users\\<user>'),
    }),
    details: a => ({ path: a.path }),
    run: async a => {
      const entries = await commands.agentListDir(a.path);
      return { entries: entries.map(e => ({ name: e.name, path: e.path, type: e.isDir ? 'dir' : 'file', size: e.size })) };
    },
  }),
  defineTool({
    name: 'write_file',
    label: 'Write File',
    summary: 'Create or overwrite files',
    activity: 'write_file',
    approval: 'always',
    description: 'Write content to a file. Creates it if missing, overwrites it otherwise. The user approves first.',
    input: z.object({
      path: windowsPath.describe('Full absolute path to the file'),
      content: z.string().describe('Content to write to the file'),
    }),
    details: a => ({ path: a.path, filename: fileName(a.path), content: a.content }),
    run: async a => {
      await commands.agentWriteFile(a.path, a.content);
      return { status: 'success', path: a.path };
    },
  }),
  defineTool({
    name: 'edit_file',
    label: 'Edit File',
    summary: 'Replace text in files',
    activity: 'edit_file',
    approval: 'always',
    description: `Replace oldString with newString in a file. oldString must be unique in the file unless all=true.
Use this for targeted edits instead of rewriting entire files. Read the file first to get the exact text. newString may be empty to delete text.`,
    input: z.object({
      path: windowsPath.describe('Full absolute path to the file'),
      oldString: z.string().min(1).describe('The exact text to replace'),
      newString: z.string().describe('The replacement text (may be empty)'),
      all: z.boolean().optional().describe('Replace all occurrences (default: false)'),
    }),
    details: a => ({ path: a.path, filename: fileName(a.path), oldString: a.oldString, newString: a.newString, all: a.all }),
    run: async a => {
      const res = await commands.agentEditFile(a.path, a.oldString, a.newString, a.all ?? false);
      if (res.status !== 'success') throw new Error(res.message);
      return res;
    },
  }),
  defineTool({
    name: 'move_file',
    label: 'Move File',
    summary: 'Move or rename files',
    activity: 'move_file',
    approval: 'always',
    description: 'Move or rename a file. The user approves first.',
    input: srcDest,
    details: a => ({ src: a.src, dest: a.dest }),
    run: async a => {
      await commands.agentMoveFile(a.src, a.dest);
      return { status: 'success', message: `Moved ${a.src} to ${a.dest}` };
    },
  }),
  defineTool({
    name: 'copy_file',
    label: 'Copy File',
    summary: 'Copy files',
    activity: 'copy_file',
    approval: 'always',
    description: 'Copy a file. The user approves first.',
    input: srcDest,
    details: a => ({ src: a.src, dest: a.dest }),
    run: async a => {
      await commands.agentCopyFile(a.src, a.dest);
      return { status: 'success', message: `Copied ${a.src} to ${a.dest}` };
    },
  }),
  defineTool({
    name: 'generate_file',
    label: 'Generate File',
    summary: 'Create downloadable files',
    activity: 'generate_file',
    approval: 'always',
    description: 'Create a file (report, log, script, config) in the agent workspace that the user can open from the chat.',
    input: z.object({
      filename: z.string().min(1).describe('File name including extension'),
      content: z.string().describe('Full content of the file'),
      description: z.string().min(1).describe('Brief description of the file'),
      mimeType: z.string().optional().describe('MIME type (optional, detected from the extension)'),
    }),
    details: a => ({ filename: a.filename, description: a.description, content: a.content }),
    run: async (a, ctx) => {
      const file = await commands.generateAgentFile(a.filename, a.content, a.description, a.mimeType ?? null, ctx.toolCallId, true);
      return { status: 'success', fileId: file.id, path: file.storedPath, size: file.size };
    },
  }),

  // ------------------------------------------------------ programs & system
  defineTool({
    name: 'list_programs',
    label: 'Programs',
    summary: 'List portable tools',
    activity: 'list_programs',
    approval: 'auto',
    description: 'Overview of all portable programs in data/programs. To locate one tool, prefer find_exe.',
    input: z.object({}),
    run: async () => ({ programs: await commands.listAgentPrograms() }),
  }),
  defineTool({
    name: 'find_exe',
    label: 'Find Executable',
    summary: 'Locate a CLI tool',
    activity: 'find_exe',
    approval: 'auto',
    description: `Locate a specific executable in data/programs (recursively), e.g. find_exe("smartctl").
Returns absolute paths; empty means not installed. Set searchPath=true to also check the system PATH.`,
    input: z.object({
      query: z.string().min(1).describe('Executable name or keyword'),
      searchPath: z.boolean().optional().describe('Also check the system PATH (default: false)'),
    }),
    details: a => ({ query: a.query }),
    run: async a => {
      const matches = await commands.agentFindExe(a.query, a.searchPath ?? false);
      return { matches, found: matches.length > 0 };
    },
  }),
  defineTool({
    name: 'list_instruments',
    label: 'Instruments',
    summary: 'List available scripts',
    activity: 'list_dir',
    approval: 'auto',
    description: 'List the custom instruments (scripts) that run_instrument can run.',
    input: z.object({}),
    details: () => ({ path: 'data/instruments' }),
    run: async () => {
      const instruments = await commands.listInstruments();
      return { instruments: instruments.map(i => ({ name: i.name, description: i.description, extension: i.extension })) };
    },
  }),
  defineTool({
    name: 'get_system_info',
    label: 'System Info',
    summary: 'Hardware & OS details',
    activity: 'get_system_info',
    approval: 'auto',
    description: `Get system information (OS, CPU, RAM, disks, GPU, network). Request only the sections you need.
Examples: sections=["disk"] for disk space, ["memory"] for RAM, ["os","cpu"] for a hardware overview.`,
    input: z.object({
      sections: z.array(z.enum(['os', 'cpu', 'memory', 'disk', 'network'])).optional()
        .describe('Sections to retrieve. Omit for all.'),
    }),
    run: async a => {
      const info = await commands.getSystemInfo();
      if (!a.sections?.length) return { info };
      const keys = { os: 'os', cpu: 'cpu', memory: 'memory', disk: 'disks', network: 'networks' } as const;
      return { info: Object.fromEntries(a.sections.map(s => [keys[s], info[keys[s]]])) };
    },
  }),
  defineTool({
    name: 'search_web',
    label: 'Web Search',
    summary: 'Search the internet',
    activity: 'web_search',
    approval: 'auto',
    description: 'Search the web for documentation, error solutions, or guides. Be specific and include error codes.',
    input: z.object({ query: z.string().min(1).describe('The search query') }),
    details: a => ({ query: a.query }),
    enabled: s => s.searchProvider !== 'none',
    run: async a => {
      const results = await commands.agentSearchWeb(a.query);
      return { results: results.map(r => ({ title: r.title, url: r.url, snippet: r.snippet })) };
    },
  }),

  // ------------------------------------------------------- service queries
  defineTool({
    name: 'list_services',
    label: 'Services',
    summary: 'List diagnostic services',
    activity: 'service_query',
    approval: 'auto',
    description: 'List all diagnostic and maintenance services with id, name, description, category, duration, required programs and options.',
    input: z.object({}),
    details: () => ({ queryType: 'List services' }),
    run: async () => ({
      services: (await commands.getServiceDefinitions()).map(d => ({
        id: d.id, name: d.name, description: d.description, category: d.category,
        estimatedDurationSecs: d.estimatedDurationSecs, requiredPrograms: d.requiredPrograms,
        options: d.options.map(o => ({ id: o.id, label: o.label, type: o.optionType, default: o.defaultValue })),
      })),
    }),
  }),
  defineTool({
    name: 'list_service_presets',
    label: 'Service Presets',
    summary: 'List service bundles',
    activity: 'service_query',
    approval: 'auto',
    description: 'List service presets (Diagnostics, General, Complete, Custom and user presets) with their services.',
    input: z.object({}),
    details: () => ({ queryType: 'List presets' }),
    run: async () => ({
      presets: (await commands.getServicePresets()).map(p => ({
        id: p.id, name: p.name, description: p.description,
        services: p.services.map(s => ({ serviceId: s.serviceId, enabled: s.enabled, options: s.options })),
      })),
    }),
  }),
  defineTool({
    name: 'check_service_requirements',
    label: 'Check Requirements',
    summary: 'Verify required programs',
    activity: 'service_query',
    approval: 'auto',
    description: 'Check whether the external programs required by the given services are installed.',
    input: z.object({ service_ids: z.array(z.string()).min(1).describe('Service IDs to check') }),
    details: a => ({ queryType: 'Check requirements', detail: a.service_ids.join(', ') }),
    run: async a => {
      const missing = await commands.validateServiceRequirements(a.service_ids);
      return { allRequirementsMet: Object.keys(missing).length === 0, missingPrograms: missing };
    },
  }),
  defineTool({
    name: 'get_service_status',
    label: 'Service Status',
    summary: 'Current run progress',
    activity: 'service_query',
    approval: 'auto',
    description: 'Get the current service run state: whether a run is active or paused, and its progress.',
    input: z.object({}),
    details: () => ({ queryType: 'Service status' }),
    run: async () => {
      const state = await commands.getServiceRunState();
      const report = state.currentReport;
      const enabled = report?.queue.filter(q => q.enabled) ?? [];
      return {
        isRunning: state.isRunning,
        isPaused: state.isPaused,
        reportId: report?.id,
        totalServices: enabled.length,
        completedResults: report?.results.length ?? 0,
        currentServiceId: report?.currentServiceIndex != null ? enabled[report.currentServiceIndex]?.serviceId : undefined,
        reportStatus: report?.status,
      };
    },
  }),

  // -------------------------------------------------------- service reports
  defineTool({
    name: 'get_service_report',
    label: 'Service Report',
    summary: 'Read a saved report',
    activity: 'service_report',
    approval: 'auto',
    description: 'Get a service report by id with all results and findings. Each result has a queueItemId to use with the editing tools when a service ran more than once.',
    input: z.object({ report_id: z.string().min(1).describe('The report ID') }),
    details: a => ({ reportAction: 'Get report', reportId: a.report_id }),
    run: async a => {
      const r = await commands.getServiceReport(a.report_id);
      return {
        id: r.id, startedAt: r.startedAt, completedAt: r.completedAt, status: r.status,
        totalDurationMs: r.totalDurationMs, agentSummary: r.agentSummary, healthScore: r.healthScore,
        results: r.results.map(res => ({
          serviceId: res.serviceId, queueItemId: res.queueItemId, success: res.success, error: res.error,
          durationMs: res.durationMs, agentAnalysis: res.agentAnalysis,
          findings: res.findings.map(f => ({ severity: f.severity, title: f.title, description: f.description, recommendation: f.recommendation })),
        })),
      };
    },
  }),
  defineTool({
    name: 'get_report_statistics',
    label: 'Report Statistics',
    summary: 'Pass/fail and severity counts',
    activity: 'service_report',
    approval: 'auto',
    description: 'Get statistics for a report: pass/fail counts, severity breakdown, computed health score, durations.',
    input: z.object({ report_id: z.string().min(1).describe('The report ID') }),
    details: a => ({ reportAction: 'Get statistics', reportId: a.report_id }),
    run: async a => ({ statistics: await commands.getReportStatistics(a.report_id) }),
  }),
  defineTool({
    name: 'edit_finding',
    label: 'Edit Finding',
    summary: 'Change a report finding',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Edit a finding in a saved report (severity, title, description or recommendation).',
    input: z.object({
      report_id: z.string().min(1),
      service_id: z.string().min(1),
      queue_item_id: z.string().optional().describe('Disambiguates when the service ran more than once'),
      finding_index: z.number().int().min(0).describe('Zero-based index of the finding'),
      severity: severity.optional(),
      title: z.string().optional(),
      description: z.string().optional(),
      recommendation: z.string().optional(),
    }),
    details: a => ({ editAction: 'Edit finding', detail: a.title }),
    run: async a => {
      await commands.editReportFinding(a.report_id, a.service_id, a.finding_index, a.severity ?? null,
        a.title ?? null, a.description ?? null, a.recommendation ?? null, a.queue_item_id ?? null);
      return { status: 'success', message: 'Finding updated' };
    },
  }),
  defineTool({
    name: 'add_finding',
    label: 'Add Finding',
    summary: 'Add a report finding',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Add a finding to a service result in a report.',
    input: z.object({
      report_id: z.string().min(1),
      service_id: z.string().min(1),
      queue_item_id: z.string().optional().describe('Disambiguates when the service ran more than once'),
      severity,
      title: z.string().min(1),
      description: z.string(),
      recommendation: z.string().optional(),
    }),
    details: a => ({ editAction: 'Add finding', detail: a.title }),
    run: async a => {
      await commands.addReportFinding(a.report_id, a.service_id, a.severity, a.title, a.description,
        a.recommendation ?? null, a.queue_item_id ?? null);
      return { status: 'success', message: 'Finding added' };
    },
  }),
  defineTool({
    name: 'remove_finding',
    label: 'Remove Finding',
    summary: 'Delete a report finding',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Remove a finding from a service result by index. Use sparingly.',
    input: z.object({
      report_id: z.string().min(1),
      service_id: z.string().min(1),
      queue_item_id: z.string().optional(),
      finding_index: z.number().int().min(0),
    }),
    details: a => ({ editAction: 'Remove finding', detail: `#${a.finding_index}` }),
    run: async a => {
      await commands.removeReportFinding(a.report_id, a.service_id, a.finding_index, a.queue_item_id ?? null);
      return { status: 'success', message: 'Finding removed' };
    },
  }),
  defineTool({
    name: 'set_report_summary',
    label: 'Report Summary',
    summary: 'Write the executive summary',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Set the executive summary of a report. Write it professionally for the customer.',
    input: z.object({ report_id: z.string().min(1), summary: z.string().min(1) }),
    details: () => ({ editAction: 'Set summary' }),
    run: async a => {
      await commands.setReportSummary(a.report_id, a.summary);
      return { status: 'success', message: 'Summary set' };
    },
  }),
  defineTool({
    name: 'set_service_analysis',
    label: 'Service Analysis',
    summary: 'Annotate a service result',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Set analysis text for one service result in a report.',
    input: z.object({
      report_id: z.string().min(1),
      service_id: z.string().min(1),
      queue_item_id: z.string().optional(),
      analysis: z.string().min(1),
    }),
    details: a => ({ editAction: 'Set analysis', detail: a.service_id }),
    run: async a => {
      await commands.setServiceAnalysis(a.report_id, a.service_id, a.analysis, a.queue_item_id ?? null);
      return { status: 'success', message: 'Analysis set' };
    },
  }),
  defineTool({
    name: 'set_health_score',
    label: 'Health Score',
    summary: 'Rate the machine 0-100',
    activity: 'service_edit',
    approval: 'auto',
    description: 'Set the overall health score (0-100) of a report. 90-100 excellent, 70-89 good, 50-69 fair, 30-49 poor, 0-29 critical.',
    input: z.object({ report_id: z.string().min(1), score: z.number().min(0).max(100) }),
    details: a => ({ editAction: 'Set health score', detail: `Score: ${Math.round(a.score)}` }),
    run: async a => {
      const score = Math.round(a.score);
      await commands.setReportHealthScore(a.report_id, score);
      return { status: 'success', message: `Health score set to ${score}` };
    },
  }),
  defineTool({
    name: 'export_report',
    label: 'Export Report',
    summary: 'Save a text copy of a report',
    activity: 'service_pdf',
    approval: 'auto',
    description: 'Export a report as a plain-text file in data/reports and return its path. (Printable PDFs are made from the Reports page.)',
    input: z.object({
      report_id: z.string().min(1),
      file_name: z.string().optional().describe('Optional file name without folders'),
    }),
    details: a => ({ reportId: a.report_id }),
    run: async a => {
      const path = await commands.exportReportText(a.report_id, a.file_name ?? null);
      return { status: 'success', path };
    },
  }),

  // --------------------------------------------------------- service control
  defineTool({
    name: 'run_service_queue',
    label: 'Run Services',
    summary: 'Start a service run',
    activity: 'service_queue_started',
    approval: 'always',
    description: `Start a service run (the user approves first). Build the queue by:
1. list_services to see what is available
2. check_service_requirements to verify required programs
3. a queue of service ids with enabled flags, order and options
You will receive an update as each service completes, and a final summary.`,
    input: z.object({
      queue: z.array(z.object({
        service_id: z.string(),
        enabled: z.boolean().default(true),
        order: z.number().int(),
        options: z.looseObject({}).default({}),
      })).min(1).describe('Service queue items'),
      technician_name: z.string().optional().describe('Technician name (business mode)'),
      customer_name: z.string().optional().describe('Customer name (business mode)'),
      reason: z.string().describe('Why these services are being run'),
    }),
    details: a => ({
      serviceCount: a.queue.filter(q => q.enabled !== false).length,
      services: a.queue.filter(q => q.enabled !== false).map(q => ({ serviceId: q.service_id, options: q.options })),
      reason: a.reason,
    }),
    run: async (a, ctx) => {
      const queue = a.queue.map(q => ({
        id: crypto.randomUUID(),
        serviceId: q.service_id,
        enabled: q.enabled,
        order: q.order,
        options: q.options ?? {},
      }));
      const report = await commands.runServices(queue, a.technician_name ?? null, a.customer_name ?? null, null, true);
      ctx.onServiceRunStarted?.(report.id);
      return {
        status: 'started',
        report_id: report.id,
        message: `Service run started with ${queue.filter(q => q.enabled).length} services. You will receive updates as they complete.`,
      };
    },
  }),
  defineTool({
    name: 'pause_service',
    label: 'Pause Services',
    summary: 'Pause the service run',
    activity: 'service_paused',
    approval: 'always',
    description: 'Pause the running service queue (takes effect between services).',
    input: reason('Why you are pausing the run'),
    details: a => ({ reason: a.reason }),
    run: async () => {
      await commands.pauseServiceRun();
      return { status: 'success', message: 'Service run paused' };
    },
  }),
  defineTool({
    name: 'resume_service',
    label: 'Resume Services',
    summary: 'Resume the service run',
    activity: 'service_resumed',
    approval: 'always',
    description: 'Resume a paused service run.',
    input: reason('Why it is safe to resume'),
    details: a => ({ reason: a.reason }),
    run: async () => {
      await commands.resumeServiceRun();
      return { status: 'success', message: 'Service run resumed' };
    },
  }),
  defineTool({
    name: 'cancel_service',
    label: 'Cancel Services',
    summary: 'Cancel the service run',
    activity: 'service_cancelled',
    approval: 'always',
    description: 'Cancel the running service queue (also stops the service currently running).',
    input: reason('Why you are cancelling the run'),
    details: a => ({ reason: a.reason }),
    run: async (_a, ctx) => {
      await commands.cancelServiceRun();
      ctx.onServiceRunCancelled?.();
      return { status: 'success', message: 'Service run cancelled' };
    },
  }),
];

const TOOLS_BY_NAME = new Map(AGENT_TOOLS.map(t => [t.name, t]));

// =============================================================================
// External (MCP) tools
// =============================================================================

/**
 * Tools from connected MCP servers. They can do anything the server allows,
 * so — like the built-in approval tools — they need approval unless the mode
 * is YOLO; the SDK never executes them directly.
 */
let externalTools: ToolSet = {};

/** Register the tools of the currently connected MCP servers */
export function setExternalTools(tools: ToolSet) {
  externalTools = tools;
}

function isExternalTool(name: string): boolean {
  return !TOOLS_BY_NAME.has(name) && Object.prototype.hasOwnProperty.call(externalTools, name);
}

// =============================================================================
// Registry helpers
// =============================================================================

export function getToolDef(name: string): AgentToolDef | undefined {
  return TOOLS_BY_NAME.get(name);
}

/** Tools offered to the model with the given settings */
export function enabledTools(settings: AgentSettings): AgentToolDef[] {
  return AGENT_TOOLS.filter(t => t.enabled?.(settings) ?? true);
}

/** Build the AI SDK tool set. Approval tools get no `execute` (human in the loop). */
export function buildToolSet(settings: AgentSettings): ToolSet {
  const set: ToolSet = {};
  for (const [name, external] of Object.entries(externalTools)) {
    set[name] = { ...external, execute: undefined } as ToolSet[string];
  }
  for (const def of enabledTools(settings)) {
    set[def.name] = def.approval === 'auto'
      ? tool({
          description: def.description,
          inputSchema: def.input,
          execute: (args: unknown, options: { toolCallId: string }) =>
            def.run(args as never, { toolCallId: options.toolCallId }),
        })
      : tool({ description: def.description, inputSchema: def.input });
  }
  return set;
}

/** Whether the tool would ever ask for approval (for UI labels) */
export function requiresApproval(name: string): boolean {
  if (isExternalTool(name)) return true;
  const def = getToolDef(name);
  return !!def && def.approval !== 'auto';
}

/** Decide whether this specific call needs the technician's approval. */
export async function needsApproval(name: string, args: unknown, mode: ApprovalMode): Promise<boolean> {
  if (isExternalTool(name)) return mode !== 'yolo';
  const def = getToolDef(name);
  if (!def || def.approval === 'auto') return false;
  if (mode === 'yolo') return false;
  if (def.approval === 'command' && mode === 'whitelist') {
    const command = (args as { command?: unknown })?.command;
    if (typeof command !== 'string') return true;
    try {
      return !(await commands.checkCommandWhitelisted(command));
    } catch {
      return true;
    }
  }
  return true;
}

/** Run an approved (or auto-approved) call. Throws on failure. */
export async function runTool(name: string, args: unknown, ctx: ToolRunContext): Promise<unknown> {
  if (isExternalTool(name)) {
    const execute = externalTools[name].execute;
    if (!execute) throw new Error(`MCP tool ${name} cannot be executed`);
    return execute(args as never, { toolCallId: ctx.toolCallId, messages: [], context: undefined });
  }
  const def = getToolDef(name);
  if (!def) throw new Error(`Unknown tool: ${name}`);
  return def.run(args as never, ctx);
}

/** Activity type and card details for a tool call */
export function describeToolCall(name: string, args: unknown): { type: ActivityType; details: Record<string, unknown> } {
  const def = getToolDef(name);
  if (!def) {
    let argText: string;
    try {
      argText = JSON.stringify(args) ?? '';
    } catch {
      argText = '';
    }
    return { type: 'mcp_tool', details: { toolName: name, arguments: compact(argText, 400) } };
  }
  const a = (args && typeof args === 'object' ? args : {}) as Record<string, unknown>;
  let details: Record<string, unknown>;
  try {
    details = def.details?.(a) ?? {};
  } catch {
    details = {};
  }
  return { type: def.activity, details };
}

/** Sidebar summary of the general-purpose tools offered with the given settings */
export function toolSummary(settings: AgentSettings) {
  const enabled = new Set(enabledTools(settings).map(t => t.name));
  return AGENT_TOOLS
    .filter(t => !t.activity.startsWith('service_'))
    .map(t => ({
      id: t.name,
      name: t.label,
      desc: t.summary,
      requiresApproval: t.approval !== 'auto',
      enabled: enabled.has(t.name),
    }));
}
