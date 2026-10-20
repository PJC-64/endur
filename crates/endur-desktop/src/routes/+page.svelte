<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  import CustomDialog, { type CustomDialogState } from "$lib/components/CustomDialog.svelte";
  import LogViewer from "$lib/components/LogViewer.svelte";
  import RepoList from "$lib/components/RepoList.svelte";
  import SnapshotView from "$lib/components/SnapshotView.svelte";
  import MetricsView from "$lib/components/MetricsView.svelte";

  // Tab State
  let activeTab: "dashboard" | "repos" | "backups" | "metrics" = $state("dashboard");

  // Daemon Status State
  let daemonStatus = $state({ running: false, pid: null, uptime_secs: null, version: null, client_version: "" });

  // Watchlist State
  let watchedRepos: string[] = $state([]);
  let watchlistError = $state("");
  let watchlistMessage = $state("");

  // File Selector State
  let selectorCurrentDir = $state("");
  let selectorEntries: { name: string, path: string, is_repo: boolean }[] = $state([]);
  let selectorLoading = $state(false);
  let selectorError = $state("");
  let baseRootDir = $state("");
  let isCurrentDirRepo = $state(false);

  // Backups State
  let selectedRepo = $state("");
  let snapshots: any[] = $state([]);
  let selectedSnapshot: any = $state(null);
  let snapshotFiles: [string, string][] = $state([]);
  let selectedFiles: Set<string> = $state(new Set());
  let activeFileDiff = $state("");
  let selectedFileForDiff = $state("");
  let backupMessage = $state("");
  let backupError = $state("");
  let showAllSnapshots = $state(false);

  // Custom Dialog Modal State
  let customDialog: CustomDialogState = $state({
    show: false,
    type: "alert",
    title: "",
    message: "",
    inputValue: "",
    placeholder: "",
    resolve: null
  });

  function showCustomAlert(message: string, title = "Notification"): Promise<void> {
    return new Promise((resolve) => {
      customDialog = {
        show: true,
        type: "alert",
        title,
        message,
        inputValue: "",
        placeholder: "",
        resolve: () => {
          customDialog.show = false;
          resolve();
        }
      };
    });
  }

  function showCustomConfirm(message: string, title = "Confirm Action"): Promise<boolean> {
    return new Promise((resolve) => {
      customDialog = {
        show: true,
        type: "confirm",
        title,
        message,
        inputValue: "",
        placeholder: "",
        resolve: (result: boolean) => {
          customDialog.show = false;
          resolve(result);
        }
      };
    });
  }

  function showCustomPrompt(message: string, title = "Input Required", placeholder = ""): Promise<string | null> {
    return new Promise((resolve) => {
      customDialog = {
        show: true,
        type: "prompt",
        title,
        message,
        inputValue: "",
        placeholder,
        resolve: (result: string | null) => {
          customDialog.show = false;
          resolve(result);
        }
      };
    });
  }

  // Service Management State
  let managementMode: "direct" | "service" = $state("direct");
  let serviceStatus = $state({ installed: false, running: false });
  let serviceActionLoading = $state(false);

  async function loadServiceStatus() {
    try {
      const installed = await invoke("is_service_installed");
      const running = await invoke("is_service_running");
      serviceStatus = { installed: installed as boolean, running: running as boolean };
    } catch (e) {
      console.error("Failed to load service status", e);
    }
  }

  async function handleServiceAction(action: "install" | "uninstall" | "start" | "stop") {
    try {
      serviceActionLoading = true;
      await invoke("control_service", { action });
      await loadServiceStatus();
      await loadDaemonStatus();
    } catch (e: any) {
      await showCustomAlert(`Error performing action '${action}' on service: ` + e, "Service Action Error");
    } finally {
      serviceActionLoading = false;
    }
  }

  function setManagementMode(mode: "direct" | "service") {
    managementMode = mode;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("endur_mgmt_mode", mode);
    }
    if (mode === "service") {
      loadServiceStatus();
    }
  }

  // Metrics State
  let metricsText = $state("");
  let rawMetrics: any[] = $state([]);

  // Real-time Logs State
  let logsText = $state("");

  // Event listener handles
  let unlistenStatus: () => void;
  // @ts-ignore
  let unlistenLogs: () => void;

  async function loadDaemonStatus() {
    try {
      daemonStatus = await invoke("get_daemon_status");
    } catch (e) {
      console.error("Failed to load daemon status", e);
    }
  }

  async function toggleDaemon() {
    try {
      const action = daemonStatus.running ? "stop" : "start";
      await invoke("control_daemon", { action });
      setTimeout(loadDaemonStatus, 500);
    } catch (e: any) {
      await showCustomAlert("Error toggling daemon: " + e, "Daemon Control Error");
    }
  }

  async function restartDaemon() {
    try {
      await invoke("control_daemon", { action: "restart" });
      setTimeout(loadDaemonStatus, 500);
    } catch (e: any) {
      await showCustomAlert("Error restarting daemon: " + e, "Daemon Control Error");
    }
  }

  async function loadRepos() {
    try {
      watchedRepos = await invoke("get_watched_repositories");
      if (watchedRepos.length > 0 && !selectedRepo) {
        selectedRepo = watchedRepos[0];
        loadSnapshots();
      }
    } catch (e) {
      console.error("Failed to load watched repos", e);
    }
  }

  async function addRepo(path: string) {
    if (!path.trim()) return;
    try {
      watchlistError = "";
      watchlistMessage = "";
      await invoke("toggle_watch_repo", { path, watch: true });
      watchlistMessage = `Successfully added watch for: ${path}`;
      await loadRepos();
      if (selectorCurrentDir) {
        await navigateSelector(selectorCurrentDir);
      }
    } catch (e: any) {
      watchlistError = e.toString();
    }
  }

  async function initSelector() {
    try {
      selectorLoading = true;
      selectorError = "";
      baseRootDir = await invoke("get_base_root_dir");
      await navigateSelector(baseRootDir);
    } catch (e: any) {
      selectorError = e.toString();
    } finally {
      selectorLoading = false;
    }
  }

  async function navigateSelector(dir: string) {
    try {
      selectorLoading = true;
      selectorError = "";
      selectorCurrentDir = dir;
      selectorEntries = await invoke("get_selector_entries", { currentDir: dir });
      isCurrentDirRepo = await invoke("is_git_repo", { path: dir });
    } catch (e: any) {
      selectorError = e.toString();
    } finally {
      selectorLoading = false;
    }
  }

  async function goUpSelector() {
    const parts = selectorCurrentDir.split(/[\/\\]/);
    if (parts.length > 1) {
      parts.pop();
      let parent = parts.join("/") || "/";
      if (parent.endsWith(":")) {
        parent += "/";
      }
      await navigateSelector(parent);
    }
  }

  async function watchPath(path: string) {
    try {
      watchlistError = "";
      watchlistMessage = "";
      await invoke("toggle_watch_repo", { path, watch: true });
      watchlistMessage = `Successfully added watch for: ${path}`;
      await loadRepos();
      if (selectorCurrentDir) {
        await navigateSelector(selectorCurrentDir);
      }
    } catch (e: any) {
      watchlistError = e.toString();
    }
  }

  async function removeRepo(path: string) {
    try {
      watchlistError = "";
      watchlistMessage = "";
      await invoke("toggle_watch_repo", { path, watch: false });
      watchlistMessage = `Stopped watching: ${path}`;
      if (selectedRepo === path) {
        selectedRepo = "";
        snapshots = [];
        selectedSnapshot = null;
        snapshotFiles = [];
        activeFileDiff = "";
      }
      await loadRepos();
    } catch (e: any) {
      watchlistError = e.toString();
    }
  }

  async function loadSnapshots() {
    if (!selectedRepo) return;
    try {
      snapshots = await invoke("get_snapshots", { repoPath: selectedRepo, showAll: showAllSnapshots });
      selectedSnapshot = null;
      snapshotFiles = [];
      activeFileDiff = "";
      selectedFiles.clear();
    } catch (e) {
      console.error("Failed to load snapshots", e);
    }
  }

  async function toggleSnapshotFilter() {
    showAllSnapshots = !showAllSnapshots;
    await loadSnapshots();
  }

  async function selectSnapshot(snap: any) {
    selectedSnapshot = snap;
    selectedFiles.clear();
    activeFileDiff = "";
    selectedFileForDiff = "";
    try {
      const files: [string, string][] = await invoke("get_snapshot_files", {
        repoPath: selectedRepo,
        hash: snap.commit_hash
      });
      snapshotFiles = files;
    } catch (e) {
      console.error("Failed to get snapshot files", e);
    }
  }

  async function viewFileDiff(file: string) {
    selectedFileForDiff = file;
    try {
      const fullDiff: string = await invoke("get_snapshot_diff", {
        repoPath: selectedRepo,
        hash: selectedSnapshot.commit_hash
      });
      activeFileDiff = fullDiff;
    } catch (e) {
      console.error("Failed to load diff", e);
    }
  }

  function toggleFileSelection(file: string) {
    const updated = new Set(selectedFiles);
    if (updated.has(file)) {
      updated.delete(file);
    } else {
      updated.add(file);
    }
    selectedFiles = updated;
  }

  async function restoreSelected() {
    if (!selectedSnapshot || selectedFiles.size === 0) return;
    try {
      backupError = "";
      backupMessage = "";
      await invoke("restore_files", {
        repoPath: selectedRepo,
        hash: selectedSnapshot.commit_hash,
        files: Array.from(selectedFiles)
      });
      backupMessage = `Successfully restored ${selectedFiles.size} file(s) from snapshot.`;
      selectedFiles.clear();
    } catch (e: any) {
      backupError = e.toString();
    }
  }

  async function restoreFull() {
    if (!selectedSnapshot) return;
    if (!await showCustomConfirm("Are you sure you want to restore the ENTIRE repository to this snapshot? This will overwrite local uncommitted changes.", "Restore Entire Repository")) return;
    try {
      backupError = "";
      backupMessage = "";
      await invoke("restore_files", {
        repoPath: selectedRepo,
        hash: selectedSnapshot.commit_hash,
        files: null
      });
      backupMessage = `Successfully restored entire repository to snapshot state.`;
    } catch (e: any) {
      backupError = e.toString();
    }
  }

  async function pruneSnapshots() {
    if (!selectedRepo) return;
    const input = await showCustomPrompt(
      "Prune Snapshots Options:\n" +
      "- Enter a commit hash to prune snapshots prior to it (e.g., a8c2e9b)\n" +
      "- Enter 'keep:<N>' to keep only the last N commits' snapshots (e.g., keep:5)\n" +
      "- Enter 'before:<DURATION>' to prune snapshots older than a duration (e.g., before:30d, before:12h)\n\n" +
      "Warning: This will permanently delete snapshot branches.",
      "Prune Snapshots"
    );
    if (input === null) return;
    
    const trimmed = input.trim();
    if (!trimmed) {
      await showCustomAlert("No input provided. Pruning cancelled.", "Pruning Cancelled");
      return;
    }

    let targetCommit = null;
    let keepLastN = null;
    let beforeDuration = null;

    if (trimmed.startsWith("keep:")) {
      const n = parseInt(trimmed.substring(5).trim(), 10);
      if (isNaN(n)) {
        await showCustomAlert("Invalid number for keep option.", "Invalid Input");
        return;
      }
      keepLastN = n;
    } else if (trimmed.startsWith("before:")) {
      const dur = trimmed.substring(7).trim();
      if (!dur) {
        await showCustomAlert("Invalid duration.", "Invalid Input");
        return;
      }
      beforeDuration = dur;
    } else {
      targetCommit = trimmed;
    }

    const runGc = await showCustomConfirm("Would you like to run Git Garbage Collection (gc) afterwards to reclaim disk space immediately?", "Run Garbage Collection");

    try {
      backupError = "";
      backupMessage = "";
      const result: string = await invoke("prune_snapshots", {
        repoPath: selectedRepo,
        targetCommit,
        keepLastN,
        beforeDuration,
        runGc
      });
      backupMessage = result;
      await loadSnapshots();
    } catch (e: any) {
      backupError = `Prune failed: ${e.toString()}`;
    }
  }

  async function loadMetrics() {
    try {
      metricsText = await invoke("get_metrics_summary", { humanReadable: true });
      const rawText = (await invoke("get_metrics_summary", { humanReadable: false })) as string;
      if (!rawText || rawText.includes("No log file found") || rawText.includes("No snapshot metrics found")) {
        rawMetrics = [];
        return;
      }
      rawMetrics = rawText
        .split("\n")
        .filter((line: string) => line.trim() !== "")
        .map((line: string) => {
          try {
            return JSON.parse(line);
          } catch {
            return null;
          }
        })
        .filter((x: any) => x !== null);
    } catch (e) {
      console.error("Failed to load metrics", e);
      rawMetrics = [];
    }
  }

  async function loadLogs() {
    try {
      logsText = await invoke("get_log_tail", { lines: 45 });
    } catch (e) {
      console.error("Failed to load logs", e);
    }
  }

  function formatUptime(secs: number | null): string {
    if (secs === null) return "N/A";
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return `${h}h ${m}m ${s}s`;
  }

  onMount(() => {
    if (typeof localStorage !== "undefined") {
      const stored = localStorage.getItem("endur_mgmt_mode");
      if (stored === "direct" || stored === "service") {
        managementMode = stored;
      }
    }

    loadRepos();
    loadLogs();
    initSelector();

    (async () => {
      try {
        await Promise.all([loadDaemonStatus(), loadServiceStatus()]);
        if (serviceStatus.running) {
          setManagementMode("service");
        } else if (daemonStatus.running) {
          setManagementMode("direct");
        }
      } catch (e) {
        console.error("Failed to load initial daemon/service status", e);
      }
    })();

    const serviceInterval = setInterval(() => {
      loadServiceStatus();
    }, 2000);
    
    listen("daemon-status", (event: any) => {
      daemonStatus = event.payload;
    }).then(fn => unlistenStatus = fn);

    listen("daemon-logs", (event: any) => {
      logsText = event.payload;
    }).then(fn => unlistenLogs = fn);

    return () => {
      if (unlistenStatus) unlistenStatus();
      if (unlistenLogs) unlistenLogs();
      clearInterval(serviceInterval);
    };
  });

  $effect(() => {
    if (activeTab === "metrics") {
      loadMetrics();
    }
  });
</script>

<div class="app-layout">
  <!-- Sidebar Navigation -->
  <aside class="sidebar">
    <div class="logo-area">
      <span class="logo-icon">🔏</span>
      <span class="logo-text">Endur</span>
    </div>
    
    <nav class="nav-links">
      <button 
        class="nav-btn" 
        class:active={activeTab === "dashboard"} 
        onclick={() => activeTab = "dashboard"}
      >
        <span class="btn-icon">📟</span> Dashboard
      </button>
      <button 
        class="nav-btn" 
        class:active={activeTab === "repos"} 
        onclick={() => activeTab = "repos"}
      >
        <span class="btn-icon">📁</span> Watchlist
      </button>
      <button 
        class="nav-btn" 
        class:active={activeTab === "backups"} 
        onclick={() => activeTab = "backups"}
      >
        <span class="btn-icon">🕒</span> Backups & Restore
      </button>
      <button 
        class="nav-btn" 
        class:active={activeTab === "metrics"} 
        onclick={() => activeTab = "metrics"}
      >
        <span class="btn-icon">📊</span> Analytics
      </button>
    </nav>

    <div class="status-footer">
      <div class="status-indicator">
        <span class="dot" class:running={daemonStatus.running || serviceStatus.running}></span>
        <span class="status-label">
          {#if serviceStatus.running}
            Service Active
          {:else if daemonStatus.running}
            Process Active
          {:else}
            Background Offline
          {/if}
        </span>
      </div>
      <div class="attribution-note">
        Maintained by <a href="https://github.com/PJC-64/endur" target="_blank" rel="noopener noreferrer">PJC-64</a>. Fork of <a href="https://github.com/tkellogg/dura" target="_blank" rel="noopener noreferrer">tkellogg/dura</a>.
      </div>
    </div>
  </aside>

  <!-- Main Content Area -->
  <main class="main-content">
    
    <!-- Tab 1: Dashboard -->
    {#if activeTab === "dashboard"}
      <div class="tab-content dashboard-view">
        <header class="view-header">
          <h1>Daemon Dashboard</h1>
          <p>Monitor background server activities and IPC status.</p>
        </header>

        <div class="card-grid">
          <!-- Status Card -->
          <div class="card glass status-card">
            <h2>Daemon Control</h2>
            
            <div class="mode-selector">
              <button 
                class="mode-btn" 
                class:active={managementMode === "direct"} 
                onclick={() => setManagementMode("direct")}
              >
                Direct Process
              </button>
              <button 
                class="mode-btn" 
                class:active={managementMode === "service"} 
                onclick={() => setManagementMode("service")}
              >
                System Service
              </button>
            </div>

            {#if managementMode === "direct"}
              <div class="status-metric">
                <span class="metric-label">Status:</span>
                <span class="metric-val status-badge" 
                      class:active={daemonStatus.running && (!daemonStatus.version || daemonStatus.version === daemonStatus.client_version)}
                      class:outdated={daemonStatus.running && daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
                >
                  {#if !daemonStatus.running}
                    STOPPED
                  {:else if daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
                    OUTDATED
                  {:else}
                    RUNNING
                  {/if}
                </span>
              </div>
              {#if serviceStatus.installed && serviceStatus.running}
                <div class="service-running-note">
                  ℹ️ Daemon is running as a system service. Use the "System Service" tab above to manage this instance.
                </div>
              {:else}
                <div class="status-metric">
                  <span class="metric-label">Process ID:</span>
                  <span class="metric-val">{daemonStatus.pid || "N/A"}</span>
                </div>
                <div class="status-metric">
                  <span class="metric-label">Uptime:</span>
                  <span class="metric-val">{formatUptime(daemonStatus.uptime_secs)}</span>
                </div>
                {#if daemonStatus.running}
                  <div class="status-metric">
                    <span class="metric-label">Running Version:</span>
                    <span class="metric-val">{daemonStatus.version || "Unknown"}</span>
                  </div>
                  {#if daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
                    <div class="version-warning-box">
                      ⚠️ Version mismatch! Running v{daemonStatus.version}, expected v{daemonStatus.client_version}.
                    </div>
                  {/if}
                {/if}
                <div class="control-actions">
                  <button 
                    class="action-btn" 
                    class:stop={daemonStatus.running}
                    onclick={toggleDaemon}
                  >
                    {daemonStatus.running ? "Terminate Daemon" : "Launch Daemon"}
                  </button>
                  {#if daemonStatus.running}
                    <button 
                      class="action-btn secondary" 
                      onclick={restartDaemon}
                    >
                      Restart Daemon
                    </button>
                  {/if}
                </div>
              {/if}
            {:else}
              <div class="status-metric">
                <span class="metric-label">Service Config:</span>
                <span class="metric-val status-badge"
                      class:installed={serviceStatus.installed}
                      class:not-installed={!serviceStatus.installed}
                >
                  {serviceStatus.installed ? "INSTALLED" : "NOT INSTALLED"}
                </span>
              </div>
              <div class="status-metric">
                <span class="metric-label">Service Status:</span>
                <span class="metric-val status-badge"
                      class:active={serviceStatus.running}
                >
                  {serviceStatus.running ? "RUNNING" : "STOPPED"}
                </span>
              </div>
              {#if serviceStatus.installed && serviceStatus.running}
                <div class="status-metric">
                  <span class="metric-label">Process ID:</span>
                  <span class="metric-val">{daemonStatus.pid || "N/A"}</span>
                </div>
                <div class="status-metric">
                  <span class="metric-label">Uptime:</span>
                  <span class="metric-val">{formatUptime(daemonStatus.uptime_secs)}</span>
                </div>
                <div class="status-metric">
                  <span class="metric-label">Running Version:</span>
                  <span class="metric-val">{daemonStatus.version || "Unknown"}</span>
                </div>
                {#if daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
                  <div class="version-warning-box">
                    ⚠️ Version mismatch! Running v{daemonStatus.version}, expected v{daemonStatus.client_version}.
                  </div>
                {/if}
              {/if}
              <div class="control-actions">
                {#if !serviceStatus.installed}
                  <button 
                    class="action-btn" 
                    disabled={serviceActionLoading}
                    onclick={() => handleServiceAction("install")}
                  >
                    {serviceActionLoading ? "Installing..." : "Install Service"}
                  </button>
                {:else}
                  <button 
                    class="action-btn" 
                    class:stop={serviceStatus.running}
                    disabled={serviceActionLoading}
                    onclick={() => handleServiceAction(serviceStatus.running ? "stop" : "start")}
                  >
                    {serviceActionLoading ? "Processing..." : (serviceStatus.running ? "Stop Service" : "Start Service")}
                  </button>
                  <button 
                    class="action-btn secondary" 
                    disabled={serviceActionLoading}
                    onclick={() => handleServiceAction("install")}
                  >
                    {serviceActionLoading ? "Reinstalling..." : "Reinstall Service"}
                  </button>
                  <button 
                    class="action-btn secondary" 
                    disabled={serviceActionLoading}
                    onclick={() => handleServiceAction("uninstall")}
                  >
                    Uninstall Service
                  </button>
                {/if}
              </div>
              <div class="service-note-box">
                ℹ️ Note: Installing or reinstalling the service will automatically stop and remove any currently-installed service version before registering and starting the latest one.
              </div>
            {/if}
          </div>

          <!-- Watchlist Stats Card -->
          <div class="card glass stats-card">
            <h2>Watchlist Summary</h2>
            <div class="big-number">{watchedRepos.length}</div>
            <p class="stats-label">Active Git Repositories Monitored</p>
            <button class="action-btn secondary" onclick={() => activeTab = "repos"}>Manage Folders</button>
          </div>
        </div>

        <!-- Real-Time Logs Console -->
        <LogViewer logsText={logsText} onRefresh={loadLogs} />
      </div>
    {/if}

    <!-- Tab 2: Repositories (Watchlist) -->
    {#if activeTab === "repos"}
      <RepoList
        watchedRepos={watchedRepos}
        selectorCurrentDir={selectorCurrentDir}
        selectorEntries={selectorEntries}
        selectorLoading={selectorLoading}
        selectorError={selectorError}
        baseRootDir={baseRootDir}
        isCurrentDirRepo={isCurrentDirRepo}
        watchlistMessage={watchlistMessage}
        watchlistError={watchlistError}
        onRemoveRepo={removeRepo}
        onAddRepo={addRepo}
        onWatchPath={watchPath}
        onNavigateSelector={navigateSelector}
        onGoUpSelector={goUpSelector}
      />
    {/if}

    <!-- Tab 3: Backups & Restore -->
    {#if activeTab === "backups"}
      <SnapshotView
        selectedRepo={selectedRepo}
        watchedRepos={watchedRepos}
        snapshots={snapshots}
        selectedSnapshot={selectedSnapshot}
        snapshotFiles={snapshotFiles}
        selectedFiles={selectedFiles}
        activeFileDiff={activeFileDiff}
        selectedFileForDiff={selectedFileForDiff}
        backupMessage={backupMessage}
        backupError={backupError}
        showAllSnapshots={showAllSnapshots}
        onSelectRepo={(repo) => { selectedRepo = repo; loadSnapshots(); }}
        onToggleSnapshotFilter={toggleSnapshotFilter}
        onPruneSnapshots={pruneSnapshots}
        onSelectSnapshot={selectSnapshot}
        onRestoreSelected={restoreSelected}
        onRestoreFull={restoreFull}
        onToggleFileSelection={toggleFileSelection}
        onViewFileDiff={viewFileDiff}
      />
    {/if}

    <!-- Tab 4: Performance Metrics -->
    {#if activeTab === "metrics"}
      <MetricsView
        rawMetrics={rawMetrics}
        metricsText={metricsText}
        onRefresh={loadMetrics}
      />
    {/if}

    <!-- Custom Modal Dialog -->
    <CustomDialog bind:customDialog />

  </main>
</div>

<style>
  :root {
    font-family: 'Outfit', sans-serif;
    color: #cbd5e1;
    background-color: #0b0f19;
  }

  .app-layout {
    display: grid;
    grid-template-columns: 260px 1fr;
    height: 100vh;
    overflow: hidden;
  }

  /* Sidebar styling */
  .sidebar {
    background-color: #0f172a;
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    flex-direction: column;
    padding: 1.5rem 1rem;
    box-sizing: border-box;
  }

  .logo-area {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 2rem;
    padding: 0 0.5rem;
  }

  .logo-icon {
    font-size: 1.75rem;
  }

  .logo-text {
    font-size: 1.5rem;
    font-weight: 700;
    color: #f8fafc;
    letter-spacing: -0.025em;
  }

  .nav-links {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex-grow: 1;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: none;
    border: none;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    color: #94a3b8;
    text-align: left;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .nav-btn:hover {
    background-color: rgba(255, 255, 255, 0.03);
    color: #f8fafc;
  }

  .nav-btn.active {
    background-color: rgba(6, 182, 212, 0.1);
    color: #06b6d4;
    border-left: 3px solid #06b6d4;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }

  .status-footer {
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .attribution-note {
    margin-top: 0.75rem;
    font-size: 0.7rem;
    line-height: 1.35;
    color: rgba(148, 163, 184, 0.45);
  }

  .attribution-note a {
    color: rgba(6, 182, 212, 0.6);
    text-decoration: none;
    transition: color 0.15s ease;
  }

  .attribution-note a:hover {
    color: #06b6d4;
    text-decoration: underline;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: #94a3b8;
  }

  .status-indicator .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: #ef4444;
    box-shadow: 0 0 8px #ef4444;
  }

  .status-indicator .dot.running {
    background-color: #10b981;
    box-shadow: 0 0 8px #10b981;
  }

  /* Main Content Area */
  .main-content {
    background: radial-gradient(circle at top right, rgba(6, 182, 212, 0.03), transparent 40%),
                radial-gradient(circle at bottom left, rgba(99, 102, 241, 0.03), transparent 40%),
                #0b0f19;
    padding: 2rem;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .tab-content {
    display: flex;
    flex-direction: column;
    height: 100%;
    animation: fadeIn 0.15s ease-in;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .view-header {
    margin-bottom: 1.5rem;
  }

  .view-header h1 {
    font-size: 1.75rem;
    font-weight: 700;
    color: #f8fafc;
    margin: 0 0 0.25rem 0;
  }

  .view-header p {
    color: #94a3b8;
    margin: 0;
    font-size: 0.95rem;
  }

  /* Cards styling (Glassmorphism) */
  .card.glass {
    background: rgba(15, 23, 42, 0.6);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.5rem;
    box-sizing: border-box;
  }

  .card h2 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-top: 0;
    margin-bottom: 1rem;
    color: #f8fafc;
  }

  .card-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .status-metric {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .status-metric:last-of-type {
    border-bottom: none;
  }

  .metric-label {
    color: #94a3b8;
  }

  .metric-val {
    font-weight: 600;
  }

  .status-badge {
    background-color: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
  }

  .status-badge.active {
    background-color: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .status-badge.outdated {
    background-color: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
  }

  .status-badge.installed {
    background-color: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .status-badge.not-installed {
    background-color: rgba(148, 163, 184, 0.15);
    color: #94a3b8;
  }

  .service-running-note {
    background: rgba(6, 182, 212, 0.08);
    border: 1px solid rgba(6, 182, 212, 0.2);
    border-radius: 6px;
    padding: 0.6rem 0.8rem;
    font-size: 0.82rem;
    color: #67e8f9;
    margin-top: 0.75rem;
    line-height: 1.4;
  }

  .version-warning-box {
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.3);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 0.8rem;
    color: #fbbf24;
    margin-top: 0.5rem;
  }

  .service-note-box {
    margin-top: 1rem;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    font-size: 0.75rem;
    color: #94a3b8;
    line-height: 1.4;
  }

  .control-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 1.5rem;
  }

  .action-btn {
    background-color: #06b6d4;
    color: #0f172a;
    border: none;
    padding: 0.6rem 1.25rem;
    border-radius: 6px;
    font-weight: 600;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
  }

  .action-btn:hover {
    background-color: #22d3ee;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(6, 182, 212, 0.25);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }

  .action-btn.stop {
    background-color: #ef4444;
    color: #f8fafc;
  }

  .action-btn.stop:hover {
    background-color: #dc2626;
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.25);
  }

  .action-btn.secondary {
    background-color: rgba(255, 255, 255, 0.05);
    color: #cbd5e1;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .action-btn.secondary:hover {
    background-color: rgba(255, 255, 255, 0.1);
    color: #f8fafc;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .stats-card {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    text-align: center;
  }

  .big-number {
    font-size: 3.5rem;
    font-weight: 700;
    color: #06b6d4;
    line-height: 1;
    margin: 1rem 0 0.5rem 0;
  }

  .stats-label {
    color: #94a3b8;
    margin: 0 0 1.5rem 0;
    font-size: 0.875rem;
  }

  /* Mode Selector */
  .mode-selector {
    display: flex;
    background-color: rgba(11, 15, 25, 0.6);
    border-radius: 8px;
    padding: 0.25rem;
    margin-bottom: 1.25rem;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .mode-btn {
    flex: 1;
    padding: 0.4rem 0.75rem;
    border: none;
    background: transparent;
    color: #94a3b8;
    font-size: 0.85rem;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .mode-btn:hover {
    color: #f8fafc;
  }

  .mode-btn.active {
    background-color: rgba(6, 182, 212, 0.15);
    color: #06b6d4;
    font-weight: 600;
  }
</style>
