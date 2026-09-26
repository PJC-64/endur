# Endur Release Notes (v1.2.2)

We are excited to announce the release of **Endur** (v1.2.2). This release brings a wealth of visual enhancements, cross-platform service installer improvements, custom premium modal dialogs in the desktop application, and automated security dependency auditing in our CI/CD pipelines.

---

## 🚀 Key Highlights

### 1. Premium Visual Metrics & Performance Charts (GUI & TUI)
* **Interactive SVG Charts (GUI)**: Added custom SVG-based Activity and Latency charts to the metrics view, styled with a modern cyan glow. Users can hover over specific days or latency points to view tooltips with detailed performance statistics.
* **TUI Sparkline Charts (Terminal)**: Integrated visual Sparkline indicators directly into the `endur tui` console metrics page, giving terminal users an instant graphical overview of backup latency and repository activity.
* **Auto-refresh Controls**: Added simple toggle and refresh actions to update active stats on demand.

### 2. Custom Glassmorphic Overlay Dialog Modals (GUI)
* **Svelte 5 Dialogs**: Replaced standard browser `alert`, `confirm`, and `prompt` dialogs in the desktop application with custom-built premium `<dialog>` overlays.
* **Rich Aesthetics**: Crafted with a dark translucent slate backdrop (`blur(16px)`) and subtle border gradients to harmonize with the main desktop dashboard interface.
* **Light-Dismiss & Esc support**: Out-of-the-box keyboard focus trapping, standard `Esc` key cancellations, and a light-dismiss backdrop gesture fallback to support Safari and other WebKit-based rendering engines.

### 3. Native Windows Task Scheduler Integration & Silent Daemon (CLI & GUI)
* **Windows Task Scheduler Service**: Extended the `endur service` installer on Windows to register the backup daemon using `schtasks.exe` rather than the registry startup key. This registers the daemon as a true system service starting automatically on logon (`/sc onlogon`) under the current user's profile context (no Administrator/UAC prompts required).
* **Silent Daemon Startup**: Automatically intercepts console window creation on Windows when starting the daemon via `endur serve`, programmatically hiding the terminal console window immediately.

### 4. Service Control & TUI Console Silencing
* **Service Shutdown Fix (macOS)**: Patched the macOS launchctl KeepAlive plist parameters so that stopping the service from the GUI or CLI leaves it stopped, rather than immediately triggering a system relaunch.
* **TUI Breakthrough Output Suppression**: Silenced breakthrough prints from service installation and subprocess commands that previously corrupted Ratatui layouts on macOS and Linux. Standard output and error buffers are flushed cleanly before and after descriptor redirection.

### 5. Automated Security CI Checks & Hardened Dependencies
* **Automated Security Pipeline**: Integrated `actions-rust-lang/audit` checks into our GitHub Actions workflow, raising PR blockages if vulnerable dependencies are detected.
* **Vulnerable Dependency Resolution**: Upgraded critical crates to resolve security alerts:
  * `crossbeam-epoch` (RUSTSEC-2026-0204): Upgraded to `v0.9.20`.
  * `quick-xml` (RUSTSEC-2026-0195, RUSTSEC-2026-0194): Upgraded to `v0.41.0`.
* **Exclusion Profile**: Setup `.cargo/audit.toml` to safely ignore vendor-patched vulnerabilities (e.g. `RUSTSEC-2024-0429`).
* **Clippy Warnings**: Cleaned up new Clippy compiler warnings to ensure warning-free builds on the latest Rust toolchains.

---
*Note: This fork is actively maintained and has been built with AI pair-programming assistance.*
