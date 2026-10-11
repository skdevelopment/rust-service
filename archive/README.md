# Archive

Inactive experiments. **RustService’s current product is the Tauri + React app** at the repo root (`src/`, `src-tauri/`).

| Path | What it is |
|------|------------|
| `src-winui/` | Abandoned C# / WinUI 3 / Windows App SDK port. Not used for builds or `bun tauri dev`. |

To work on the WinUI snapshot:

```powershell
dotnet build archive\src-winui\RustService.sln -c Debug -p:Platform=x64
```
