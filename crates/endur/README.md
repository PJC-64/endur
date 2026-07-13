# Endur (CLI & TUI)

`endur` is an automatic Git-backed auto-backup daemon. It runs invisibly as a background OS-level service, monitoring your active Git repositories for file modifications. When changes are made, it immediately debounces and commits snapshots to isolated backup branches (`endur/HEAD_HASH`), ensuring that your work-in-progress is protected from crashes, editor loss, or accidental deletion without ever cluttering your active Git staging area.

---

## Features

* **Event-Driven Backups**: Listens to native filesystem events (`notify` crate) with a 500ms debounce window. Extremely CPU-efficient (no heavy polling loops).
* **Zero Side-Effects Staging**: Backups are written to a dedicated internal index (`.git/endur_index`). Your primary Git staging index (`git status`) remains entirely unaffected.
* **OS-Level Service Management**: Integrates natively with `launchd` on macOS, `systemd` on Linux, and Windows Task Scheduler on Windows to run silently on startup.
* **Control Center TUI**: Includes a full-featured terminal dashboard (`endur tui`) for repository management, snapshot logging, and interactive side-by-side diff restores.
* **Performance Scraper**: Scrapes background logs to plot latency trends and insertions/deletions.

---

## Installation

### Prerequisite: Git
Ensure `git` is installed and available in your `$PATH`.

### Install via Cargo
```bash
cargo install endur
```

This installs the `endur` executable into your local Cargo binary folder (usually `~/.cargo/bin`). Make sure this path is in your environment's `$PATH`.

---

## Quickstart

### 1. Register & Start the Startup Service
To start the daemon automatically on logon:
```bash
endur service install
```
This registers the appropriate service config (`launchd` plist on macOS, `systemd` user service on Linux, or Scheduled Task on Windows) and launches the daemon in the background.

To stop and remove the background service:
```bash
endur service uninstall
```

### 2. Monitor a Repository
Navigate to any active Git repository and start backing it up:
```bash
endur watch
```
You can also watch specific paths or define inclusion/exclusion directories.

### 3. Interactive TUI Control Center
Launch the terminal dashboard to manage repositories, preview diffs, and select files to restore:
```bash
endur tui
```

### 4. CLI Recovery & Restore
List recent snapshot backups taken since your last formal Git commit:
```bash
endur list-snapshots
```

Restore the entire working directory to a specific snapshot hash:
```bash
endur restore <commit-hash>
```

Restore only specific files or folders from a snapshot (discrete restore):
```bash
endur restore <commit-hash> --files path/to/file1.txt path/to/dir/
```

### 5. Pruning Old Backups
To reclaim space, prune older snapshots by keeping only those matching the last `N` formal commits:
```bash
endur prune --keep 5
```
You can also pass the `--gc` flag to trigger Git Garbage Collection (`git gc --prune=now`) immediately after pruning.

---

## Under the Hood

### Storage Isolation
Endur creates backups by committing snapshots directly to a parallel git branch named `endur/<base-commit-hash>`, where `<base-commit-hash>` is the hash of the latest formal commit in your active branch. This ensures that:
* No commits are ever added to your active development branch history.
* The branch is kept fully localized to your machine (never pushed to remote origins).
* Your backup history is easily prunable since it lives on separate branches.

### SQL Metadata Cache
To optimize response times in the GUI and TUI, Endur caches snapshot records in a SQLite database located at `~/.cache/endur/snapshot_cache.db`. If the cache database is missing or corrupt, it automatically regenerates or falls back to standard Git rev-walking dynamically.

---

## Documentation & GUI Client
For more information, developer guides, and the native Tauri-based Desktop application client, visit the [Endur Main Repository](https://github.com/PJC-64/endur).
