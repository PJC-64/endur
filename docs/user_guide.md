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

#### Discrete Path Restore
If you want to restore only specific files or folders from a snapshot rather than the entire repository, specify the `--files` (or `-f`) flag:
```bash
durable restore <commit-hash> --files path/to/file1.txt path/to/dir/
```
Only the specified files/directories will be reverted to the snapshot's state; other modifications in your working directory will remain untouched.

#### Interactive Mode (TUI)
For a visual selection interface, run:
```bash
durable restore -i
```
This launches an interactive Terminal User Interface (TUI) with a multi-step selection flow:
1. **Repository Selection**:
   * Use `Up` / `Down` to navigate the list of watched repositories on the left. A preview of backups is shown on the right.
   * Press `Enter` (or `Right`/`Tab`) to select the highlighted repository. This transitions to the Snapshot view.
2. **Backup/Snapshot Selection**:
   * The left pane now shows the list of snapshots/backups for the selected repository. The right pane displays a preview of modified files in the highlighted snapshot.
   * Use `Up` / `Down` to navigate snapshots.
   * Press `Enter` to restore the **entire** highlighted snapshot.
   * Press `Esc` or `Backspace` or `Left` to go back to the Repository Selection screen.
   * Press `Right` (or `Tab`) to switch focus to the **Changed Files** list on the right.
3. **Changed Files Selection (Discrete Restore)**:
   * The right pane is now active. Use `Up` / `Down` to navigate through the list of modified files.
   * Press `Space` to toggle checkbox selection `[x]` on individual files.
   * Press `Enter` to restore **only the selected files** (or the highlighted file, if no checkmarks are active).
   * Press `Left` or `Esc` or `Backspace` or `Tab` to switch focus back to the Backup/Snapshot list on the left.
*   At any screen, press `q` to exit the TUI without restoring.

#### What happens during a restore?
*   **Safe checkout**: The files matching the snapshot are checked out into your working directory, overwriting current unstaged changes.
*   **Staged changes**: Because the files match the snapshot, Git will show the differences between your `HEAD` and the restored files as **staged changes** (`git status` shows them as changes to be committed).
*   **Detached HEAD is avoided**: The active HEAD pointer and branch (e.g., `main` or `feature-xyz`) are not changed. You remain on your current branch.

---

## Appendix: CLI Help Reference

### 1. Main Executable Help (`durable --help`)
```text
Durable backs up your work automatically via Git commits.

Usage: durable [COMMAND]

Commands:
  capture, -C, --capture  Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.
  info, -I, --info        Prints summary information about the current configuration and repository status.
  serve, -S, --serve      Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.
  watch, -W, --watch      Add the current working directory as a repository to watch.
  unwatch, -U, --unwatch  Remove the current working directory as a repository to watch.
  kill, -K, --kill        Stop the running worker (should only be a single worker).
  metrics, -M, --metrics  Convert logs into richer metrics about snapshots.
  list-snapshots          List all local durable backup snapshots.
  restore                 Restore files from a specific durable backup snapshot.
  cleanup                 Remove any inaccessible or invalid repositories from the watch list.
  service                 Manage durable background service
  help                    Print this message or the help of the given subcommand(s)

Options:
  -v, --version  Print version information
  -h, --help     Print help
```

### 2. Capture Command Help (`durable capture --help`)
```text
Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.

Usage: durable {capture|--capture|-C} [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -h, --help  Print help
```

### 3. Info Command Help (`durable info --help`)
```text
Prints summary information about the current configuration and repository status.

Usage: durable {info|--info|-I} [OPTIONS]

Options:
  -d, --detail  Show detailed output
  -h, --help    Print help
```

### 4. Serve Command Help (`durable serve --help`)
```text
Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.

Usage: durable {serve|--serve|-S} [OPTIONS]

Options:
      --logfile <FILE>  Sets custom logfile. Default is logging to stdout
  -h, --help            Print help
```

### 5. Watch Command Help (`durable watch --help`)
```text
Add the current working directory as a repository to watch.

Usage: durable {watch|--watch|-W} [OPTIONS] [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -i, --include [<include>...]  Overrides excludes by re-including specific directories relative to the watch directory.
  -e, --exclude [<exclude>...]  Excludes specific directories relative to the watch directory
  -d, --maxdepth [<maxdepth>]   Determines the depth to recurse into when scanning directories [default: 255]
  -h, --help                    Print help
```

### 6. Unwatch Command Help (`durable unwatch --help`)
```text
Remove the current working directory as a repository to watch.

Usage: durable {unwatch|--unwatch|-U} [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -h, --help  Print help
```

### 7. Kill Command Help (`durable kill --help`)
```text
Stop the running worker (should only be a single worker).

Usage: durable {kill|--kill|-K}

Options:
  -h, --help  Print help
```

### 8. List Snapshots Command Help (`durable list-snapshots --help`)
```text
List all local durable backup snapshots.

Usage: durable list-snapshots [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -h, --help  Print help
```

### 9. Restore Command Help (`durable restore --help`)
```text
Restore files from a specific durable backup snapshot.

Usage: durable restore [OPTIONS] [hash] [directory]

Arguments:
  [hash]       The commit hash of the snapshot to restore
  [directory]  The directory to watch. Defaults to current directory

Options:
  -i, --interactive      Interactive mode using TUI
  -f, --files <FILE>...  Specific files or directories to restore
  -h, --help             Print help
```

### 10. Service Command Help (`durable service --help`)
```text
Manage durable background service

Usage: durable service [COMMAND]

Commands:
  install    Install durable as a system startup service (launchd on macOS, systemd on Linux)
  uninstall  Uninstall durable startup service
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```
