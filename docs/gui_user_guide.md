# Endur GUI User Guide

Endur Desktop is a premium, cross-platform graphical application built on Tauri 2.0 and Svelte 5. It provides a visual dashboard for monitoring background backups, traversing repositories, previewing changes, and restoring historical snapshots.

---

## 1. Installation

### macOS
1. Download the `.dmg` installer from the [GitHub Releases](https://github.com/PJC-64/endur/releases) page.
2. Double-click the `.dmg` file.
3. Drag **Endur Desktop** into your `/Applications` folder.
4. Launch the app from your Applications menu or Spotlight.

### Windows
1. Download the `.msi` or `.exe` installer.
2. Run the installer and follow the setup wizard.
3. Launch **Endur Desktop** from the Start Menu.

### Linux
1. Download the `.deb` package or `.AppImage`.
2. For Debian/Ubuntu-based distributions:
   ```bash
   sudo dpkg -i endur-desktop_*.deb
   ```
3. For standalone AppImage, make the file executable and run it:
   ```bash
   chmod +x EndurDesktop.AppImage
   ./EndurDesktop.AppImage
   ```

---

## 2. Navigating the Dashboard

The application dashboard uses a dark theme with glassmorphism panels. It is divided into four main sections, accessible via the sidebar navigation:

1. **Dashboard**: Displays background daemon status, direct process controls, and system startup service management controls side-by-side.
2. **Watchlist**: Shows currently monitored directories and provides a graphical explorer to browse and watch new Git repositories.
3. **Backups & Restore**: Displays snapshot history, lists changed files, shows side-by-side git diffs, and provides selective/full restore actions.
4. **Analytics**: Renders a premium metrics dashboard including summary statistics cards (Total Snapshots, Watched Repos, Lines Changed, Avg Latency), custom high-fidelity SVG-based charts (Backup Activity stacked bars and Backup Latency area charts), and a performance statistics table.

---

## 3. Managing the Daemon (Direct vs. System Service)

Endur runs as a background process to watch your filesystem. You can control how the daemon runs via the **Daemon Service** tab:

### Direct Process Tab
- **Direct Mode**: Runs the daemon directly as a child process of the GUI or links to an already running CLI daemon instance.
- **Start/Stop Process**: Launch or terminate the daemon process with one click.
- **Service Detection**: If Endur detects that a system service is already running, the Direct Process tab disables duplicate launching controls and shows a status message redirecting you to manage the service.

### System Service Tab
- **Deprecated Background Processes**: Endur favors OS-level services for background execution. The System Service tab registers the daemon directly into the OS service manager.
- **Actions**:
  - **Install**: Installs the configuration (`launchd` plist on macOS, `systemd` user unit on Linux) to automatically start the daemon when you log into your system.
  - **Uninstall**: Deregisters the service.
  - **Start**: Tells the OS service manager to start the daemon service in the background.
  - **Stop**: Terminates the running service.
- **GUI Independence**: The GUI is fully decoupled from the system service. Starting, stopping, or installing the service will not restart the GUI or disrupt your current views.
- **Running Version**: Displays the exact binary version currently running as a system service.

### Startup Mode Auto-Switching
On startup, the GUI automatically queries the status of the daemon and system service. If the system service is active and running, the GUI automatically switches to **Service** management mode. If the daemon is running directly (and not as a service), it switches to **Direct** management mode. If neither is running, it defaults to your last chosen configuration (saved in browser `localStorage`). You can manually switch between modes at any time.

---

## 4. Watchlist Management & Directory Browser

The **Watchlist** tab is organized into a two-column layout:

### Left Column: Active Repositories
- Displays the list of all directory paths currently monitored by the background engine.
- Click **Unwatch** on any row to stop backup snapshots for that repository.

### Right Column: Interactive Repository Browser
- **Configurable Default Root (`base_root`)**: The browser automatically opens in your designated development directory. By default, this is `~/Development`. You can configure this directory by adding the following line to your `~/.config/endur/config.json` file:
  ```json
  {
    "base_root": "~/MyDevelopmentFolder"
  }
  ```
- **Git-Restricted Traversal**: To optimize performance and keep the interface focused, directory trees are filtered. Only subdirectories containing active Git repositories (up to 5 folders deep) are displayed.
- **Navigation Controls**:
  - **📁 Folder Rows**: Click any folder row to enter and browse its contents.
  - **↱ Up**: Navigate to the parent directory.
  - **🏠 Base**: Instantly jump back to your configured `base_root` folder.
  - **Watch Current**: If the folder you are currently inside is a Git repository, click the **Watch Current** button at the top to add it.
  - **Watch**: Click the **Watch** button next to any nested Git repository folder row to register it.
- **Fallback Manual Input**: If you need to watch a path outside your configured root folder, enter the absolute path manually into the input box at the bottom of the card and click **Watch Folder**.

---

## 5. Snapshot Recovery & Patch Previews

Restore files easily in the **Backups & Restore** tab:

1. **Select Repository**: Use the drop-down selector to choose which watched repository you want to restore from.
2. **Browse Snapshots**: The left pane lists available snapshots.
   - **Filtered History (HEAD Mode)**: By default, only backups taken *since your last formal Git commit* are shown to focus on your latest work-in-progress.
   - **Full History (All Mode)**: Click the **📌 HEAD / 🕰 All** toggle to view all backups recorded for the repository.
3. **Inspect Modified Files**: Click a snapshot hash row to view the list of files modified during that snapshot. Each file will show a status code:
   - <span style="color: #10b981; font-weight: bold;">[A]</span>: Added
   - <span style="color: #3b82f6; font-weight: bold;">[M]</span>: Modified
   - <span style="color: #ef4444; font-weight: bold;">[D]</span>: Deleted
4. **Side-by-Side Diff Preview**: Click on any file name to open the diff viewer. It displays a standard unified patch diff highlighting insertions in green and deletions in red.
5. **Recovery Options**:
   - **Discrete File Restore**: Select specific files using checkboxes, and click **Restore Selected** to revert only those files.
   - **Full Restore**: Click **Restore All** to revert the entire repository state back to that snapshot.

---

## 6. Performance Analytics

The **Analytics** tab provides a real-time visual dashboard of snapshot backup performance and activity history across all monitored repositories:

*   **Summary Cards**: Quick-glance counters at the top displaying:
    *   **Total Snapshots**: Total number of backups recorded.
    *   **Watched Repos**: Count of active watched repositories.
    *   **Total Lines Changed**: Accumulated insertions (`(+)`) and deletions (`(-)`) across history.
    *   **Average Latency**: Mean time taken to execute a snapshot commit.
*   **Recent Backup Activity Chart**: A custom SVG stacked bar chart graphing lines changed per snapshot. Green bars represent insertions, and red bars represent deletions. Hovering over a bar reveals the exact repository and lines changed details.
*   **Backup Latency Trend**: A custom SVG line/area chart graphing snapshot execution durations. Blue nodes map latency over the last backups, with hover actions displaying elapsed times.
*   **Snapshot Performance Table**: A tabular presentation of all snapshot metadata, letting you scroll and inspect the date, repository path, file changes, latency, and git commit hashes.
