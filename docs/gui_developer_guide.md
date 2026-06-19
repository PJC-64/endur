# Endur GUI Developer Guide

This document details the development environment, frontend architecture, and Tauri IPC interfaces for the Endur Desktop Application. It serves as a guide for developers wishing to compile, modify, or extend the GUI client.

---

## 1. Technical Stack

The desktop client is structured as a member of the Cargo workspace:
- **Core Engine**: Rust (embedded as a library dependency in Tauri).
- **Desktop Framework**: [Tauri 2.0](https://tauri.app) to build native installer assets and expose OS-level integrations.
- **Frontend Framework**: [Svelte 5](https://svelte.dev) utilizing TypeScript and Vite.
- **Styling**: Vanilla CSS with custom scrollbars, animations, and glassmorphic panels.

---

## 2. Directory Layout

The GUI code lives in `crates/endur-desktop/`:
```text
crates/endur-desktop/
├── package.json              # NPM dependencies & scripts (Vite, Svelte, Tauri CLI)
├── tsconfig.json             # TypeScript configuration
├── vite.config.js            # Vite compilation config
├── svelte.config.js          # Svelte configuration
├── src/                      # Svelte Frontend Root
│   └── routes/
│       ├── +layout.ts        # SPA routing setting (disabling SSR)
│       └── +page.svelte      # Main Application View & State
└── src-tauri/                # Tauri Backend Root
    ├── Cargo.toml            # Rust dependencies (walkdir, dirs, tokio)
    ├── tauri.conf.json       # App configuration (version, identifier, permissions)
    └── src/
        ├── lib.rs            # Application entry, system menu, & command registration
        ├── main.rs           # Launcher entrypoint
        └── commands.rs       # IPC command handlers linking frontend to endur core
```

---

## 3. Tauri IPC Command Layer

The frontend communicates with the Rust backend via Tauri's IPC system using the `invoke` API. The backend commands are defined in `crates/endur-desktop/src-tauri/src/commands.rs`.

### Daemon Control Commands
- `get_daemon_status() -> Result<DaemonStatus, String>`:
  Checks if a daemon is running (direct process).
- `control_daemon(action: String) -> Result<String, String>`:
  Starts or stops the direct background daemon process.

### Service Management Commands
- `is_service_installed() -> Result<bool, String>`:
  Checks if the OS system service configuration exists (`launchd` plist on macOS, `systemd` user unit on Linux).
- `is_service_running() -> Result<bool, String>`:
  Queries the OS service manager to check if the background service is active.
- `control_service(action: String) -> Result<String, String>`:
  Instructs the OS service manager to perform `install`, `uninstall`, `start`, or `stop` actions.

### Repository Selection & Watchlist Commands
- `get_base_root_dir() -> Result<String, String>`:
  Resolves the configured `base_root` path from `config.json` (resolving `~/` prefixes to the user's home folder) or falls back to `~/Development`.
- `get_selector_entries(current_dir: String) -> Result<Vec<SelectorEntry>, String>`:
  Tails the subdirectories of `current_dir`. It filters out hidden files (directories beginning with `.`) and checks if they contain a Git repository. To prevent hangs on deeply nested non-git directory hierarchies, it uses `walkdir` limited to a depth of 5 subfolders.
- `is_git_repo(path: String) -> Result<bool, String>`:
  Quickly checks if the specified path has a `.git/` folder.

---

## 4. Frontend Architecture & State Management

The frontend is a single-page app inside [+page.svelte](file:///Users/pjc/Development/endur/crates/endur-desktop/src/routes/+page.svelte) using Svelte 5's new reactivity model.

### State Declarations (`$state`)
- `activeTab`: Tracks the selected navigation tab (`dashboard`, `repos`, `backups`, `service`).
- `watchedRepos`: Stores the array of directory paths currently monitored.
- **Repository Browser State**:
  - `selectorCurrentDir`: The path currently opened in the folder browser.
  - `selectorEntries`: An array of child subdirectories containing git repos, mapped to `{ name, path, is_repo }`.
  - `isCurrentDirRepo`: Boolean indicating if the currently opened folder is itself a Git repository.
  - `selectorLoading`/`selectorError`: Handles UI state spinners and error notices during directory walks.
- **Service Management State**:
  - `serviceInstalled`: Boolean indicating if system service is registered.
  - `serviceRunning`: Boolean indicating if system service is active.
  - `runningServiceVersion`: Stores the running service version parsed from log metrics or status check.

### Directory Selector Functions
- `initSelector()`: Runs on component mount. Resolves `baseRootDir = await invoke("get_base_root_dir")` and calls `navigateSelector(baseRootDir)`.
- `navigateSelector(dir: string)`: Updates `selectorCurrentDir`, triggers the directory walk, and loads the list of subfolders. It concurrently calls `is_git_repo` to update `isCurrentDirRepo`.
- `goUpSelector()`: Splits the current directory path on directory separators and navigates one level up.
- `watchPath(path: string)`: Calls the Tauri watch command and updates the local watched list.

---

## 5. Development Workflow

### Prerequisites
1. Install system prerequisites (Xcode Command Line Tools on macOS, Build Essentials on Linux, or WebView2 on Windows).
2. Install NodeJS (v18+) and Rust.

### Running in Development Mode
To launch the desktop app in live-reloading development mode:
```bash
cd crates/endur-desktop
npm run dev
# In another terminal window:
npm run tauri dev
```

### Compiling and Bundling
To build production-ready installers:
```bash
cd crates/endur-desktop
npm run build
npm run tauri build
```
This generates binaries and bundles them into platform-specific installers (e.g. `.dmg` on macOS) under `crates/endur-desktop/src-tauri/target/release/bundle/`.
