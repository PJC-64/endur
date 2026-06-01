# Durable Developer Guide

This document explains the internal architecture, design patterns, and code structure of `durable`. It is intended for developers who wish to modify, debug, or extend the codebase.

---

## System Architecture

The following diagram illustrates the relationship between the CLI client, the background Daemon, the filesystem, and the Git repository:

```mermaid
graph TD
    Client[CLI Client: durable watch/list-snapshots/restore]
    Daemon[Daemon: durable serve]
    LockFile[Lock File: durable.lock]
    UDS[Unix Domain Socket: UDS IPC]
    FS[File System]
    Index[Isolated Git Index: durable_index]
    Repo[Git Repository]

    Client -->|IPC Commands| UDS
    UDS -->|RPC handler| Daemon
    Daemon -->|Exclusive Advisory Lock| LockFile
    Daemon -->|File Events| FS
    FS -->|Notify Event Channel| Daemon
    Daemon -->|Stage changes| Index
    Daemon -->|Commit to durable/HEAD branch| Repo
```

---

## Core Components

### 1. Command-Line Interface ([src/main.rs](file:///Users/pjc/Development/durable/src/main.rs))
*   **Subcommand Routing**: Parses arguments using `clap` and routes them.
*   **Daemon Notification**: Subcommands like `watch` and `unwatch` perform the local configuration changes and notify the running daemon using the `poller::send_uds_command` helper.
*   **Direct Command execution**: Core recovery tasks like `list-snapshots` and `restore` execute directly in the client context by parsing the Git repository, avoiding UDS overhead.

### 2. Runtime Locking & Single Instance enforcement ([src/database.rs](file:///Users/pjc/Development/durable/src/database.rs))
*   **`RuntimeLock` struct**: Enforces that only one daemon instance is active.
*   **Exclusive Advisory Locking**: Utilizes `fs2::FileExt::try_lock_exclusive` to acquire an OS-level lock on the file `~/.cache/durable/durable.lock`.
*   **Auto-Cleanup**: The lock is automatically released by the operating system when the daemon process terminates.

### 3. Client-Server IPC ([src/poller.rs](file:///Users/pjc/Development/durable/src/poller.rs))
*   **Unix Domain Sockets**: The daemon binds to `~/.cache/durable/durable.sock` using `tokio::net::UnixListener`.
*   **JSON-RPC Protocol**: Communication uses simple JSON payloads:
    *   `reload`: Reloads configuration when watched repositories change.
    *   `kill`: Triggers a graceful shutdown of the daemon.
    *   `status`: Responds with daemon metrics.

### 4. File Watcher & Filter Engine ([src/watcher.rs](file:///Users/pjc/Development/durable/src/watcher.rs))
*   **`WatcherManager` struct**: Manages recursive file system monitoring using the `notify` crate.
*   **Path Canonicalization**: Filesystem events on macOS and Linux often report relative paths or symlink roots (like `/var` vs `/private/var`). The watcher canonicalizes all event paths prior to comparison.
*   **Git-Aware Filtering**:
    *   Excludes `.git/` folder and its contents automatically.
    *   Loads `.gitignore` files using `ignore::gitignore::Gitignore` to match change events against gitignore rules and discard ignored files.

### 5. Daemon Control Loop ([src/poller.rs](file:///Users/pjc/Development/durable/src/poller.rs))
*   **Event Await & Debounce**: The daemon loop listens to three event sources using `tokio::select!`:
    *   *IPC Command Channel*: Process socket operations.
    *   *File Watcher Channel*: Pushes modified files. Captured file changes are cached in a hash map with a 500ms delay.
    *   *Timer Channel*: Triggers a capture operation for repositories that have modifications older than 500ms.
*   **Optimization**: Debouncing prevents multiple rapid writes (e.g., compilation artifacts or quick editor saves) from creating dozens of intermediate commits.

### 6. Isolated Snapshot Capture ([src/snapshots.rs](file:///Users/pjc/Development/durable/src/snapshots.rs))
*   **`snapshots::capture`**: Stages changes and commits them.
*   **Git Index Isolation**: Rather than using the user's primary Git index (`.git/index`), it creates and maintains a dedicated staging index file at `.git/durable_index` via `git2::Index::open`.
*   **Branch Architecture**:
    *   Backups are committed to local branches named `durable/<base-commit-hash>`.
    *   The parent of the first durable snapshot is the user's HEAD commit. Subsequent backups chain off the previous durable backup commit, keeping histories completely linear.

### 7. Interactive TUI Restore ([src/tui.rs](file:///Users/pjc/Development/durable/src/tui.rs))
*   **`TuiState`**: State machine maintaining lists of active watched repositories, snapshots for the selected repo, navigation list indices, and current panel focus (`Repos` or `Snapshots`).
*   **Terminal Interface**: Powered by `ratatui` with `crossterm`. Employs a split layout showing repositories on the left (40% width) and snapshots on the right (60% width), using border color highlights (Green vs Dark Gray) to clearly show panel focus.
*   **Safe Terminal RAII**: An RAII wrapper `TerminalGuard` manages raw mode, alternate screen buffers, and cursor visibility, ensuring the terminal is always cleanly restored on program exit or panic.

### 8. Background Service Management ([src/service.rs](file:///Users/pjc/Development/durable/src/service.rs))
*   **macOS (`launchctl` Integration)**: Creates `~/Library/LaunchAgents/com.durable.daemon.plist` configured to run `durable serve` in the background, and registers it using the modern `launchctl bootstrap` framework (falling back to `launchctl load` if necessary).
*   **Linux (`systemd` Integration)**: Configures a systemd user unit at `~/.config/systemd/user/durable.service` and executes systemd commands to daemon-reload, enable, and start/stop the service.

---

## Development & Testing Workflow

### Compiling
```bash
cargo build
```

### Running Tests
The test suite consists of several test suites covering locking, IPC, watcher, and snapshotting:
```bash
cargo test
```

### Key Integration Tests
*   [tests/startup_test.rs](file:///Users/pjc/Development/durable/tests/startup_test.rs): Tests UDS communication, invalid lock files, and double-lock prevention.
*   [tests/watch_test.rs](file:///Users/pjc/Development/durable/tests/watch_test.rs): Tests watcher registration and event-driven backup snapshots.
*   [tests/snapshots_test.rs](file:///Users/pjc/Development/durable/tests/snapshots_test.rs): Tests Git index isolation, git ignore support, listing snapshots, and restoring snapshots.
*   [tests/poll_guard_test.rs](file:///Users/pjc/Development/durable/tests/poll_guard_test.rs): Tests poller lock mechanics and state changes.
