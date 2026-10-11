# Agent System

The Agent tab is an AI assistant for repair technicians. It runs on the
technician's configured model (OpenAI, Anthropic, Google, xAI, Mistral,
DeepSeek, Groq, OpenRouter, Ollama or any OpenAI-compatible endpoint) through
the Vercel AI SDK, and acts on the machine through tools. Anything that changes
the system waits for the technician's approval unless the approval mode says
otherwise.

## Key files

| File | Purpose |
|------|---------|
| `src/components/agent-session-context.tsx` | The agent session: conversation state, the turn loop, approvals, Stop/Clear, MCP connections, service-run supervision |
| `src/lib/agent-tools.ts` | **The tool registry** — every tool's schema, approval rule, activity card mapping, sidebar label and executor |
| `src/lib/agent-chat.ts` | Provider/model creation, system prompt, `streamChat`, legacy history normalization |
| `src/lib/agent-conversations.ts` | Saving/loading conversations and rebuilding chat messages from history |
| `src/lib/mcp-manager.ts` | Connections to external MCP servers (agent as MCP client) |
| `src/pages/AgentPage.tsx` | The chat view (state lives in the session, not the page) |
| `src/components/agent/*` | Message list, activity/approval cards, input area, sidebar |
| `src-tauri/src/commands/agent/` | Backend commands: shell execution, file tools, attachments, conversations, web search |
| `src-tauri/src/mcp/` | The built-in MCP server (agent as MCP server for external LLMs) |

## Session and turn loop

`AgentSessionProvider` is mounted in `App.tsx` above the tab switcher, so
switching tabs never interrupts a turn, loses a pending approval, or drops
service-run updates. All turns go through **one serialized dispatch queue**.

A turn:

1. calls `streamText` with the history and the tool set from the registry;
2. renders the stream (text, tool calls, results, tool errors);
3. appends the SDK's own `response.messages` to the history — every executed
   tool call, tool error and invalid call (unknown tool, arguments failing the
   schema) is paired with a result, so the history sent to the provider is
   always well-formed;
4. handles calls to approval tools (which have no `execute` in the SDK):
   auto-approved ones run immediately; otherwise the turn **suspends** with
   status `awaiting_approval` until the technician approves or denies each one,
   then their results are appended and the turn continues.

While a turn is suspended the input is disabled (sending a message then would
leave tool calls without results).

**Stop** aborts the stream (the AI SDK v5 stream ends with an `abort` part
rather than throwing), drops queued jobs, records "stopped" results for any
pending approvals and never auto-continues. **Clear** stops and starts a new
conversation.

## Tools

Tools are defined once in `AGENT_TOOLS` (`src/lib/agent-tools.ts`):

```ts
defineTool({
  name: 'read_file',
  label: 'Read File',                 // sidebar + activity card
  summary: 'Open file contents',      // sidebar description
  activity: 'read_file',              // activity card type
  approval: 'auto',                   // 'auto' | 'always' | 'command'
  description: '...',                 // sent to the model
  input: z.object({ path: z.string().min(1) }),
  details: a => ({ path: a.path }),   // fields shown on the card
  run: a => commands.agentReadFile(a.path, null, null, true),
})
```

Everything else — the SDK tool set, the system prompt's tool list, the
sidebar, activity cards and approval decisions — is derived from it. Argument
validation lives in the zod schema, so invalid calls become tool errors the
model can correct instead of approval cards that can never run.

Backend calls go through the generated `commands` object from
`src/bindings.ts`, so argument names and types are checked by `tsc`.

### Approval modes

| Mode | Behaviour |
|------|-----------|
| `always` (default) | Every `always`/`command` tool needs approval |
| `whitelist` | `execute_command` runs automatically if it matches a whitelist regex **and** is a single simple command (no `;`, `|`, `&`, redirection, `$(...)`, variables) — checked by the backend (`check_command_whitelisted`); everything else needs approval |
| `yolo` | Nothing needs approval (dangerous) |

Tools from connected external MCP servers always need approval unless the
mode is YOLO.

Approval cards show exactly what will happen: the full command, the content of
files to be written or generated, the edit diff, and the services (with
options) a service run will start.

### File access policy

The agent's file tools (and the MCP server's) share one policy,
`validate_agent_path` in `src-tauri/src/commands/agent/files.rs`:

- paths must be under the data folder, the working directory or the user's
  home directory;
- `Windows\System32`, `SysWOW64`, `WinSxS` and `WindowsApps` are denied;
- the app's own secrets — `settings.json` (API keys), its backups and the agent
  database — are denied;
- glob/grep patterns may not contain `..` or absolute paths.

## Services

`run_service_queue` (approval required) starts a run with
`agentInitiated: true`; `run_services` returns the report id as soon as the run
starts. The session listens for `service-state-changed` / `service-completed`
and feeds each completed service — and the final summary — back to the agent as
an update message, delivered only when the agent is idle. The agent then uses
the report tools (`get_service_report`, `set_service_analysis`,
`set_report_summary`, `set_health_score`, `export_report`, …).

## Conversations

Conversations are stored in `data/agent/memory.db` (SQLite) as the model
history itself (`ModelMessage[]`), saved atomically after every turn. On load
the history is normalized (older formats, tool calls left without results) and
the chat UI is rebuilt from it.

## File attachments

Uploaded and generated files live in `data/agent/files/`. Ids are UUIDs and are
validated before use. **Open** copies the file to a temp folder under its
original name and opens it; scripts and executables are only revealed in
Explorer, never launched.

## Chat rendering

Markdown links open in the system browser. Remote images are not loaded (a
prompt-injected model could otherwise leak data through an image URL); inline
`data:` images still render. The production build also has a Content Security
Policy (`tauri.conf.json`).

## MCP server (remote control)

Settings → AI Agent → MCP Server exposes the agent's tools to external LLM
clients over JSON-RPC (`POST /mcp`). It:

- listens on `127.0.0.1` only, unless **Allow network access** is enabled;
- requires `Authorization: Bearer <key>` (keys shorter than 16 characters are
  refused; the UI generates 32);
- refuses browser requests (any `Origin` header) and, on loopback, requests
  whose `Host` is not local (DNS-rebinding protection);
- applies settings immediately — no restart needed.

MCP clients run tools **without approval**, so only give the key to clients
you trust.

Tools: `execute_command`, `read_file`, `write_file`, `edit_file`, `list_dir`,
`move_file`, `copy_file`, `grep`, `glob`, `get_system_info`, `search_web`,
`list_programs`, `find_exe`, `list_instruments`, `run_instrument`.

```bash
curl -X POST -H "Authorization: Bearer <KEY>" -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}' http://localhost:8377/mcp
```

## Adding a tool

1. If it needs the backend, add a `#[tauri::command]` + `#[specta::specta]`
   function and register it in `specta_builder()` in `src-tauri/src/lib.rs`;
   run the app (or `cargo test --test bindings`) to regenerate `src/bindings.ts`.
2. Add a `defineTool({...})` entry to `AGENT_TOOLS`. Choose `approval: 'auto'`
   only for read-only or report-editing tools.
3. If it needs a new activity card type, add it to `src/types/agent-activity.ts`
   and `AgentActivityItem.tsx`.
4. To expose it to MCP clients too, add it to `src-tauri/src/mcp/tools.rs`
   (calling the same backend function).
