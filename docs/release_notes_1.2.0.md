# Endur Release Notes (v1.2.0)

We are proud to announce the release of **Endur** (v1.2.0). This release is packed with major usability enhancements, including the deprecation of raw background processes in favor of OS-level system services, fully integrated graphical and terminal service managers, and interactive git-repository-aware filesystem browsers.

---

## 🚀 Key Highlights

### 1. Interactive Repository Selector (GUI & TUI)
* **Directory Tree Traversal**: Replaced manual absolute path inputs with an interactive file browser. Users can now easily traverse directory trees visually.
* **Git-Restricted Filtering**: To prevent clutter and optimize lookup speeds on large development environments, the browser filters out non-Git directories and recursive walks are restricted to a maximum depth of 5 folders.
* **Configurable Default Root (`base_root`)**: Traversal automatically defaults to your active development area. A configurable `base_root` setting in `config.json` (e.g. `"base_root": "~/Development"`) dictates the starting directory.
* **Up / Base Actions**: Navigate to the parent directory (`↱ Up`) or jump directly back to your default root (`🏠 Base`) with one key or click.

### 2. Native OS Service Management (GUI & TUI)
* **Integrated Service Panel**: Directly control OS-level startup services (`launchd` on macOS, `systemd` on Linux) from the user interface.
* **Service Lifecycle Controls**: Install, uninstall, start, and stop system services with a single click or keyboard command.
* **Running Service Version**: Displays the exact version of the active background system service inside the status panels.
* **Direct Mode Service Detection**: The direct process control panel automatically detects if Endur is active as a system service, displaying a warning note and locking duplicate process execution.
* **Startup Mode Auto-Switching**: Automatically detects the active daemon mode on TUI and GUI startup, switching to Service management mode if the system service is running, or Direct mode if only the direct process is active.
* **Clean Service Reinstallation**: Running the service install command when a service is already installed now automatically stops and uninstalls the old service version first, then registers and starts the latest version cleanly.
* **GUI Decoupling**: Completely separated the GUI application process from the background service, ensuring that installing, starting, or stopping the system service never causes the GUI to restart or launch duplicate windows.

### 3. Stability & Developer Experience
* **OS-Level Signal Compatibility**: Enhanced background daemon shutdown loops to handle native shutdown signals cleanly.
* **Robust File Handling**: Resolved edge-case file watching bugs and cleaned up unused package configurations.
* **Expanded Documentation**: Added new dedicated User and Developer Guides specifically detailing the Tauri 2.0 and Svelte 5 GUI stack.

---
*Note: This fork is actively maintained and has been built with AI pair-programming assistance.*
