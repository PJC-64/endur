# Endur Release Notes (v1.2.3)

We are pleased to announce the release of **Endur** (v1.2.3). This release focuses on internal architecture refactoring to resolve circular dependency cycles, enhanced Neovim statusline contextual indicators, multi-picker support for Snacks.nvim and fzf-lua, and cleaner codebase indexing.

---

## 🚀 Key Highlights

### 1. Architectural Cycle Breaking & Decoupling
* **Eliminated Circular Dependencies**: Resolved all module-level circular dependency warnings detected during codebase graph analysis:
  * **`config` $\longleftrightarrow$ `git_repo_iter`**: Inlined the recursive directory iterator `GitRepoIter` directly into `config.rs`, removing the unnecessary standalone helper module.
  * **`cache` $\longleftrightarrow$ `snapshots`**: Extracted the shared `SnapshotInfo` struct into a lightweight, standalone data-transfer module (`snapshot_info.rs`). Both `cache.rs` and `snapshots.rs` now import cleanly from `snapshot_info`, eliminating the mutual dependency.
* **Graph Cleanliness**: Zero circular dependencies remain in the codebase.

### 2. Neovim Integration (`endur-nvim`) Improvements
* **Contextual Repository Name in Statusline**: Enhanced `M.statusline()`, `M.statusline_raw()`, and `M.print_status()` to parse and display the active Git repository folder name (e.g. `Endur[my-project] Clean (3)` rather than the generic `Endur Clean (3)`), making multi-buffer and multi-repo workflows clearer.
* **Snacks.nvim Integration Fixes**:
  * Corrected the Snacks custom picker preview callback signature to accept a single `ctx` parameter, reading `ctx.buf` and `ctx.item.hash` properly for real-time diff previews.
  * Fixed the `<C-f>` (`restore_file`) action callback signature to take `(picker, item)` with a fallback to `picker:current()`.
* **Additional Pickers & Controls**: Exposed dedicated commands for `:EndurSnapshotsSnacks`, `:EndurSnapshotsFzf`, and `:EndurTUI` terminal launching.

### 3. Repository Graph & Tooling Hardening
* **`.graphifyignore` Support**: Introduced a `.graphifyignore` configuration file to exclude the vendored glib patch directory (`patches/glib-0.18.5/`) from code indexing, keeping architecture graph models clean and focused purely on core application logic.

### 4. Registry Publishing
* **crates.io Release**: Successfully verified, packaged, and published the `endur` crate (v1.2.3) to the crates.io registry.

---
*Note: This fork is actively maintained and has been built with AI pair-programming assistance.*
