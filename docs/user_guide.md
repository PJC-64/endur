# Dura User Guide

Dura is a background daemon that monitors your active Git repositories and automatically backs up uncommitted changes. By utilizing filesystem events and keeping its operations strictly isolated, Dura ensures that you never lose unsaved code due to a computer crash, accidental deletion, or editor state loss.

---

## Key Features

1. **Zero-Configuration Backups**: Simply run `dura serve` and `dura watch` a repository.
2. **Instant Event-Driven Capture**: Dura uses native filesystem events (`notify` crate) to immediately capture snapshots of your changes with a 500ms debounce window. No CPU-heavy polling loops.
3. **Smart Git Filtering**: Respects your `.gitignore` rules and ignores the `.git/` folder automatically.
4. **Zero-Side-Effects Staging**: Uses an isolated index (`.git/dura_index`) so your primary Git index (`git status` and staging) is never touched during backups.
5. **Built-in CLI Recovery**: Restore files directly using native CLI recovery commands without writing complex Git plumbing commands.

---

## Installation

### Prerequisite: Rust and Cargo
Ensure you have the Rust toolchain installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from Source
From the `new-dura` directory:
```bash
cargo install --path .
```

This compiles the release binary and installs it to your local Cargo bin directory (usually `~/.cargo/bin`). Make sure this directory is in your `$PATH`.

---

## Command Reference

### 1. Start the Daemon (`serve`)
Dura runs as a background process. To start the daemon, run:
```bash
dura serve &
```
*   **Log Customization**: By default, `dura serve` logs to standard output. Use `--logfile <FILE>` to log to a file instead:
    ```bash
    dura serve --logfile ~/.cache/dura/dura.log &
    ```

### 2. Monitor a Repository (`watch`)
To add a repository to the watch list, navigate to its directory and run:
```bash
dura watch
```
You can also specify a directory path:
```bash
dura watch /path/to/my/project
```

### 3. Stop Monitoring a Repository (`unwatch`)
To stop backing up a repository:
```bash
dura unwatch
```
Or specify the directory path:
```bash
dura unwatch /path/to/my/project
```

### 4. Check Daemon Status & configuration (`info`)
To print the current configuration, list of watched repositories, and daemon status, run:
```bash
dura info
```
Use `--detail` for more detailed information:
```bash
dura info --detail
```

### 5. Stop the Daemon (`kill`)
To safely stop the background daemon process:
```bash
dura kill
```

---

## Recovery & Restore Guide

When you modify files, Dura instantly commits changes to a branch specific to your current HEAD commit. If your current HEAD is `a1b2c3d...`, Dura commits to a local branch named `dura/a1b2c3d...`.

### Listing Snapshots
To see all local backup snapshots for your repository, run:
```bash
dura list-snapshots
```
Example Output:
```
Dura Snapshots for repository: /Users/pjc/Development/project
Commit Hash                              Date/Time                 Changes
--------------------------------------------------------------------------------
4a5b6c7d8e9f...                          2026-05-27 08:35:10       2 files
1a2b3c4d5e6f...                          2026-05-27 08:20:00       1 files
```

### Restoring a Snapshot
To restore your working directory and staging index to the state captured in a specific snapshot, copy the snapshot's commit hash and run:
```bash
dura restore <commit-hash>
```
Example:
```bash
$ dura restore 4a5b6c7d8e9f
Restored working directory and index to snapshot 4a5b6c7d8e9f:
  M  src/main.rs
  A  tests/new_test.rs
```

#### What happens during a restore?
*   **Safe checkout**: The files matching the snapshot are checked out into your working directory, overwriting current unstaged changes.
*   **Staged changes**: Because the files match the snapshot, Git will show the differences between your `HEAD` and the restored files as **staged changes** (`git status` shows them as changes to be committed).
*   **Detached HEAD is avoided**: The active HEAD pointer and branch (e.g., `main` or `feature-xyz`) are not changed. You remain on your current branch.
