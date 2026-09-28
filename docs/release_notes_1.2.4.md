# Endur Release Notes (v1.2.4)

We are pleased to announce the release of **Endur** (v1.2.4). This release delivers key improvements for Windows background service management from the TUI, desktop dialog safety and frontend type-checking cleanliness, along with CI and dependency updates.

---

## 🚀 Key Highlights

### 1. Windows TUI Service Management Fixes
* **Silent Execution**: Background scheduled task operations (`schtasks_cmd`) now execute using `CREATE_NO_WINDOW` on Windows with captured output (`.output()`), preventing transient cmd prompt windows from interrupting the user experience.
* **Low-Level Output Silencer**: Implemented a Windows-specific `OutputSilencer` utilizing Win32 `GetStdHandle` and `SetStdHandle` redirected to `NUL`, ensuring service status inquiries and registrations do not corrupt interactive TUI rendering.

### 2. Desktop Application & Frontend Type Safety
* **Modal Dialog Null-Safety**: Fixed `dialogElement` null-check assertions in the Tauri desktop frontend (`+page.svelte`).
* **Clean Frontend Type-Checking**: Integrated `@types/node` in development dependencies and cleared obsolete TypeScript directives in `vite.config.js`, achieving a clean build and type-check with 0 errors and 0 warnings across the entire web interface.

### 3. CI/CD & Dependency Upgrades
* **GitHub Actions**: Upgraded `actions/setup-node` to `@v7` and `tauri-apps/tauri-action` to `@v1`.
* **Frontend Dependencies**: Bumped `@sveltejs/kit` to `2.70.1` and `postcss` to `8.5.26`.
* **Documentation**: Completed release notes for previous minor milestones and synchronized user and developer guides.

---
*Note: This fork is actively maintained and has been built with AI pair-programming assistance.*
