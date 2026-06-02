# Durable

[![Build][build badge]][build action]

> [!NOTE]
> **Durable** is an actively maintained fork of Tim Kellogg's original [dura](https://github.com/tkellogg/dura) project. Since the fork, this project is being developed and built with AI assistance (leveraging Antigravity, Google DeepMind's coding assistant). It contains significant modernizations, performance improvements, and feature additions.

Durable is a background process that watches your Git repositories and commits your uncommitted changes without impacting
HEAD, the current branch, or the Git index (staged files). If you ever get into an "oh snap!" situation where you think
you just lost days of work, checkout a `durable` branch and recover.

Without `durable`, you use Ctrl-Z in your editor to get back to a good state. That's so 2021. Computers crash and Ctrl-Z
only works on files independently. Durable snapshots changes across the entire repository as-you-go, so you can revert to
"4 hours ago" instead of "hit Ctrl-Z like 40 times or whatever". Finally, some sanity.

## Key Enhancements & Differences from Original `dura`

Compared to the upstream `dura` repository, **Durable** adds the following features and improvements:

1. **Strict Advisory File Locking**: Replaced PID tracking files with OS-level advisory file locks (`fs2` crate) for single-instance daemon enforcement.
2. **Unix Domain Sockets (UDS) IPC**: Replaced the native TCP loopback setup with a robust, asynchronous Unix Domain Socket communication layer.
3. **Event-Driven & Debounced File Watching**: Replaced CPU-heavy active directory polling with native, event-driven file monitoring (`notify` crate) combined with a 500ms debounce cache.
4. **Git-Aware Filtering**: Automatically respects `.gitignore` rules and ignores the `.git/` folder, reducing disk write and commit activity.
5. **Discrete Path Restore**: Allows you to restore only specific files or folders from a backup snapshot (e.g. `durable restore <hash> --files path/to/file`) instead of checking out the entire repository tree.
6. **Built-in CLI Recovery & Interactive TUI**:
   * `durable list-snapshots`: Lists all snapshots with files changed and date/time.
   * `durable restore -i`: Visual Terminal User Interface (TUI) powered by `ratatui` to browse repositories, drill down into snapshots, preview modified files, and selectively restore files using interactive checkboxes (`Space` to toggle, `Enter` to restore).
7. **System Startup Service Subcommands**: Exposes `durable service install` and `durable service uninstall` to register the daemon as a system service automatically (`launchd` on macOS, `systemd` on Linux).
8. **Configurable Log Redirection**: The `durable serve` daemon runs completely silently, logging only to a configurable file path (defaults to `~/.cache/durable/durable.log`).

## Documentation

*   [User Guide](docs/user_guide.md): Comprehensive instructions on using and recovering with Durable.
*   [Developer Guide](docs/developer_guide.md): Details on the internal architecture, event-driven loop, UDS socket IPC, and testing.

## How to use

Run it in the background:

```bash
$ durable serve &
```

The `serve` can happen in any directory. The `&` is Unix shell syntax to run the process in the background, meaning that you can start
`durable` and then keep using the same terminal window while `durable` keeps running. You could also run `durable serve` in a
window that you keep open.

Let `durable` know which repositories to watch:

```bash
$ durable watch some/git/repo
```

You can pass a relative or absolute path to the directory you want to watch. If no path is specified, it defaults to the current working directory.

To watch all git repositories under a specific folder (e.g. your development directory), you can run:

```bash
$ find ~/Development -type d -name .git -prune | xargs -I{} sh -c "durable watch {}/.."
```

Make some changes. No need to commit or even stage them. Use any Git tool to see the `durable` branches:

```bash
$ git log --all
```

`durable` produces a branch for every real commit you make and makes commits to that branch without impacting your working
copy. You keep using Git exactly as you did before.


Let `durable` know that it should stop running in the background with the `kill` command.

```bash
$ durable kill
```

The `kill` can happen in any directory. It indicates to the `serve`
process that it should exit if there is a `serve` process running.

## How to recover

Durable now provides built-in recovery subcommands:

1. **List all snapshots**:
   ```bash
   $ durable list-snapshots
   ```
   This displays all snapshots in the repository with their timestamps and files changed.

2. **Restore files from a snapshot**:
   *   To restore the entire repository state:
       ```bash
       $ durable restore <commit-hash>
       ```
    *   To restore only specific files or directories (discrete restore):
        ```bash
        $ durable restore <commit-hash> --files path/to/file1.txt path/to/dir/
        ```
    *   To use the visual interactive mode (supports full or selective checkbox-based restore):
        ```bash
        $ durable restore -i
        ```
    These options check out the snapshot files directly to your working directory and staging index, keeping you on your current branch.

For more details on commands and recovery, see the [User Guide](docs/user_guide.md).

## Install

### Cargo Install
1. Install Cargo  
2. To install the release version, run:
   ```bash
   cargo install durable
   ```

### By Source

1. Install Rust (e.g., `brew install rustup && brew install rust`)
2. Clone this repository:
   ```bash
   git clone https://github.com/PJC-64/durable.git
   ```
3. Navigate to repository base directory (`cd durable`)
4. Run:
   ```bash
   cargo install --path .
   ```

### macOS & Linux (Startup Service)

Durable can be configured as a background service that launches automatically at login (macOS) or boot (Linux) using the built-in subcommand:

```bash
$ durable service install
```

To stop and remove the service, run:

```bash
$ durable service uninstall
```

### Windows
1. Download [rustup-init](https://www.rust-lang.org/tools/install)
2. Clone this repository:
   ```bash
   git clone https://github.com/PJC-64/durable.git
   ```
3. Navigate to repository base directory (`cd durable`)
4. Run `cargo install --path .` **Note:** If you receive a failure fetching the cargo dependencies try using the local [git client for cargo fetches](https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli). `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --path .`

### Arch Linux

```bash
$ paru -S durable-git
```

### Nix / Nixos

[Nix][nix website] is a tool that takes a unique approach to package
management and system configuration. NixOS is a Linux distribution
built on top of the Nix package manager.

To run `durable` locally using pre-compiled binaries:

```bash
nix shell nixpkgs#durable
```

If you're willing to contribute and develop, `durable` also provides its
own ready-to-use [Nix flake][nix flake].

To build and run the latest development version of `durable` locally:

```bash
nix run github:PJC-64/durable
```

To run a development environment with the required tools
to develop:

```bash
nix develop github:PJC-64/durable
```

## FAQ

### Is this stable?

Yes. Lots of people have been using it since 2022-01-01 without issue. It uses [libgit2](https://libgit2.org/) to make the commits, so it's fairly battle hardened.

### How often does this check for changes?

Durable uses event-driven file monitoring (`notify` crate) to listen for filesystem events. It captures changes immediately after they occur, with a 500ms debounce delay to combine rapid consecutive writes into a single snapshot.


Brought to you by <a rel="nofollow me" href="https://hachyderm.io/@kellogh">Tim Kellogg</a>.


[build badge]: https://github.com/PJC-64/durable/actions/workflows/build.yaml/badge.svg
[build action]: https://github.com/PJC-64/durable/actions/workflows/build.yaml
[nix website]: https://nixos.org/
[nix flake]: https://nixos.wiki/wiki/Flakes
