# Durable User Guide

Durable is a background daemon that monitors your active Git repositories and automatically backs up uncommitted changes. By utilizing filesystem events and keeping its operations strictly isolated, Durable ensures that you never lose unsaved code due to a computer crash, accidental deletion, or editor state loss.

---

## Key Features

1. **Zero-Configuration Backups**: Simply run `durable serve` and `durable watch` a repository.
2. **Instant Event-Driven Capture**: Durable uses native filesystem events (`notify` crate) to immediately capture snapshots of your changes with a 500ms debounce window. No CPU-heavy polling loops.
3. **Smart Git Filtering**: Respects your `.gitignore` rules and ignores the `.git/` folder automatically.
4. **Zero-Side-Effects Staging**: Uses an isolated index (`.git/durable_index`) so your primary Git index (`git status` and staging) is never touched during backups.
5. **Built-in CLI Recovery**: Restore files directly using native CLI recovery commands without writing complex Git plumbing commands.

---

## Installation

### Prerequisite: Rust and Cargo
Ensure you have the Rust toolchain installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from Source
From the `new-durable` directory:
```bash
cargo install --path .
```

This compiles the release binary and installs it to your local Cargo bin directory (usually `~/.cargo/bin`). Make sure this directory is in your `$PATH`.

---

## Command Reference

### 1. Start the Daemon (`serve`)
Durable runs as a background process. To start the daemon, run:
```bash
durable serve &
```
*   **Log File**: By default, `durable serve` produces absolutely no output to `stdout` or `stderr`, instead automatically logging to a file named `durable.log` inside your Durable cache home directory (e.g. `~/.cache/durable/durable.log` on macOS/Linux). You can configure a custom log location with the `--logfile <FILE>` option:
    ```bash
    durable serve --logfile /path/to/custom.log &
    ```

### 2. Monitor a Repository (`watch`)
To add a repository to the watch list, navigate to its directory and run:
```bash
durable watch
```
You can also specify a directory path:
```bash
durable watch /path/to/my/project
```

### 3. Stop Monitoring a Repository (`unwatch`)
To stop backing up a repository:
```bash
durable unwatch
```
Or specify the directory path:
```bash
durable unwatch /path/to/my/project
```

### 4. Check Daemon Status & configuration (`info`)
To print the current configuration, list of watched repositories, and daemon status, run:
```bash
durable info
```
Use `--detail` for more detailed information:
```bash
durable info --detail
```

### 5. Stop the Daemon (`kill`)
To safely stop the background daemon process:
```bash
durable kill
```

### 6. Version and Features (`-v` / `--version`)
To output detailed version and configuration info:
```bash
durable -v
```
This prints the package version, compiled features (TUI backend, IPC format, lock strategy), and path locations of both Durable config and cache directories.

### 7. Clean Up Configuration (`cleanup`)
To remove any invalid or inaccessible repositories (e.g., deleted folders, non-git directories, or directories with permission errors) from your watch list, run:
```bash
durable cleanup
```
This automatically updates your Durable configuration and notifies the background daemon to reload.

### 8. System Startup Service (`service`)
You can configure Durable to start automatically when you log in (macOS) or boot the system (Linux) by installing it as a user-level startup service.

*   **Install Service**:
    ```bash
    durable service install
    ```
    This writes the appropriate configuration (`launchd` plist on macOS, `systemd` user service on Linux) and loads/starts the background service.

*   **Uninstall Service**:
    ```bash
    durable service uninstall
    ```
    This stops the background service and deletes the plist/service configuration files.

---

## Recovery & Restore Guide

When you modify files, Durable instantly commits changes to a branch specific to your current HEAD commit. If your current HEAD is `a1b2c3d...`, Durable commits to a local branch named `durable/a1b2c3d...`.

### Listing Snapshots
To see all local backup snapshots for your repository, run:
```bash
durable list-snapshots
```
Example Output:
```
Durable Snapshots for repository: /Users/pjc/Development/project
Commit Hash                              Date/Time                 Changes
--------------------------------------------------------------------------------
4a5b6c7d8e9f...                          2026-05-27 08:35:10       2 files
1a2b3c4d5e6f...                          2026-05-27 08:20:00       1 files
```

### Restoring a Snapshot
To restore your working directory and staging index to the state captured in a specific snapshot, copy the snapshot's commit hash and run:
```bash
durable restore <commit-hash>
```

#### Interactive Mode (TUI)
For a visual selection interface, run:
```bash
durable restore -i
```
This launches a Terminal User Interface (TUI):
*   Use `Left` / `Right` arrow keys to switch focus between the **Repositories** panel and the **Backups** panel.
*   Use `Up` / `Down` arrow keys to navigate the highlighted list.
*   Press `Enter` on a backup snapshot to restore it.
*   Press `Esc` or `q` to exit the TUI without restoring.

#### What happens during a restore?
*   **Safe checkout**: The files matching the snapshot are checked out into your working directory, overwriting current unstaged changes.
*   **Staged changes**: Because the files match the snapshot, Git will show the differences between your `HEAD` and the restored files as **staged changes** (`git status` shows them as changes to be committed).
*   **Detached HEAD is avoided**: The active HEAD pointer and branch (e.g., `main` or `feature-xyz`) are not changed. You remain on your current branch.
