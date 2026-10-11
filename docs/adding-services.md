# Adding New Services to RustService

This guide explains how to add a new service to the modular service automation system.

## Overview

Services are modular diagnostic/maintenance tasks that can be:
- Combined into presets (Diagnostics, General, Complete, Custom)
- Individually enabled/disabled by users
- Configured with custom options
- Run in sequence or in parallel (experimental) with live log output
- Have custom results renderers (for findings view and customer print)

## Architecture

The service system uses a modular architecture:

```
src-tauri/src/services/     # Backend: One file per service
├── mod.rs                  # Service trait & registry
├── ping_test.rs            # Ping test service
└── [new_service].rs        # Your new service

src/components/service-renderers/   # Frontend: Custom renderers
├── index.ts                        # Renderer registry
├── PingTestRenderer.tsx            # Ping test renderer
└── [NewService]Renderer.tsx        # Your new renderer
```

---

## Steps to Add a New Service

### 1. Create the Service File (Backend)

Create a new file in `src-tauri/src/services/`. A service describes itself in
`definition()` and does its work in `run()` through the `ServiceCtx` it is
given, which:

- streams log lines to the UI (`ctx.log(...)`, one consistent `service-log` payload),
- collects findings (`ctx.finding(...)` or `ctx.findings.push(...)`),
- runs child processes so that **cancelling the run kills them**
  (`ctx.output(&mut cmd)`; use `crate::proc::command(...)` so no console window flashes),
- builds the result with the measured duration (`ctx.finish(success, error)`).

```rust
// src-tauri/src/services/my_service.rs

use serde_json::json;

use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, ServiceDefinition, ServiceOptionSchema, ServiceResult,
};

pub struct MyService;

impl Service for MyService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "my-service".to_string(),
            name: "My Service".to_string(),
            description: "What this service does".to_string(),
            category: "diagnostics".to_string(), // or "cleanup", "security", "maintenance"
            estimated_duration_secs: 30,
            required_programs: vec![], // IDs from REQUIRED_PROGRAMS (e.g. "bleachbit")
            options: vec![ServiceOptionSchema {
                id: "option_name".to_string(),
                label: "Option Label".to_string(),
                option_type: OptionType::Number, // String, Boolean, Select, UsbDrive
                default_value: json!(10),
                min: Some(1.0),
                max: Some(100.0),
                options: None,
                description: Some("Help text".to_string()),
            }],
            icon: "icon-name".to_string(), // lucide icon name
            exclusive_resources: vec![],   // resource tags for parallel mode (see below)
            dependencies: vec![],          // services that must run earlier in the queue
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        let count = options.get("option_name").and_then(|v| v.as_u64()).unwrap_or(10);
        ctx.log(format!("Starting my service ({} items)...", count));

        // External tools: hidden window, killed if the user cancels the run.
        let output = match ctx.output(crate::proc::command("whoami").arg("/all")) {
            Ok(o) => o,
            Err(e) => {
                ctx.log(format!("ERROR: {}", e));
                return ctx.finish(false, Some(e.to_string()));
            }
        };
        // Decodes UTF-8, UTF-16LE and the console code page (non-English Windows)
        let text = crate::proc::decode_output(&output.stdout);

        ctx.finding(
            FindingSeverity::Success,
            "Result Title",
            format!("Read {} lines", text.lines().count()),
            None,
            Some(json!({ "type": "my_finding_type", "value": 42 })), // for a custom renderer
        );

        ctx.log("Service complete");
        ctx.finish(true, None)
    }
}
```

Parse tool output without relying on English wording where you can (exit codes,
numbers, `TTL=`-style tokens): technicians run this on localized Windows.

### 2. Register the Service

Edit `src-tauri/src/services/mod.rs`. The registry is an ordered list, which is
also the order services are shown in:

```rust
mod my_service; // module declaration

static SERVICE_REGISTRY: LazyLock<Vec<(String, Box<dyn Service>)>> = LazyLock::new(|| {
    let services: Vec<Box<dyn Service>> = vec![
        // ...existing services
        Box::new(my_service::MyService),
    ];
    // ...
});
```

### 3. Add to Presets (Optional)

In `get_all_presets()` in the same file, use the `svc` helper:

```rust
svc("my-service", true, json!({ "option_name": 10 })),
```

### 4. Add Icon to Frontend (if new)

Edit `src/pages/ServicePage.tsx`:

```tsx
import { MyIcon } from 'lucide-react';

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  // existing icons...
  'my-icon': MyIcon,
};
```

---

## Adding a Custom Results Renderer (Optional)

For enhanced results display, create a custom renderer:

### 1. Create the Renderer

```tsx
// src/components/service-renderers/MyServiceRenderer.tsx

import type { ServiceRendererProps } from './index';

function FindingsRenderer({ result, definition }: ServiceRendererProps) {
  // Extract custom data from findings
  const finding = result.findings.find(
    (f) => (f.data as any)?.type === 'my_finding_type'
  );
  const data = finding?.data as { value: number } | undefined;

  return (
    <div className="p-4 rounded-lg bg-muted/50 border">
      <h3>{definition.name}</h3>
      {data && <p className="text-2xl font-bold">{data.value}</p>}
    </div>
  );
}

function CustomerRenderer({ result }: ServiceRendererProps) {
  // Simplified view for customer print
  return (
    <div className="p-4 border border-gray-200 rounded-lg bg-white">
      <p className="font-bold">{result.success ? '✓ Passed' : '✗ Failed'}</p>
    </div>
  );
}

export function MyServiceRenderer(props: ServiceRendererProps) {
  if (props.variant === 'customer') {
    return <CustomerRenderer {...props} />;
  }
  return <FindingsRenderer {...props} />;
}
```

### 2. Register the Renderer

Edit `src/components/service-renderers/index.ts`:

```typescript
import { MyServiceRenderer } from './MyServiceRenderer';

export const SERVICE_RENDERERS: Partial<Record<string, ServiceRenderer>> = {
  'ping-test': PingTestRenderer,
  'my-service': MyServiceRenderer,  // Add your renderer
};
```

---

## Service Categories

| Category | Description |
|----------|-------------|
| `diagnostics` | System health checks, tests |
| `cleanup` | Junk file removal, optimization |
| `security` | Malware/adware scanning |
| `maintenance` | Updates, repairs |

## Finding Severities

| Severity | Use For |
|----------|---------|
| `Info` | Neutral information |
| `Success` | Passed checks, good results |
| `Warning` | Minor issues, recommendations |
| `Error` | Problems that need attention |
| `Critical` | Severe issues requiring immediate action |

## External Program Dependencies

If your service requires an external program (like BleachBit or AdwCleaner), you need to:

### 1. Add Program to Registry

Edit `src-tauri/src/commands/required_programs.rs` and add an entry to `REQUIRED_PROGRAMS`:

```rust
RequiredProgramDef {
    id: "my-program".to_string(),           // Stable ID used in service definitions
    name: "My Program".to_string(),          // Display name
    description: "What this program does".to_string(),
    exe_names: vec![                         // Executable names to auto-detect
        "myprogram.exe".to_string(),
        "myprogram64.exe".to_string(),       // Include variants
    ],
    url: Some("https://example.com/".to_string()),  // Download link (optional)
},
```

### 2. Reference in Service Definition

In your service's `definition()` method, add the program ID to `required_programs`:

```rust
required_programs: vec!["my-program".to_string()],
```

### 3. Use the Program in Your Service

Get the executable path via the command:

```rust
use crate::commands::required_programs::get_program_exe_path;

// In your run() method:
let exe_path = match get_program_exe_path("my-program".to_string()) {
    Ok(Some(path)) => path,
    _ => return ctx.finish(false, Some("my-program executable not found".to_string())),
};

// Run it through the context so a cancel stops it
let output = match ctx.output(crate::proc::command(&exe_path).args(["--your", "--args"])) {
    Ok(o) => o,
    Err(e) => return ctx.finish(false, Some(format!("Failed to run my-program: {}", e))),
};
```

### How Auto-Detection Works

1. **Programs folder scan**: The system searches `data/programs/` recursively for any exe matching the `exe_names` list
2. **User override**: Users can set a custom path in **Settings → Programs** if auto-detection fails
3. **Validation**: Before running services, the system validates all required programs are available

### User Experience

Users see required programs in **Settings → Programs**:
- **Added**: Program auto-detected or custom path set ✓
- **Missing**: Program not found, with download link if available

If a service run is blocked due to missing programs, a dialog shows what's needed and links to Settings.

## Testing

1. Run `bun tauri dev`
2. Navigate to Service tab
3. Select a preset that includes your service
4. Verify it appears in the queue
5. Run the service and check logs/findings
6. If you added a custom renderer, verify it displays correctly

---

## Parallel Execution (Experimental)

Services can optionally run in parallel. When the user enables the **Parallel** toggle in the queue view, services without conflicting resource tags execute concurrently, while services sharing any resource tag are serialized.

### `exclusive_resources` Field

Each `ServiceDefinition` has an `exclusive_resources: Vec<String>` field that declares which shared resources this service requires exclusive access to. Services with overlapping tags will never run at the same time.

**Resource tags in use:**

| Tag | Description | Services |
|-----|-------------|----------|
| `network-bandwidth` | Services that measure network speed | speedtest, iperf |
| `cpu-stress` | CPU/GPU stress tests and benchmarks | heavyload, winsat, furmark |
| `disk-exclusive` | Services that lock a disk volume | chkdsk |
| `disk-heavy` | Heavy disk I/O (repairs, cleanups) | dism, sfc, bleachbit, drivecleanup |
| `filesystem-scan` | Full filesystem scans (virus/malware) | kvrt-scan, adwcleaner, stinger |

Services with an empty `exclusive_resources` vec (e.g., `ping-test`, `battery-info`, `disk-space`, `network-config`) can run in parallel with anything.

### How It Works

1. The scheduler maintains a set of currently-held resource tags
2. For each unstarted service, it checks if any of its `exclusive_resources` overlap with held resources
3. If no conflict, the service starts on a new thread and its resources are marked as held
4. When a service completes, its resources are released, potentially unblocking waiting services
5. Services with no resource tags can always start immediately

### Choosing Resource Tags

When adding a new service, consider:
- **Does it heavily use the network?** → Add `"network-bandwidth"`
- **Does it stress the CPU or GPU?** → Add `"cpu-stress"`
- **Does it lock a volume/drive?** → Add `"disk-exclusive"`
- **Does it do heavy disk I/O?** → Add `"disk-heavy"`
- **Does it scan the entire filesystem?** → Add `"filesystem-scan"`
- **Is it a lightweight info-gathering tool?** → Use `vec![]`

You can also create a new tag if needed. Any string works as a resource tag — services sharing the same tag will be serialized.
