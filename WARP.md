# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Endur is a background process that watches Git repositories and automatically commits uncommitted changes to special `endur/*` branches without impacting HEAD, the current branch, or the Git index. It's a Rust-based tool that provides "undo" across an entire repository by snapshotting changes as-you-go.

## Common Commands

### Building and Testing
```bash
# Run all tests (both unit and integration)
cargo test

# Run linting
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt

# Pre-commit checks (runs tests, clippy, and fmt)
./scripts/pre-commit.sh
```

### Running the Application
```bash
# Build and install locally
cargo install --path .

# Run the background service
endur serve &

# Watch a repository
cd /path/to/repo
endur watch

# Stop the service
endur kill

# Capture a single snapshot
endur capture [directory]

# View configuration and status
endur info
endur info --detail
```

### Development with Nix
```bash
# Run development version
nix run github:PJC-64/endur

# Enter development shell
nix develop github:PJC-64/endur
```

## Architecture

### Core Components

**Control Loop (`poller.rs`)**
- Main background loop that processes events
- Iterates through configured repositories using `GitRepoIter`
- Uses `PollGuard` optimization to check file timestamps before attempting Git operations
- Calls `snapshots::capture()` for changed directories

**Snapshot System (`snapshots.rs`)**
- Creates commits on special `endur/<HEAD-hash>` branches
- Uses libgit2 to stage all changes (tracked and untracked)
- Only commits if there are actual changes detected
- Returns `CaptureStatus` with branch name, commit hash, and base hash

**Configuration (`config.rs`)**
- Stored in `~/.config/endur/config.toml` (or `$ENDUR_CONFIG_HOME/config.toml`)
- Tracks watched repositories with their `WatchConfig` (includes/excludes, max_depth)
- Supports custom commit author/email settings
- Can exclude Git config via `commit_exclude_git_config`

**Runtime State (`database.rs`)**
- `RuntimeLock` database at `~/.cache/endur/runtime.db` and lock file at `runtime.lock` (or `$ENDUR_CACHE_HOME/runtime.db` and `runtime.lock`)
- Tracks running poller's PID to prevent multiple instances
- Used by `endur kill` to signal shutdown

**Repository Discovery (`git_repo_iter.rs`)**
- Iterator that recursively walks watched directories
- Respects include/exclude patterns and max_depth from `WatchConfig`
- Returns only Git repository paths (detected by presence of `.git`)

**Poll Guard Optimization (`poll_guard.rs`)**
- Fast-path check using file modification timestamps
- Compares against last known commit time (from endur branch or HEAD)
- Only triggers full Git operations if files modified > 1 second after watermark
- Caches Git repository handles to avoid repeated opens

### Data Flow

1. `poller::start()` initializes runtime lock with current PID
2. Control loop waits for filesystem events and IPC socket messages
3. For each watched repo in config:
   - `PollGuard::dir_changed()` checks file timestamps
   - If changed: `snapshots::capture()` creates commit on `endur/<hash>` branch
   - Logs operation metrics (latency, status)
4. Loop repeats until shutdown signal is received

## Testing

### Test Structure
- **Unit tests**: Inside source files in `#[cfg(test)]` modules - test internal logic without filesystem
- **Integration tests**: In `/tests` directory - use real filesystem and Git operations
- Key integration tests:
  - `startup_test.rs` - good place for testing new functionality
  - `snapshots_test.rs` - snapshot creation logic
  - `watch_test.rs` - repository watching
  - `poll_guard_test.rs` - optimization logic

### Test Utilities (`tests/util/`)
- `git_repo` - creates temp Git repositories for parallel test execution
- `endur` - spawns real endur subprocess with isolated `$ENDUR_CACHE_HOME` and `$ENDUR_CONFIG_HOME`
- `daemon` - facilitates working with `endur serve` via blocking `read_line()`

## Environment Variables

- `ENDUR_CONFIG_HOME` - Override config directory (default: `~/.config/endur`)
- `ENDUR_CACHE_HOME` - Override cache directory (default: `~/.cache/endur`)
- `ENDUR_VERSION_SUFFIX` - Appends to version string
- `ENDUR_PLAIN_TEXT` - Use plain ASCII symbols instead of Unicode
- `ENDUR_FANCY` - Force Unicode symbols
- `NO_COLOR` - Disable Unicode symbols (follows standard)
- `RUST_LOG` - Control logging level (e.g., `RUST_LOG=debug`)

## Output Conventions

- All structured output (logs) goes to `stdout` as JSON
- User messages go to `stderr` as plain text
- Use serialized structs for JSON logs to maintain backward compatibility
- Do not rename JSON fields that may be used in user scripts
