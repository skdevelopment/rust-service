# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
`AGENTS.md` has the full development guide; `docs/` has the agent and service guides.

## Project Overview

RustService is a portable Windows desktop toolkit for computer repair technicians. Built with **Tauri 2** (Rust backend + React frontend), targeting Windows 10/11 only.

## Commands

```bash
bun install                 # Install frontend dependencies
bun tauri dev               # Run in dev mode (some features require admin)
bun tauri build             # Build → src-tauri/target/release/rustservice.exe
bun run typecheck           # tsc -b
bun run lint                # oxlint (ESLint can't run on TypeScript 7)
cd src-tauri && cargo test  # Rust tests (also regenerates src/bindings.ts)
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

Rust unit tests cover the backend logic; UI verification is manual (`bun tauri dev`).

### Adding shadcn Components
Always use the CLI, never create them manually:
```bash
bunx shadcn@latest add <component-name>
```
Components use the `base-nova` style (Base UI primitives, not Radix) with TailwindCSS v4:
compose with the `render` prop instead of `asChild`, and pass `nativeButton={false}` when a
button-like trigger renders a non-`<button>` element. Select/Slider `onValueChange` receive
`string | null` / `number | readonly number[]`. Do NOT manually edit files in `src/components/ui/`.

## Architecture

### Frontend (`src/`)
- **`App.tsx`** — Tab navigation (`TABS`), providers: Settings → Theme → Animation → ServiceRun → AgentSession.
- **`bindings.ts`** — GENERATED typed IPC bindings (tauri-specta). Never edit; never call `invoke` directly.
- **`pages/`** — One component per tab.
- **`components/`** — Shared components and providers (`settings-context.tsx`, `theme-provider.tsx`, `agent-session-context.tsx`, `service-run-context.tsx`, `animation-context.tsx`).
- **`lib/agent-tools.ts`** — The single agent tool registry.
- **`types/`** — UI types; types mirroring Rust are aliases of `bindings.ts`.
- **`styles/`** — `globals.css` (Tailwind setup, `@theme inline` bindings) and `themes/<id>.css`, one per color scheme. A new scheme is imported in `globals.css` and added to `COLOR_SCHEMES` / `ColorScheme` in `src/types/settings.ts`.

### Backend (`src-tauri/src/`)
- **`lib.rs`** — App setup and the command list (`specta_builder()`); exports bindings in debug.
- **`commands/`** — Tauri commands per domain (`agent/`, `services/`, `settings.rs`, ...).
- **`services/`** — One file per service, all using `ServiceCtx` (`services/ctx.rs`).
- **`mcp/`** — Built-in MCP server (localhost by default, Bearer auth).
- **`proc.rs`** — Child processes (hidden window, timeouts, output decoding). **`llm.rs`**, **`logging.rs`**.
- **`types/`** — Serde + specta types shared with the frontend.

### Data Folder (Portable)
Created next to the `.exe` (dev: the working directory's `data/`, usually `src-tauri/data/`):
`programs/`, `reports/`, `scripts/`, `instruments/`, `logs/`, `agent/memory.db` (conversations), `agent/files/`, `settings.json`.

## Key Patterns & Conventions

### Tauri IPC — always through the generated bindings
```typescript
import { commands } from '@/bindings';
const settings = await commands.getSettings(); // ✅ typed
await invoke('get_settings');                   // ❌ never call invoke directly
```
Tauri maps Rust `snake_case` parameters to **camelCase** JS keys; the bindings handle this, so argument mistakes are compile errors.

### Adding a Tauri Command
1. Add the function in `src-tauri/src/commands/*` with `#[tauri::command(async)]` (or `async fn`) **and** `#[specta::specta]`. Plain sync commands run on the UI thread — don't.
2. IPC types derive `Serialize`/`Deserialize` + `specta::Type`, with `#[serde(rename_all = "camelCase")]`.
3. Register it in `collect_commands![...]` in `specta_builder()` (`src-tauri/src/lib.rs`).
4. Regenerate `src/bindings.ts` (run the app in dev, or `cargo test --test bindings`) and commit it — CI fails if it is stale.
5. Add capability permissions in `src-tauri/capabilities/default.json` only for plugin APIs.

### Child processes
Use `crate::proc::command(...)` (no console window) and `proc::output_with_timeout` for queries; in services use `ctx.output(&mut cmd)` so cancel kills the process. Decode with `proc::decode_output`. Don't parse localized English text.

### Settings System
- **Access**: `useSettings()` from `@/components/settings-context`; `updateSetting('a.b.c', value)` accepts any dotted path (type-checked). Use `useDebouncedSetting` for text inputs.
- **Theme**: `useTheme()` from `@/components/theme-provider` (stored in `appearance.*`).
- **Backend**: `commands/settings.rs` owns the file — use `load_settings()` / `update_settings(|s| ...)`, never write `settings.json` directly.
- **Adding a setting**: add the field (with `#[serde(default)]`) to the Rust struct in `src-tauri/src/types/settings.rs`, regenerate bindings, add UI in the settings panel.

### Animation System
- Toggle-able via `appearance.enableAnimations`
- Import from `@/components/animation-context`: `useAnimation`, `motion`, `AnimatedList`, `AnimatedItem`
- Presets: `fadeIn`, `fadeInUp`, `fadeInScale`, `staggerContainer`, `staggerItem`, `hoverScale`, `hoverLift`

### Service System
4-step flow: **Presets → Queue → Runner → Results**
- Services: `src-tauri/src/services/*.rs`, registered (in display order) in `services/mod.rs`
- Runner state machine: `commands/services/runner.rs`; reports: `commands/services/reports.rs`
- Parallel execution uses `exclusive_resources` tags; `dependencies` must run earlier
- See `docs/adding-services.md`

### Agent System
- `AgentSessionProvider` (`components/agent-session-context.tsx`) owns the conversation above the tabs; one serialized turn queue; Stop/Clear abort cleanly.
- Tools are defined once in `src/lib/agent-tools.ts` (schema, approval rule, activity card, sidebar, executor).
- Approval modes: `always` (default), `whitelist` (backend-checked simple commands), `yolo`.
- Conversations are stored as model history in `data/agent/memory.db`.
- See `docs/agent-system.md`.

## Known Expected Lint Warnings (CSS)
These are valid TailwindCSS v4 / Windows-specific syntax — ignore them:
- `Unknown at rule @custom-variant`, `@theme`, `@apply`
- `Unknown property: 'app-region'` (titlebar drag region)
