# Endur User Guide

Endur is a background daemon that monitors your active Git repositories and automatically backs up uncommitted changes. By utilizing filesystem events and keeping its operations strictly isolated, Endur ensures that you never lose unsaved code due to a computer crash, accidental deletion, or editor state loss.

---

## Key Features

1. **Zero-Configuration Backups**: Simply run `endur serve` and `endur watch` a repository.
2. **Instant Event-Driven Capture**: Endur uses native filesystem events (`notify` crate) to immediately capture snapshots of your changes with a 500ms debounce window. No CPU-heavy polling loops.
3. **Smart Git Filtering**: Respects your `.gitignore` rules and ignores the `.git/` folder automatically.
4. **Zero-Side-Effects Staging**: Uses an isolated index (`.git/endur_index`) so your primary Git index (`git status` and staging) is never touched during backups.
5. **Built-in CLI Recovery**: Restore files directly using native CLI recovery commands without writing complex Git plumbing commands.
6. **SQLite Metadata Cache**: Snapshot metadata is cached in `~/.cache/endur/snapshot_cache.db` for fast lookups. The cache is kept warm on every backup and falls back silently to a raw Git walk if missing or corrupt.

---

## Installation

### Prerequisite: Rust and Cargo (For CLI/TUI)
Ensure you have the Rust toolchain installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Install CLI/TUI via Cargo (crates.io)
You can install the published crate directly from crates.io:
```bash
cargo install endur
```

### Build CLI/TUI from Source
From the `endur` directory:
```bash
cargo install --path .
```
This compiles the release binary and installs it to your local Cargo bin directory (usually `~/.cargo/bin`). Make sure this directory is in your `$PATH`.

### Install Endur Desktop GUI (macOS, Windows, Linux)
For users who prefer a graphical interface, Endur provides a native cross-platform desktop application:
*   **macOS**: Packaged inside a `.dmg` installer.
*   **Windows**: Packaged as a `.msi` or `.exe` installer.
*   **Linux**: Packaged as a `.deb` package or standalone `.AppImage`.

Simply download the appropriate installer from the [GitHub Releases page](https://github.com/PJC-64/endur/releases), run the installer to set up the GUI app, and launch it from your applications menu.

---

## Endur Desktop GUI Application

The Endur Desktop App is built on **Tauri 2.0** and **Svelte 5** to expose the same backup engine and daemon metrics as the CLI, but in a premium, glassmorphic graphical dashboard.

### Key Features
1. **Interactive Daemon Controller**: One-click launch, restart, and termination of the background daemon with live status badges (Active/Inactive), Process ID display, and real-time uptime calculations.
2. **Watchlist Management**: Add or remove Git repositories from the watch list using a text input field, showing paths in a clean scrollable directory view.
3. **Recovery Pane (Side-by-Side Diff Preview)**:
   * Select a repository and browse its snapshots. By default, only snapshots taken **since your last formal Git commit** are shown, keeping the list focused on your current work-in-progress. Click the **📌 HEAD / 🕰 All** toggle button to switch between the filtered and full-history views.
   * Inspect the exact list of modified files recorded in each snapshot.
   * View full side-by-side git patch diff previews with insertions and deletions highlighted.
   * Select specific files via checkboxes to run a **discrete restore**, or click **Restore All** to revert the entire repository state.
4. **Performance Analytics Tab**: Displays real-time metrics, average/maximum backup latency stats, and snapshot throughput tables directly inside the app window.
5. **Live Log Console**: Streams human-readable background daemon logs in real-time with the most recent entries displayed at the top, matching the filtering rules of the CLI Control Center (filtering out redundant check loops to prevent clutter).

### Under the Hood: Hybrid Daemon Resolution
To avoid software conflicts and duplicate processes, the GUI implements a **hybrid daemon resolver**:
1. **Existing Daemon Check**: When starting the daemon, the GUI checks if a global `endur` CLI daemon is already installed (`~/.cargo/bin/endur` or in PATH). If present, the GUI binds to and controls that CLI daemon.
2. **Bundled Daemon Fallback**: If no global CLI daemon is found, the GUI starts its own bundled `endur` core engine. Both engines share the same cache directory (`~/.cache/endur`), ensuring that command-line tools, statuslines, and GUI views remain completely synchronized.

---


## Command Reference

### 1. Start the Daemon (`serve`)
Endur runs as a background process. To start the daemon, run:
```bash
endur serve &
```
*   **Log File**: By default, `endur serve` produces absolutely no output to `stdout` or `stderr`, instead automatically logging to a file named `endur.log` inside your Endur cache home directory (e.g. `~/.cache/endur/endur.log` on macOS/Linux). You can configure a custom log location with the `--logfile <FILE>` option:
    ```bash
    endur serve --logfile /path/to/custom.log &
    ```

### 2. Monitor a Repository (`watch`)
To add a repository to the watch list, navigate to its directory and run:
```bash
endur watch
```
You can also specify a directory path:
```bash
endur watch /path/to/my/project
```

### 3. Stop Monitoring a Repository (`unwatch`)
To stop backing up a repository:
```bash
endur unwatch
```
Or specify the directory path:
```bash
endur unwatch /path/to/my/project
```

### 4. Check Daemon Status & configuration (`info`)
To print the current configuration, list of watched repositories, and daemon status, run:
```bash
endur info
```
Use `--detail` for more detailed information:
```bash
endur info --detail
```

### 5. Stop the Daemon (`kill`)
To safely stop the background daemon process:
```bash
endur kill
```

### 6. Version and Features (`-v` / `--version`)
To output detailed version and configuration info:
```bash
endur -v
```
This prints the package version, compiled features (TUI backend, IPC format, lock strategy), and path locations of both Endur config and cache directories.

### 7. Clean Up Configuration (`cleanup`)
To remove any invalid or inaccessible repositories (e.g., deleted folders, non-git directories, or directories with permission errors) from your watch list, run:
```bash
endur cleanup
```
This automatically updates your Endur configuration and notifies the background daemon to reload.

### 8. System Startup Service (`service`)
You can configure Endur to start automatically when you log in (macOS) or boot the system (Linux) by installing it as a user-level startup service.

*   **Install Service**:
    ```bash
    endur service install
    ```
    This writes the appropriate configuration (`launchd` plist on macOS, `systemd` user service on Linux) and loads/starts the background service.

*   **Uninstall Service**:
    ```bash
    endur service uninstall
    ```
    This stops the background service and deletes the plist/service configuration files.

### 9. Control Center TUI (`tui`)
To open the comprehensive interactive TUI Control Center, run:
```bash
endur tui
```
This launches a full-screen interface featuring four tabbed panes:
1. **[1] Repositories**: Lists all watched repositories. Allows adding new repository paths (press `a`), stopping watching a repository (press `d`), or running a watchlist cleanup (press `c`).
2. **[2] Backups & Restore**: Browse through snapshots, toggle file selections using `Space`, view real-time git diff previews, and trigger full or selective restores.
3. **[3] Full Log**: View a scrollable list of the running daemon log file.
4. **[4] Metrics**: Displays a clean, aligned table of snapshot metrics (latency, hashes, insertions, deletions) and overall performance statistics (average/max latencies).

*   **Controls**:
    *   Switch tabs using keys `1`, `2`, `3`, `4` or by pressing `Tab`.
    *   Use arrow keys (`Up` / `Down` / `Left` / `Right`) to navigate lists.
    *   On the **Metrics** tab, scroll through the metrics list using `Up` and `Down` arrow keys.
    *   Press `q` to quit the Control Center.

### 10. Performance Metrics (`metrics`)
To scrape logs and generate performance metrics (such as snapshot backup latency, counts of changed files, insertions, and deletions), run:
```bash
endur metrics
```
*   **Log Input**: By default, `endur metrics` checks if standard input is a terminal. If it is run interactively on a TTY, it automatically reads from the default cached daemon log file (`~/.cache/endur/endur.log`). Otherwise, it expects log lines piped through standard input (e.g. `cat endur.log | endur metrics`). You can also specify an input file using `-i <FILE>`.
*   **JSON Output (Default)**: By default, it outputs raw JSON data suitable for post-processing and automation tools.
*   **Human-Readable Table (`-h` / `--human-readable`)**: Provide the `-h` or `--human-readable` flag to format the snapshot statistics into a cleanly aligned text table and print a performance summary including total snapshots, average latency, and max latency:
    ```bash
    endur metrics -h
    ```

---

## Recovery & Restore Guide

When you modify files, Endur instantly commits changes to a branch specific to your current HEAD commit. If your current HEAD is `a1b2c3d...`, Endur commits to a local branch named `endur/a1b2c3d...`.

### Listing Snapshots
To see backup snapshots for your repository, run:
```bash
endur list-snapshots
```

By default, Endur only shows snapshots taken **after your most recent formal Git commit** — that is, the work-in-progress you haven't committed yet. This keeps the list short and immediately relevant.

To see every snapshot Endur has ever recorded for this repository:
```bash
endur list-snapshots --all
```

Example Output:
```
Endur Snapshots for repository: /Users/pjc/Development/project
Commit Hash                              Date/Time                 Changes
--------------------------------------------------------------------------------
4a5b6c7d8e9f...                          2026-05-27 08:35:10       2 files
1a2b3c4d5e6f...                          2026-05-27 08:20:00       1 files
```

### Restoring a Snapshot
To restore your working directory and staging index to the state captured in a specific snapshot, copy the snapshot's commit hash and run:
```bash
endur restore <commit-hash>
```

#### Discrete Path Restore
If you want to restore only specific files or folders from a snapshot rather than the entire repository, specify the `--files` (or `-f`) flag:
```bash
endur restore <commit-hash> --files path/to/file1.txt path/to/dir/
```
Only the specified files/directories will be reverted to the snapshot's state; other modifications in your working directory will remain untouched.

#### Interactive Mode (TUI)
For a visual selection interface, run:
```bash
endur restore -i
```
This launches an interactive Terminal User Interface (TUI) with a multi-step selection flow:
1. **Repository Selection**:
   * Use `Up` / `Down` to navigate the list of watched repositories on the left. A preview of backups is shown on the right.
   * Press `Enter` (or `Right`/`Tab`) to select the highlighted repository. This transitions to the Snapshot view.
2. **Backup/Snapshot Selection**:
   * The left pane now shows the list of snapshots/backups for the selected repository (filtered to **since the last HEAD commit** by default). The right pane displays a preview of modified files in the highlighted snapshot.
   * Use `Up` / `Down` to navigate snapshots.
   * Press `A` to toggle between **Since HEAD** (default) and **All** historical snapshots. The panel title updates to reflect the active filter.
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

### 1. Main Executable Help (`endur --help`)
```text
Endur backs up your work automatically via Git commits.

Usage: endur [COMMAND]

Commands:
  capture, -C, --capture  Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.
  info, -I, --info        Prints summary information about the current configuration and repository status.
  serve, -S, --serve      Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.
  watch, -W, --watch      Add the current working directory as a repository to watch.
  unwatch, -U, --unwatch  Remove the current working directory as a repository to watch.
  kill, -K, --kill        Stop the running worker (should only be a single worker).
  metrics, -M, --metrics  Convert logs into richer metrics about snapshots.
  list-snapshots          List all local endur backup snapshots.
  restore                 Restore files from a specific endur backup snapshot.
  cleanup                 Remove any inaccessible or invalid repositories from the watch list.
  service                 Manage endur background service
  tui                     Interactive control center for daemon monitoring and snapshot management.
  help                    Print this message or the help of the given subcommand(s)

Options:
  -v, --version  Print version information
  -h, --help     Print help
```

### 2. Capture Command Help (`endur capture --help`)
```text
Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.

Usage: endur {capture|--capture|-C} [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -h, --help  Print help
```

### 3. Info Command Help (`endur info --help`)
```text
Prints summary information about the current configuration and repository status.

Usage: endur {info|--info|-I} [OPTIONS]

Options:
  -d, --detail  Show detailed output
  -h, --help    Print help
```

### 4. Serve Command Help (`endur serve --help`)
```text
Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.

Usage: endur {serve|--serve|-S} [OPTIONS]

Options:
      --logfile <FILE>  Sets custom logfile. Default is logging to stdout
  -h, --help            Print help
```

### 5. Watch Command Help (`endur watch --help`)
```text
Add the current working directory as a repository to watch.

Usage: endur {watch|--watch|-W} [OPTIONS] [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -i, --include [<include>...]  Overrides excludes by re-including specific directories relative to the watch directory.
  -e, --exclude [<exclude>...]  Excludes specific directories relative to the watch directory
  -d, --maxdepth [<maxdepth>]   Determines the depth to recurse into when scanning directories [default: 255]
  -h, --help                    Print help
```

### 6. Unwatch Command Help (`endur unwatch --help`)
```text
Remove the current working directory as a repository to watch.

Usage: endur {unwatch|--unwatch|-U} [directory]

Arguments:
  [directory]  The directory to watch. Defaults to current directory

Options:
  -h, --help  Print help
```

### 7. Kill Command Help (`endur kill --help`)
```text
Stop the running worker (should only be a single worker).

Usage: endur {kill|--kill|-K}

Options:
  -h, --help  Print help
```

### 8. List Snapshots Command Help (`endur list-snapshots --help`)
```text
List all local endur backup snapshots.

Usage: endur list-snapshots [OPTIONS] [directory]

Arguments:
  [directory]  The directory to inspect. Defaults to current directory

Options:
  -a, --all   Show all snapshots, including those predating the current HEAD commit
  -h, --help  Print help
```

### 9. Restore Command Help (`endur restore --help`)
```text
Restore files from a specific endur backup snapshot.

Usage: endur restore [OPTIONS] [hash] [directory]

Arguments:
  [hash]       The commit hash of the snapshot to restore
  [directory]  The directory to watch. Defaults to current directory

Options:
  -i, --interactive      Interactive mode using TUI
  -f, --files <FILE>...  Specific files or directories to restore
  -h, --help             Print help
```

### 10. Service Command Help (`endur service --help`)
```text
Manage endur background service

Usage: endur service [COMMAND]

Commands:
  install    Install endur as a system startup service (launchd on macOS, systemd on Linux)
  uninstall  Uninstall endur startup service
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### 11. Metrics Command Help (`endur metrics --help`)
```text
Convert logs into richer metrics about snapshots.

Usage: endur {metrics|--metrics|-M} [OPTIONS]

Options:
      --help            Print help
  -i, --input <FILE>    The log file to read. Defaults to stdin.
  -o, --output <FILE>   The json file to write. Defaults to stdout.
  -h, --human-readable  Print metrics in a human-readable table and summary format
```

### 12. TUI Command Help (`endur tui --help`)
```text
Interactive control center for daemon monitoring and snapshot management.

Usage: endur tui

Options:
  -h, --help  Print help
```
