# Endur Release Notes (v1.2.13)

We are pleased to announce the release of **Endur** (v1.2.13). This release delivers major enhancements across core reliability, daemon lifecycle, Desktop application integration, Neovim editor support, and automated background maintenance.

---

## 🚀 Key Highlights

### 1. Neovim Integration & Discrete Restore Path Normalization (`endur-nvim`)
* **Path Normalization for Discrete Restores**: When restoring single files (`-f` / `--files`), Endur now automatically normalizes both relative and absolute filesystem paths against the Git repository root. This resolves an issue where absolute buffer paths provided by editor plugins failed to match repo-relative delta paths during restore.
* **Expanded Command Suite in Neovim**: Added `:EndurUnwatch`, `:EndurSnapshotsSnacks`, `:EndurSnapshotsFzf`, and `:EndurTUI` commands, complementing `:EndurWatch`, `:EndurSnapshots`, and `:EndurStatus`.
* **Multi-Picker Support & Diff Previews**: Support for Snacks.nvim, Fzf-Lua, and Telescope pickers with inline diff previews and discrete `<C-f>` single-file restore action.
* **Automated Monorepo Subtree Sync**: Changes made to `plugins/endur-nvim` in the monorepo are now automatically synchronized to the standalone [`PJC-64/endur-nvim`](https://github.com/PJC-64/endur-nvim) repository on every push to main.

### 2. Endur Desktop GUI Architecture & Daemon Resolution
* **Prioritized Binary Resolution**: Resolved an issue where EndurDesktop could spawn or connect to stale `v1.2.3-dev1` daemon binaries from `~/.cargo/bin`. The resolver now prioritizes bundled and sibling `endur` binaries matching the desktop package release.
* **Modular Svelte 5 Architecture**: Refactored the desktop client frontend into discrete, maintainable Svelte components (`DashboardTab`, `WatchlistTab`, `BackupsTab`, `AnalyticsTab`, `ServiceTab`).
* **Robust Daemon Control & Config Persistence**: Fixed desktop daemon restart lifecycle and ensured atomic configuration saves.

### 3. Core Engine & Daemon Efficiency
* **Idle Debouncer Sleep**: Eliminated busy spin loops in the debounce poller by sleeping until the earliest pending repository debounce time or falling back to an idle sleep when no repositories have pending modifications.
* **Native Signal Dispatch**: Replaced shell kill invocations (`kill -9`) with direct native OS signal dispatch (`libc::kill` on Unix) for clean, reliable process termination.
* **SQLite Cache Optimization & Atomic Transactions**: Enhanced snapshot metadata caching with atomic batch transactions and automatic cache eviction when repositories are unwatched or snapshots are pruned.
* **Automated Log Rotation**: Added log rotation for `endur.log` to cap disk usage and memory footprint during extended background daemon execution.
* **Unified Git Diff Logic**: Consolidated snapshot diff generation into a shared helper that safely handles added, modified, and deleted files.

### 4. TUI Display & Rendering Fixes
* **Clean Watch/Unwatch in TUI**: Suppressed extraneous stdout notifications during repository watch/unwatch operations to prevent display corruption in the TUI snapshot counters.
* **Live Log Streaming**: Improved initial log tailing and live update dispatch in the TUI log pane.

### 5. Security & Dependency Maintenance
* **Vulnerability Resolutions**: Resolved Dependabot CVEs across frontend dependencies including Vite, devalue, and source-map-js.
* **Dead Code & Dependency Pruning**: Removed obsolete dependencies (`walkdir`, `hdrhistogram`, `sudo`) and pruned unused structs.
* **Cross-Platform IPC & CI Fixes**: Resolved closure borrow issues on Windows IPC servers and ensured non-interactive apt-get execution in Ubuntu CI.

---
*Note: This fork is actively maintained and has been built with AI pair-programming assistance.*
