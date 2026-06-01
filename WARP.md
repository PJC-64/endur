# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Durable is a background process that watches Git repositories and automatically commits uncommitted changes to special `durable/*` branches without impacting HEAD, the current branch, or the Git index. It's a Rust-based tool that provides "undo" across an entire repository by snapshotting changes every ~5 seconds.

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
durable serve &

# Watch a repository
cd /path/to/repo
durable watch

# Stop the service
durable kill

# Capture a single snapshot
durable capture [directory]

# View configuration and status
durable info
durable info --detail
```

### Development with Nix
```bash
# Run development version
nix run github:tkellogg/durable

# Enter development shell
nix develop github:tkellogg/durable
```

## Architecture

### Core Components

**Control Loop (`poller.rs`)**
- Main background loop that runs every 5 seconds
- Iterates through configured repositories using `GitRepoIter`
- Uses `PollGuard` optimization to check file timestamps before attempting Git operations
- Calls `snapshots::capture()` for changed directories

**Snapshot System (`snapshots.rs`)**
- Creates commits on special `durable/<HEAD-hash>` branches
- Uses libgit2 to stage all changes (tracked and untracked)
- Only commits if there are actual changes detected
- Returns `CaptureStatus` with branch name, commit hash, and base hash

**Configuration (`config.rs`)**
- Stored in `~/.config/durable/config.toml` (or `$DURABLE_CONFIG_HOME/config.toml`)
- Tracks watched repositories with their `WatchConfig` (includes/excludes, max_depth)
- Supports custom commit author/email settings
- Can exclude Git config via `commit_exclude_git_config`

**Runtime State (`database.rs`)**
- `RuntimeLock` stored in `~/.cache/durable/runtime.db` (or `$DURABLE_CACHE_HOME/runtime.db`)
- Tracks running poller's PID to prevent multiple instances
- Used by `durable kill` to signal shutdown

**Repository Discovery (`git_repo_iter.rs`)**
- Iterator that recursively walks watched directories
- Respects include/exclude patterns and max_depth from `WatchConfig`
- Returns only Git repository paths (detected by presence of `.git`)

**Poll Guard Optimization (`poll_guard.rs`)**
- Fast-path check using file modification timestamps
- Compares against last known commit time (from durable branch or HEAD)
- Only triggers full Git operations if files modified > 1 second after watermark
- Caches Git repository handles to avoid repeated opens

### Data Flow

1. `poller::start()` initializes runtime lock with current PID
2. Control loop sleeps 5 seconds between iterations
3. For each watched repo in config:
   - `PollGuard::dir_changed()` checks file timestamps
   - If changed: `snapshots::capture()` creates commit on `durable/<hash>` branch
   - Logs operation metrics (latency, status)
4. Loop repeats until PID in runtime lock changes (killed)

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
- `durable` - spawns real durable subprocess with isolated `$DURA_HOME`
- `daemon` - facilitates working with `durable serve` via blocking `read_line()`

## Environment Variables

- `DURABLE_CONFIG_HOME` - Override config directory (default: `~/.config/durable`)
- `DURABLE_CACHE_HOME` - Override cache directory (default: `~/.cache/durable`)
- `DURABLE_VERSION_SUFFIX` - Appends to version string
- `DURABLE_PLAIN_TEXT` - Use plain ASCII symbols instead of Unicode
- `DURABLE_FANCY` - Force Unicode symbols
- `NO_COLOR` - Disable Unicode symbols (follows standard)
- `RUST_LOG` - Control logging level (e.g., `RUST_LOG=debug`)

## Output Conventions

- All structured output (logs) goes to `stdout` as JSON
- User messages go to `stderr` as plain text
- Use serialized structs for JSON logs to maintain backward compatibility
- Do not rename JSON fields that may be used in user scripts
