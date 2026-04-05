# HUDcon

A console version of the "HUD" app that displays machine info and automates some simple installations and machine maintenance.

Web version, using SSE  
<https://github.com/merklegroot/hudsse>

Original web version  
<https://github.com/merklegroot/hudapp>

## Run (console)

```bash
cargo run
```

## Run (Tauri desktop)

The desktop UI lives under `ui/` (Vite + TypeScript). The Rust crate `hudcon` is shared with the CLI; `src-tauri` exposes the same `gather_*` APIs via Tauri commands.

1. Install JS dependencies and build the frontend into `dist/` (required before compiling the Tauri crate):

   ```bash
   npm install
   npm run build
   ```

2. Start the app in dev mode (runs Vite and the Tauri backend):

   ```bash
   npm run tauri dev
   ```

To build the desktop binary only (after step 1):

```bash
cargo build -p hudcon-tauri --release
```

Workspace note: `cargo build` / `cargo test` at the repo root build **only** the `hudcon` library and CLI (`default-members`). Use `cargo build --workspace` or `-p hudcon-tauri` when you need the desktop crate.

## Debug (VS Code / Cursor)

`.vscode/launch.json` uses **CodeLLDB**.

Install the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension, then use **Run and Debug** on **Debug HUDcon**. You’ll be prompted **once** for **`hudcon`** (console) or **`hudcon-tauri`** (desktop). The pre-launch step always runs **`npm run build && cargo build --workspace`** so `dist/` exists and both crates are built, whichever binary you pick.
