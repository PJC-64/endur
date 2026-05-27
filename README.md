# Dura

[![Build][build badge]][build action]

Dura is a background process that watches your Git repositories and commits your uncommitted changes without impacting
HEAD, the current branch, or the Git index (staged files). If you ever get into an "oh snap!" situation where you think
you just lost days of work, checkout a `dura` branch and recover.

Without `dura`, you use Ctrl-Z in your editor to get back to a good state. That's so 2021. Computers crash and Ctrl-Z
only works on files independently. Dura snapshots changes across the entire repository as-you-go, so you can revert to
"4 hours ago" instead of "hit Ctrl-Z like 40 times or whatever". Finally, some sanity.

## Documentation

*   [User Guide](docs/user_guide.md): Comprehensive instructions on using and recovering with Dura.
*   [Developer Guide](docs/developer_guide.md): Details on the internal architecture, event-driven loop, UDS socket IPC, and testing.

## How to use

Run it in the background:

```bash
$ dura serve &
```

The `serve` can happen in any directory. The `&` is Unix shell syntax to run the process in the background, meaning that you can start
`dura` and then keep using the same terminal window while `dura` keeps running. You could also run `dura serve` in a
window that you keep open.

Let `dura` know which repositories to watch:

```bash
$ cd some/git/repo
$ dura watch
```

Right now, you have to `cd` into each repo that you want to watch, one at a time.

If you have thoughts on how to do this better, share them [here](https://github.com/tkellogg/dura/issues/3). Until that's sorted, you can
run something like `find ~ -type d -name .git -prune | xargs -I= sh -c "cd =/..; dura watch"` to get started on your existing repos.

Make some changes. No need to commit or even stage them. Use any Git tool to see the `dura` branches:

```bash
$ git log --all
```

`dura` produces a branch for every real commit you make and makes commits to that branch without impacting your working
copy. You keep using Git exactly as you did before.


Let `dura` know that it should stop running in the background with the `kill` command.

```bash
$ dura kill
```

The `kill` can happen in any directory. It indicates to the `serve`
process that it should exit if there is a `serve` process running.

## How to recover

Dura now provides built-in recovery subcommands:

1. **List all snapshots**:
   ```bash
   $ dura list-snapshots
   ```
   This displays all snapshots in the repository with their timestamps and files changed.

2. **Restore files from a snapshot**:
   ```bash
   $ dura restore <commit-hash>
   ```
   This checks out the snapshot files directly to your working directory and staging index, keeping you on your current branch.

For more details on commands and recovery, see the [User Guide](docs/user_guide.md).

## Install

### Cargo Install
1. Install Cargo  
2. If you want run release version, type ```cargo install dura``` else type ```cargo install --git https://github.com/tkellogg/dura```

### By Source

1. Install Rust (e.g., `brew install rustup && brew install rust`)
2. Clone this repository (e.g., `git clone https://github.com/tkellogg/dura.git`)
3. Navigate to repository base directory (`cd dura`)
4. Run `cargo install --path .` **Note:** If you receive a failure fetching the cargo dependencies try using the local [git client for cargo fetches](https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli). `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --path .`

### Mac OS X

This installs `dura` and sets up a launchctl service to keep it running.

```bash
$ brew install dura
```

### Windows
1. Download [rustup-init](https://www.rust-lang.org/tools/install)
2. Clone this repository (e.g., `git clone https://github.com/tkellogg/dura.git`)
3. Navigate to repository base directory (`cd dura`)
4. Run `cargo install --path .` **Note:** If you receive a failure fetching the cargo dependencies try using the local [git client for cargo fetches](https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli). `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --path .`

### Arch Linux

```bash
$ paru -S dura-git
```

### Nix / Nixos

[Nix][nix website] is a tool that takes a unique approach to package
management and system configuration. NixOS is a Linux distribution
built on top of the Nix package manager.

To run `dura` locally using pre-compiled binaries:

```bash
nix shell nixpkgs#dura
```

If you're willing to contribute and develop, `dura` also provides its
own ready-to-use [Nix flake][nix flake].

To build and run the latest development version of `dura` locally:

```bash
nix run github:tkellogg/dura
```

To run a development environment with the required tools
to develop:

```bash
nix develop github:tkellogg/dura
```

## FAQ

### Is this stable?

Yes. Lots of people have been using it since 2022-01-01 without issue. It uses [libgit2](https://libgit2.org/) to make the commits, so it's fairly battle hardened.

### How often does this check for changes?

Dura uses event-driven file monitoring (`notify` crate) to listen for filesystem events. It captures changes immediately after they occur, with a 500ms debounce delay to combine rapid consecutive writes into a single snapshot.


Brought to you by <a rel="nofollow me" href="https://hachyderm.io/@kellogh">Tim Kellogg</a>.


[build badge]: https://github.com/tkellogg/dura/actions/workflows/build.yaml/badge.svg
[build action]: https://github.com/tkellogg/dura/actions/workflows/build.yaml
[nix website]: https://nixos.org/
[nix flake]: https://nixos.wiki/wiki/Flakes
