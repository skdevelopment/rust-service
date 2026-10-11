# RustService (WinUI 3) — archived

Inactive C# / WinUI 3 / Windows App SDK 2.5.1 port. **The live product is Tauri** (`bun tauri dev` from the repo root).

```powershell
dotnet build archive\src-winui\RustService.sln -c Debug -p:Platform=x64
dotnet run --project archive\src-winui\RustService\RustService.csproj -c Debug -p:Platform=x64
```

Unpackaged WinUI 3 on Windows App SDK **2.5.1**, `win-x64`. After build, output was staged at `archive/src-winui/dist/`. `data/` is created next to the exe.

**Release** builds are unpackaged Windows App SDK self-contained (`WindowsAppSDKSelfContained=true`). **Debug** is framework-dependent: self-contained Release could fail-fast (`c0000402` / `c0000602`) on some Insider hosts when creating the window.
