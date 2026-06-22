<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  // Tab State
  let activeTab: "dashboard" | "repos" | "backups" | "metrics" = $state("dashboard");

  // Daemon Status State
  let daemonStatus = $state({ running: false, pid: null, uptime_secs: null, version: null, client_version: "" });

  // Watchlist State
  let watchedRepos: string[] = $state([]);
  let newRepoPath = $state("");
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
  let snapshotFiles: [string, string][] = $state([]); // [status, path]
  let selectedFiles: Set<string> = $state(new Set());
  let activeFileDiff = $state("");
  let selectedFileForDiff = $state("");
  let backupMessage = $state("");
  let backupError = $state("");
  let showAllSnapshots = $state(false);

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
      alert(`Error performing action '${action}' on service: ` + e);
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
  let activeBarHover: any = $state(null);
  let activeLatencyHover: any = $state(null);

  // Derived metrics for charts
  let chartMetrics = $derived(rawMetrics.slice(-30));
  let maxLatency = $derived(Math.max(...chartMetrics.map(m => m.latency), 0.001));
  let maxChanges = $derived(Math.max(...chartMetrics.map(m => m.insertions + m.deletions), 10));

  let avgLatency = $derived(rawMetrics.length > 0 ? rawMetrics.reduce((sum, m) => sum + m.latency, 0) / rawMetrics.length : 0);
  let maxLatencyVal = $derived(rawMetrics.length > 0 ? Math.max(...rawMetrics.map(m => m.latency)) : 0);
  let totalLinesChanged = $derived(rawMetrics.reduce((sum, m) => sum + m.insertions + m.deletions, 0));

  let latencyPoints = $derived.by(() => {
    const width = 500;
    const height = 150;
    const padding = 20;
    const xStep = chartMetrics.length > 1 ? (width - padding * 2) / (chartMetrics.length - 1) : width - padding * 2;
    
    return chartMetrics.map((m, i) => ({
      x: padding + i * xStep,
      y: height - padding - (m.latency / maxLatency) * (height - padding * 2),
      val: m.latency < 1.0 ? `${(m.latency * 1000).toFixed(1)}ms` : `${m.latency.toFixed(2)}s`,
      repo: m.repo.split('/').pop() || m.repo,
      fullRepo: m.repo,
      time: new Date(m.time).toLocaleTimeString(),
      date: new Date(m.time).toLocaleDateString()
    }));
  });

  let latencyPath = $derived.by(() => {
    const pts = latencyPoints;
    if (pts.length === 0) return "";
    return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(" ");
  });

  let latencyAreaPath = $derived.by(() => {
    const pts = latencyPoints;
    if (pts.length === 0) return "";
    const width = 500;
    const height = 150;
    const padding = 20;
    const firstX = padding;
    const lastX = pts[pts.length - 1].x;
    return `${latencyPath} L ${lastX} ${height - padding} L ${firstX} ${height - padding} Z`;
  });

  let activityBars = $derived.by(() => {
    const width = 500;
    const height = 150;
    const padding = 20;
    const xStep = chartMetrics.length > 0 ? (width - padding * 2) / chartMetrics.length : width - padding * 2;
    const barWidth = Math.max(xStep - 4, 4);

    return chartMetrics.map((m, i) => {
      const totalHeight = height - padding * 2;
      const insHeight = (m.insertions / maxChanges) * totalHeight;
      const delHeight = (m.deletions / maxChanges) * totalHeight;
      
      return {
        x: padding + i * xStep,
        insY: height - padding - insHeight,
        insHeight,
        delY: height - padding - insHeight - delHeight,
        delHeight,
        barWidth,
        insertions: m.insertions,
        deletions: m.deletions,
        repo: m.repo.split('/').pop() || m.repo,
        fullRepo: m.repo,
        time: new Date(m.time).toLocaleTimeString(),
        date: new Date(m.time).toLocaleDateString()
      };
    });
  });

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
      alert("Error toggling daemon: " + e);
    }
  }

  async function restartDaemon() {
    try {
      await invoke("control_daemon", { action: "restart" });
      setTimeout(loadDaemonStatus, 500);
    } catch (e: any) {
      alert("Error restarting daemon: " + e);
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

  async function addRepo() {
    if (!newRepoPath.trim()) return;
    try {
      watchlistError = "";
      watchlistMessage = "";
      await invoke("toggle_watch_repo", { path: newRepoPath, watch: true });
      watchlistMessage = `Successfully added watch for: ${newRepoPath}`;
      newRepoPath = "";
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
    if (!confirm("Are you sure you want to restore the ENTIRE repository to this snapshot? This will overwrite local uncommitted changes.")) return;
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
    const input = prompt(
      "Prune Snapshots Options:\n" +
      "- Enter a commit hash to prune snapshots prior to it (e.g., a8c2e9b)\n" +
      "- Enter 'keep:<N>' to keep only the last N commits' snapshots (e.g., keep:5)\n" +
      "- Enter 'before:<DURATION>' to prune snapshots older than a duration (e.g., before:30d, before:12h)\n\n" +
      "Warning: This will permanently delete snapshot branches."
    );
    if (input === null) return; // Cancelled
    
    const trimmed = input.trim();
    if (!trimmed) {
      alert("No input provided. Pruning cancelled.");
      return;
    }

    let targetCommit = null;
    let keepLastN = null;
    let beforeDuration = null;

    if (trimmed.startsWith("keep:")) {
      const n = parseInt(trimmed.substring(5).trim(), 10);
      if (isNaN(n)) {
        alert("Invalid number for keep option.");
        return;
      }
      keepLastN = n;
    } else if (trimmed.startsWith("before:")) {
      const dur = trimmed.substring(7).trim();
      if (!dur) {
        alert("Invalid duration.");
        return;
      }
      beforeDuration = dur;
    } else {
      targetCommit = trimmed;
    }

    const runGc = confirm("Would you like to run Git Garbage Collection (gc) afterwards to reclaim disk space immediately?");

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
    // Load management mode preference
    if (typeof localStorage !== "undefined") {
      const stored = localStorage.getItem("endur_mgmt_mode");
      if (stored === "direct" || stored === "service") {
        managementMode = stored;
      }
    }

    loadRepos();
    loadLogs();
    initSelector();

    // Dynamically auto-switch management mode on startup if service or daemon is running
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
      <div class="status-indicator" 
           class:running={daemonStatus.running && (!daemonStatus.version || daemonStatus.version === daemonStatus.client_version)}
           class:outdated={daemonStatus.running && daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
      >
        <span class="dot"></span>
        <span class="status-text">
          {#if !daemonStatus.running}
            Daemon Inactive
          {:else if daemonStatus.version && daemonStatus.version !== daemonStatus.client_version}
            Daemon Outdated
          {:else}
            Daemon Active
          {/if}
        </span>
      </div>
      <div class="attribution-note">
        This fork brought to you by <a href="https://github.com/PJC-64" target="_blank" rel="noopener noreferrer">PJC-64</a>, original 'dura' by <a href="https://github.com/tkellogg" target="_blank" rel="noopener noreferrer">Tim Kellogg</a>
      </div>
    </div>
  </aside>

  <!-- Main Content Panel -->
  <main class="content-panel">
    
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
        <div class="console-box card glass">
          <div class="console-header">
            <h3>Live Daemon Logs</h3>
            <button class="console-refresh" onclick={loadLogs}>Refresh</button>
          </div>
          <pre class="console-body">{logsText || "No logs available. Ensure daemon is serving."}</pre>
        </div>
      </div>
    {/if}

    <!-- Tab 2: Repositories (Watchlist) -->
    {#if activeTab === "repos"}
      <div class="tab-content repos-view">
        <header class="view-header">
          <h1>Monitored Watchlist</h1>
          <p>Register or unwatch directory paths from backups.</p>
        </header>

        <div class="repos-layout">
          <!-- Left Column: Active Monitored Repositories -->
          <div class="card glass repos-list-card">
            <h2>Active Repositories</h2>
            {#if watchedRepos.length === 0}
              <p class="empty-text">No active repositories registered. Use the browser on the right to add folders.</p>
            {:else}
              <div class="repos-list">
                {#each watchedRepos as path}
                  <div class="repo-item">
                    <div class="repo-info">
                      <span class="folder-icon">📁</span>
                      <span class="repo-path" title={path}>{path}</span>
                    </div>
                    <button class="unwatch-btn" onclick={() => removeRepo(path)}>Unwatch</button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Right Column: Interactive File Selector & Manual Form -->
          <div class="card glass add-repo-card">
            <h2>Monitor New Repository</h2>
            
            <!-- File Selector Component -->
            <div class="selector-component">
              <div class="selector-path-bar">
                <span class="path-label" title={selectorCurrentDir}>Current: {selectorCurrentDir || "Loading..."}</span>
                <div class="path-bar-actions">
                  <button class="btn-sm" onclick={goUpSelector} disabled={selectorLoading || !selectorCurrentDir || selectorCurrentDir === "/"}>↱ Up</button>
                  <button class="btn-sm" onclick={() => navigateSelector(baseRootDir)} disabled={selectorLoading || !baseRootDir} title="Go to default base root directory">🏠 Base</button>
                </div>
              </div>

              {#if selectorLoading}
                <div class="selector-status">Loading directory entries...</div>
              {:else if selectorError}
                <div class="selector-status error">{selectorError}</div>
              {:else}
                <div class="selector-list">
                  {#if isCurrentDirRepo}
                    <div class="selector-item current-dir-repo">
                      <span class="folder-icon-repo">●</span>
                      <span class="repo-name-label">Current Folder is Git Repo</span>
                      <button 
                        class="watch-action-btn" 
                        onclick={() => watchPath(selectorCurrentDir)}
                        disabled={watchedRepos.includes(selectorCurrentDir)}
                      >
                        {watchedRepos.includes(selectorCurrentDir) ? "Watched" : "Watch Current"}
                      </button>
                    </div>
                  {/if}

                  {#if selectorEntries.length === 0}
                    <div class="empty-selector">No subdirectories containing Git repositories found.</div>
                  {:else}
                    {#each selectorEntries as entry}
                      <div class="selector-row">
                        <div class="entry-info" onclick={() => navigateSelector(entry.path)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === 'Enter') navigateSelector(entry.path); }}>
                          <span class="folder-icon">📁</span>
                          <span class="entry-name">{entry.name}</span>
                          {#if entry.is_repo}
                            <span class="repo-badge">git</span>
                          {/if}
                        </div>
                        {#if entry.is_repo}
                          <button 
                            class="watch-action-btn"
                            onclick={() => watchPath(entry.path)}
                            disabled={watchedRepos.includes(entry.path)}
                          >
                            {watchedRepos.includes(entry.path) ? "Watched" : "Watch"}
                          </button>
                        {/if}
                      </div>
                    {/each}
                  {/if}
                </div>
              {/if}
            </div>

            <!-- Fallback Manual Input -->
            <div class="manual-input-box">
              <h3>Or enter path manually:</h3>
              <form class="repo-form" onsubmit={(e) => { e.preventDefault(); addRepo(); }}>
                <input 
                  type="text" 
                  placeholder="Enter absolute repository path..." 
                  bind:value={newRepoPath} 
                  class="path-input"
                />
                <button type="submit" class="action-btn">Watch Folder</button>
              </form>
            </div>

            {#if watchlistMessage}
              <p class="msg-success">{watchlistMessage}</p>
            {/if}
            {#if watchlistError}
              <p class="msg-error">{watchlistError}</p>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    <!-- Tab 3: Backups & Restore -->
    {#if activeTab === "backups"}
      <div class="tab-content backups-view">
        <header class="view-header">
          <h1>Browse Backups & Recovery</h1>
          <p>Navigate Git snapshots, preview file changes, and recover specific files.</p>
        </header>

        <div class="backups-layout">
          <!-- Repo Selector & Snapshots Column (Left) -->
          <div class="pane-column snapshots-pane card glass">
            <div class="pane-header">
              <h3>Snapshots {showAllSnapshots ? '(All)' : '(Since HEAD)'}</h3>
              <div class="pane-header-controls">
                <button
                  class="filter-toggle-btn"
                  class:active={showAllSnapshots}
                  onclick={toggleSnapshotFilter}
                  title={showAllSnapshots ? 'Showing all snapshots — click to show only current HEAD' : 'Showing snapshots since last commit — click to show all'}
                >
                  {showAllSnapshots ? '🕰 All' : '📌 HEAD'}
                </button>
                <button
                  class="filter-toggle-btn prune-btn"
                  onclick={pruneSnapshots}
                  disabled={!selectedRepo}
                  title="Prune historical snapshots to reclaim disk space"
                >
                  ✂️ Prune
                </button>
                <select bind:value={selectedRepo} onchange={loadSnapshots} class="repo-select">
                  {#each watchedRepos as path}
                    <option value={path}>{path.split('/').pop() || path}</option>
                  {/each}
                </select>
              </div>
            </div>
            
            {#if snapshots.length === 0}
              <p class="empty-pane-text">No snapshot branches found for this repository.</p>
            {:else}
              <div class="snapshots-list">
                {#each snapshots as snap}
                  <button 
                    class="snapshot-item" 
                    class:active={selectedSnapshot && selectedSnapshot.commit_hash === snap.commit_hash}
                    onclick={() => selectSnapshot(snap)}
                  >
                    <div class="snap-hash">{snap.commit_hash.substring(0, 7)}</div>
                    <div class="snap-desc">{snap.message || "Auto-backup snapshot"}</div>
                    <div class="snap-meta">
                      <span>🕒 {new Date(snap.timestamp * 1000).toLocaleTimeString()}</span>
                      <span>📂 {snap.files_changed} file(s)</span>
                    </div>
                  </button>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Files Tree Column (Middle) -->
          <div class="pane-column files-pane card glass">
            <div class="pane-header">
              <h3>Changed Files</h3>
              {#if selectedSnapshot}
                <div class="pane-actions">
                  <button class="pane-action-btn" onclick={restoreSelected} disabled={selectedFiles.size === 0}>
                    Restore Selected ({selectedFiles.size})
                  </button>
                  <button class="pane-action-btn danger" onclick={restoreFull}>
                    Restore All
                  </button>
                </div>
              {/if}
            </div>

            {#if !selectedSnapshot}
              <p class="empty-pane-text">Select a snapshot commit to inspect changed files.</p>
            {:else if snapshotFiles.length === 0}
              <p class="empty-pane-text">No files recorded in this snapshot.</p>
            {:else}
              <div class="files-list">
                {#each snapshotFiles as [status, filePath]}
                  <div class="file-item" class:active={selectedFileForDiff === filePath}>
                    <input 
                      type="checkbox" 
                      checked={selectedFiles.has(filePath)} 
                      onchange={() => toggleFileSelection(filePath)}
                      class="file-checkbox"
                    />
                    <button class="file-path-btn" onclick={() => viewFileDiff(filePath)}>
                      <span class="file-status" class:added={status === 'A'} class:modified={status === 'M'} class:deleted={status === 'D'}>
                        [{status}]
                      </span>
                      <span class="file-name">{filePath}</span>
                    </button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Diff Previewer (Right) -->
          <div class="pane-column diff-pane card glass">
            <div class="pane-header">
              <h3>Commit Diff Preview</h3>
            </div>
            {#if !activeFileDiff}
              <p class="empty-pane-text">Select a file from the changed files list to view its code changes.</p>
            {:else}
              <pre class="diff-viewer"><code>{activeFileDiff}</code></pre>
            {/if}
          </div>
        </div>

        {#if backupMessage}
          <div class="alert success">{backupMessage}</div>
        {/if}
        {#if backupError}
          <div class="alert error">{backupError}</div>
        {/if}
      </div>
    {/if}

    <!-- Tab 4: Performance Metrics -->
    {#if activeTab === "metrics"}
      <div class="tab-content metrics-view">
        <header class="view-header">
          <h1>Performance Analytics</h1>
          <p>Inspect log statistics, save latency, and file changes aggregated over time.</p>
        </header>

        {#if rawMetrics.length > 0}
          <!-- Summary Cards -->
          <div class="metrics-summary-grid">
            <div class="summary-card glass">
              <span class="label">Total Snapshots</span>
              <span class="val">{rawMetrics.length}</span>
            </div>
            <div class="summary-card glass">
              <span class="label">Total Lines Changed</span>
              <span class="val">
                {totalLinesChanged.toLocaleString()}
              </span>
            </div>
            <div class="summary-card glass">
              <span class="label">Avg Latency</span>
              <span class="val">
                {avgLatency < 1.0 ? `${(avgLatency * 1000).toFixed(1)}ms` : `${avgLatency.toFixed(2)}s`}
              </span>
            </div>
            <div class="summary-card glass">
              <span class="label">Max Latency</span>
              <span class="val">
                {maxLatencyVal < 1.0 ? `${(maxLatencyVal * 1000).toFixed(1)}ms` : `${maxLatencyVal.toFixed(2)}s`}
              </span>
            </div>
          </div>

          <!-- Charts Section -->
          <div class="metrics-charts-grid">
            
            <!-- Chart 1: Activity Stacked Bar Chart -->
            <div class="card glass chart-card">
              <div class="chart-header">
                <h3>Lines Changed (Last {chartMetrics.length} Backups)</h3>
                <div class="hover-details">
                  {#if activeBarHover}
                    <span class="hover-title" title={activeBarHover.fullRepo}>{activeBarHover.repo}</span>
                    <span class="hover-info">
                      {activeBarHover.time} &rarr; 
                      <span class="text-ins">+{activeBarHover.insertions}</span> / 
                      <span class="text-del">-{activeBarHover.deletions}</span> lines
                    </span>
                  {:else}
                    <span class="text-muted">Hover over a bar for details</span>
                  {/if}
                </div>
              </div>

              <div class="chart-svg-wrapper">
                <svg width="100%" height="150" viewBox="0 0 500 150" preserveAspectRatio="none">
                  <!-- Gradients -->
                  <defs>
                    <linearGradient id="insGrad" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stop-color="#10b981" stop-opacity="0.85"/>
                      <stop offset="100%" stop-color="#10b981" stop-opacity="0.25"/>
                    </linearGradient>
                    <linearGradient id="delGrad" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stop-color="#ef4444" stop-opacity="0.85"/>
                      <stop offset="100%" stop-color="#ef4444" stop-opacity="0.25"/>
                    </linearGradient>
                  </defs>

                  <!-- Grid line -->
                  <line x1="20" y1="130" x2="480" y2="130" stroke="#334155" stroke-width="1" />

                  {#each activityBars as bar}
                    <!-- Insertions Stack (bottom) -->
                    {#if bar.insHeight > 0}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <rect
                        x={bar.x}
                        y={bar.insY}
                        width={bar.barWidth}
                        height={bar.insHeight}
                        fill="url(#insGrad)"
                        rx="2"
                        class="chart-rect"
                        onmouseenter={() => activeBarHover = bar}
                        onmouseleave={() => activeBarHover = null}
                      />
                    {/if}
                    <!-- Deletions Stack (top of insertions) -->
                    {#if bar.delHeight > 0}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <rect
                        x={bar.x}
                        y={bar.delY}
                        width={bar.barWidth}
                        height={bar.delHeight}
                        fill="url(#delGrad)"
                        rx="2"
                        class="chart-rect"
                        onmouseenter={() => activeBarHover = bar}
                        onmouseleave={() => activeBarHover = null}
                      />
                    {/if}
                    <!-- Invisible full-height bar for easier hovering -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <rect
                      x={bar.x}
                      y="20"
                      width={bar.barWidth}
                      height="110"
                      fill="transparent"
                      onmouseenter={() => activeBarHover = bar}
                      onmouseleave={() => activeBarHover = null}
                      style="cursor: pointer;"
                    />
                  {/each}
                </svg>
              </div>
            </div>

            <!-- Chart 2: Latency Line / Area Chart -->
            <div class="card glass chart-card">
              <div class="chart-header">
                <h3>Backup Latency Trend (Last {chartMetrics.length} Backups)</h3>
                <div class="hover-details">
                  {#if activeLatencyHover}
                    <span class="hover-title" title={activeLatencyHover.fullRepo}>{activeLatencyHover.repo}</span>
                    <span class="hover-info">
                      {activeLatencyHover.time} &rarr; 
                      <span class="text-lat">Latency: {activeLatencyHover.val}</span>
                    </span>
                  {:else}
                    <span class="text-muted">Hover over a point for details</span>
                  {/if}
                </div>
              </div>

              <div class="chart-svg-wrapper">
                <svg width="100%" height="150" viewBox="0 0 500 150" preserveAspectRatio="none">
                  <defs>
                    <linearGradient id="latAreaGrad" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stop-color="#3b82f6" stop-opacity="0.3"/>
                      <stop offset="100%" stop-color="#3b82f6" stop-opacity="0.0"/>
                    </linearGradient>
                  </defs>

                  <!-- Grid line -->
                  <line x1="20" y1="130" x2="480" y2="130" stroke="#334155" stroke-width="1" />

                  {#if latencyPoints.length > 0}
                    <!-- Area under the line -->
                    <path d={latencyAreaPath} fill="url(#latAreaGrad)" />

                    <!-- Line path -->
                    <path d={latencyPath} fill="none" stroke="#3b82f6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />

                    <!-- Neon node circles -->
                    {#each latencyPoints as p}
                      <!-- Large hover target circle -->
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <circle
                        cx={p.x}
                        cy={p.y}
                        r="8"
                        fill="transparent"
                        style="cursor: pointer;"
                        onmouseenter={() => activeLatencyHover = p}
                        onmouseleave={() => activeLatencyHover = null}
                      />
                      <!-- Visible visual circle -->
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <circle
                        cx={p.x}
                        cy={p.y}
                        r="4"
                        fill="#3b82f6"
                        stroke="#0b0f19"
                        stroke-width="1.5"
                        onmouseenter={() => activeLatencyHover = p}
                        onmouseleave={() => activeLatencyHover = null}
                      />
                    {/each}
                  {/if}
                </svg>
              </div>
            </div>

          </div>
        {/if}

        <div class="card glass metrics-table-card">
          <div class="metrics-header">
            <h2>Snapshot Performance Table</h2>
            <button class="console-refresh" onclick={loadMetrics}>Refresh Stats</button>
          </div>
          <pre class="metrics-output">{metricsText || "Generating metrics. Ensure backup logs are active."}</pre>
        </div>
      </div>
    {/if}

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

  .status-indicator.running .dot {
    background-color: #10b981;
    box-shadow: 0 0 8px #10b981;
  }

  .status-indicator.running .status-text {
    color: #e2e8f0;
  }

  .status-indicator.outdated .dot {
    background-color: #f59e0b;
    box-shadow: 0 0 8px #f59e0b;
  }

  .status-indicator.outdated .status-text {
    color: #f59e0b;
  }

  /* Main Content Area */
  .content-panel {
    background-color: #0b0f19;
    padding: 2rem;
    box-sizing: border-box;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    height: 100%;
  }

  .view-header h1 {
    font-size: 1.75rem;
    font-weight: 700;
    color: #f8fafc;
    margin: 0 0 0.25rem 0;
    letter-spacing: -0.02em;
  }

  .view-header p {
    color: #64748b;
    margin: 0;
    font-size: 0.95rem;
  }

  /* Cards styling (Glassmorphism) */
  .card {
    border-radius: 12px;
    padding: 1.5rem;
    box-sizing: border-box;
  }

  .card.glass {
    background: rgba(15, 23, 42, 0.45);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.03);
    box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.2);
  }

  .card-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
  }

  .card h2 {
    font-size: 1.2rem;
    font-weight: 600;
    color: #f8fafc;
    margin: 0 0 1rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 0.5rem;
  }

  .status-metric {
    display: flex;
    justify-content: space-between;
    padding: 0.5rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.02);
    font-size: 0.95rem;
  }

  .control-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .version-warning-box {
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.75rem;
    color: #f59e0b;
    background-color: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.2);
    border-radius: 4px;
    line-height: 1.4;
    text-align: left;
  }

  .metric-label {
    color: #64748b;
  }

  .metric-val {
    color: #e2e8f0;
    font-weight: 500;
  }

  .status-badge {
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-weight: 700;
    font-size: 0.75rem;
    background-color: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  .status-badge.active {
    background-color: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .status-badge.outdated {
    background-color: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
  }

  .action-btn {
    margin-top: 1rem;
    background-color: #06b6d4;
    color: #0f172a;
    border: none;
    border-radius: 6px;
    padding: 0.6rem 1.2rem;
    font-weight: 600;
    font-size: 0.9rem;
    cursor: pointer;
    width: 100%;
    transition: all 0.2s ease;
  }

  .action-btn:hover {
    background-color: #22d3ee;
    box-shadow: 0 0 12px rgba(6, 182, 212, 0.4);
  }

  .action-btn.stop {
    background-color: #ef4444;
    color: #f8fafc;
  }

  .action-btn.stop:hover {
    background-color: #f87171;
    box-shadow: 0 0 12px rgba(239, 68, 68, 0.4);
  }

  .action-btn.secondary {
    background: none;
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #cbd5e1;
  }

  .action-btn.secondary:hover {
    background-color: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.3);
    box-shadow: none;
  }

  .big-number {
    font-size: 3rem;
    font-weight: 800;
    color: #06b6d4;
    margin: 0.5rem 0;
  }

  .stats-label {
    color: #94a3b8;
    margin: 0 0 1rem 0;
    font-size: 0.95rem;
  }

  /* Console Logger */
  .console-box {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
    min-height: 250px;
  }

  .console-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .console-header h3 {
    margin: 0;
    font-size: 1.05rem;
    color: #f8fafc;
  }

  .console-refresh {
    background: none;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #94a3b8;
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .console-refresh:hover {
    color: #f8fafc;
    border-color: rgba(255, 255, 255, 0.25);
  }

  .console-body {
    background-color: #05070f;
    border-radius: 6px;
    padding: 1rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.8rem;
    color: #38bdf8;
    overflow: auto;
    flex-grow: 1;
    margin: 0;
    white-space: pre-wrap;
    line-height: 1.4;
  }

  /* Repos layout */
  .repos-layout {
    display: grid;
    grid-template-columns: 1fr 1.2fr;
    gap: 1.5rem;
    height: calc(100vh - 180px);
    overflow: hidden;
  }

  .repos-list-card, .add-repo-card {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .repos-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    overflow-y: auto;
    flex-grow: 1;
    margin-top: 1rem;
    padding-right: 0.25rem;
  }

  .repo-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background-color: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    padding: 0.75rem 1rem;
    border-radius: 6px;
  }

  .repo-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .repo-path {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.875rem;
    color: #e2e8f0;
  }

  .unwatch-btn {
    background-color: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: none;
    border-radius: 4px;
    padding: 0.4rem 0.8rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .unwatch-btn:hover {
    background-color: #ef4444;
    color: #f8fafc;
  }

  .empty-text {
    color: #64748b;
    text-align: center;
    margin: 2rem 0;
    font-size: 0.95rem;
  }

  /* File Selector Component */
  .selector-component {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
    overflow: hidden;
    background-color: rgba(11, 15, 25, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    margin-bottom: 1rem;
  }

  .selector-path-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background-color: rgba(255, 255, 255, 0.02);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding: 0.6rem 0.8rem;
    gap: 1rem;
  }

  .path-label {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.85rem;
    color: #06b6d4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-grow: 1;
  }

  .path-bar-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .btn-sm {
    background-color: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #cbd5e1;
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.2s ease;
  }

  .btn-sm:hover:not(:disabled) {
    background-color: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
    color: #f8fafc;
  }

  .btn-sm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .selector-status {
    padding: 2rem 1rem;
    text-align: center;
    color: #64748b;
    font-size: 0.9rem;
  }

  .selector-status.error {
    color: #ef4444;
  }

  .selector-list {
    flex-grow: 1;
    overflow-y: auto;
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .selector-item.current-dir-repo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background-color: rgba(6, 182, 212, 0.06);
    border: 1px dashed rgba(6, 182, 212, 0.3);
    border-radius: 6px;
    padding: 0.6rem 0.8rem;
    margin-bottom: 0.5rem;
  }

  .folder-icon-repo {
    color: #06b6d4;
    font-size: 0.75rem;
    margin-right: 0.5rem;
  }

  .repo-name-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: #06b6d4;
    flex-grow: 1;
  }

  .watch-action-btn {
    background-color: #06b6d4;
    color: #0f172a;
    border: none;
    border-radius: 4px;
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .watch-action-btn:hover:not(:disabled) {
    background-color: #22d3ee;
    box-shadow: 0 0 8px rgba(6, 182, 212, 0.4);
  }

  .watch-action-btn:disabled {
    background-color: rgba(255, 255, 255, 0.05);
    color: #64748b;
    cursor: not-allowed;
    box-shadow: none;
  }

  .empty-selector {
    color: #64748b;
    text-align: center;
    padding: 2rem 1rem;
    font-size: 0.85rem;
  }

  .selector-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    background-color: rgba(255, 255, 255, 0.01);
    border: 1px solid rgba(255, 255, 255, 0.02);
    transition: all 0.15s ease;
  }

  .selector-row:hover {
    background-color: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .entry-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    flex-grow: 1;
    overflow: hidden;
    padding: 0.25rem 0;
  }

  .folder-icon {
    font-size: 0.9rem;
    flex-shrink: 0;
  }

  .entry-name {
    font-size: 0.85rem;
    color: #e2e8f0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .repo-badge {
    background-color: rgba(99, 102, 241, 0.15);
    color: #818cf8;
    border: 1px solid rgba(99, 102, 241, 0.3);
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 0.05rem 0.3rem;
    border-radius: 4px;
    letter-spacing: 0.05em;
  }

  .manual-input-box {
    margin-top: auto;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding-top: 1rem;
    flex-shrink: 0;
  }

  .manual-input-box h3 {
    margin: 0 0 0.5rem 0;
    font-size: 0.9rem;
    font-weight: 500;
    color: #94a3b8;
  }

  .repo-form {
    display: flex;
    gap: 0.75rem;
  }

  .path-input {
    flex-grow: 1;
    background-color: #090d16;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #f8fafc;
    padding: 0.6rem 1rem;
    border-radius: 6px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.875rem;
    outline: none;
  }

  .path-input:focus {
    border-color: #06b6d4;
  }

  .msg-success {
    color: #10b981;
    font-size: 0.875rem;
    margin: 0.75rem 0 0 0;
  }

  .msg-error {
    color: #ef4444;
    font-size: 0.875rem;
    margin: 0.75rem 0 0 0;
  }

  /* Backups View Layout */
  .backups-layout {
    display: grid;
    grid-template-columns: 300px 300px 1fr;
    gap: 1.5rem;
    height: calc(100vh - 180px);
    overflow: hidden;
  }

  .pane-column {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    padding: 1rem !important;
  }

  .pane-header {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 0.75rem;
    margin-bottom: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 50px;
    justify-content: center;
  }

  .pane-header h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: #f8fafc;
  }

  .repo-select {
    width: 100%;
    background-color: #090d16;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #f8fafc;
    padding: 0.4rem;
    border-radius: 4px;
    outline: none;
  }

  .pane-header-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: nowrap;
  }

  .filter-toggle-btn {
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    color: #94a3b8;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    white-space: nowrap;
  }

  .filter-toggle-btn:hover {
    background: rgba(99, 102, 241, 0.15);
    border-color: rgba(99, 102, 241, 0.4);
    color: #c7d2fe;
  }

  .filter-toggle-btn.active {
    background: rgba(99, 102, 241, 0.25);
    border-color: rgba(99, 102, 241, 0.6);
    color: #a5b4fc;
  }

  .filter-toggle-btn.prune-btn:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.4);
    color: #fca5a5;
  }

  .snapshots-list, .files-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    overflow-y: auto;
    flex-grow: 1;
  }

  .snapshot-item {
    background-color: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 6px;
    padding: 0.75rem;
    text-align: left;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .snapshot-item:hover {
    background-color: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .snapshot-item.active {
    background-color: rgba(6, 182, 212, 0.1);
    border-color: #06b6d4;
  }

  .snap-hash {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.8rem;
    font-weight: 700;
    color: #06b6d4;
  }

  .snap-desc {
    font-size: 0.85rem;
    color: #e2e8f0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .snap-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: #64748b;
  }

  .empty-pane-text {
    color: #64748b;
    text-align: center;
    margin: auto;
    font-size: 0.875rem;
    padding: 1rem;
  }

  /* Changed Files Pane */
  .pane-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    width: 100%;
  }

  .pane-action-btn {
    border: none;
    border-radius: 4px;
    padding: 0.35rem;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    text-align: center;
    background-color: #06b6d4;
    color: #0f172a;
    transition: all 0.2s ease;
  }

  .pane-action-btn:disabled {
    background-color: rgba(255, 255, 255, 0.05);
    color: #64748b;
    cursor: not-allowed;
  }

  .pane-action-btn.danger {
    background-color: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .pane-action-btn.danger:hover {
    background-color: #ef4444;
    color: #f8fafc;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background-color: rgba(255, 255, 255, 0.01);
    border: 1px solid rgba(255, 255, 255, 0.03);
    border-radius: 4px;
    padding: 0.4rem;
  }

  .file-item.active {
    background-color: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .file-checkbox {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: #06b6d4;
  }

  .file-path-btn {
    background: none;
    border: none;
    color: #cbd5e1;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.8rem;
    text-align: left;
    cursor: pointer;
    flex-grow: 1;
    display: flex;
    gap: 0.4rem;
    overflow: hidden;
  }

  .file-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file-status {
    font-weight: 700;
  }

  .file-status.added { color: #10b981; }
  .file-status.modified { color: #3b82f6; }
  .file-status.deleted { color: #ef4444; }

  /* Diff pane */
  .diff-viewer {
    background-color: #05070f;
    border-radius: 6px;
    padding: 1rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.75rem;
    color: #e2e8f0;
    overflow: auto;
    flex-grow: 1;
    margin: 0;
    white-space: pre;
    line-height: 1.4;
  }

  /* Alerts */
  .alert {
    padding: 0.75rem 1rem;
    border-radius: 6px;
    font-size: 0.9rem;
    margin-top: 1rem;
  }

  .alert.success {
    background-color: rgba(16, 185, 129, 0.1);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.2);
  }

  .alert.error {
    background-color: rgba(239, 68, 68, 0.1);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.2);
  }

  /* Metrics view */
  .metrics-output {
    background-color: #05070f;
    border-radius: 6px;
    padding: 1rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.85rem;
    color: #10b981;
    overflow: auto;
    margin: 1rem 0 0 0;
    white-space: pre;
    line-height: 1.4;
  }

  .metrics-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 0.5rem;
  }

  .metrics-header h2 {
    margin: 0;
    font-size: 1.2rem;
    color: #f8fafc;
  }

  .metrics-summary-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .summary-card {
    display: flex;
    flex-direction: column;
    padding: 1rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    text-align: center;
  }

  .summary-card .label {
    font-size: 0.8rem;
    color: #64748b;
    margin-bottom: 0.25rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .summary-card .val {
    font-size: 1.4rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .metrics-charts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .chart-card {
    display: flex;
    flex-direction: column;
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .chart-header {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.75rem;
  }

  .chart-header h3 {
    margin: 0;
    font-size: 1rem;
    color: #cbd5e1;
    font-weight: 600;
  }

  .hover-details {
    font-size: 0.75rem;
    min-height: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .hover-title {
    color: #94a3b8;
    font-weight: 500;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hover-info {
    color: #cbd5e1;
  }

  .text-ins {
    color: #10b981;
    font-weight: 600;
  }

  .text-del {
    color: #ef4444;
    font-weight: 600;
  }

  .text-lat {
    color: #3b82f6;
    font-weight: 600;
  }

  .chart-svg-wrapper {
    width: 100%;
    overflow: visible;
  }

  .chart-svg-wrapper svg {
    overflow: visible;
  }

  .chart-rect {
    transition: opacity 0.15s ease;
  }

  .chart-rect:hover {
    opacity: 0.7;
  }

  /* Mode Selector */
  .mode-selector {
    display: flex;
    background-color: rgba(11, 15, 25, 0.6);
    border-radius: 8px;
    padding: 0.25rem;
    gap: 0.25rem;
    margin-bottom: 1.25rem;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .mode-btn {
    flex: 1;
    background: none;
    border: none;
    color: #94a3b8;
    padding: 0.5rem;
    font-size: 0.85rem;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .mode-btn:hover {
    color: #f8fafc;
    background-color: rgba(255, 255, 255, 0.03);
  }

  .mode-btn.active {
    background-color: #06b6d4;
    color: #0f172a;
    font-weight: 600;
  }

  .status-badge.installed {
    background-color: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }

  .status-badge.not-installed {
    background-color: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  .service-running-note {
    background-color: rgba(6, 182, 212, 0.1);
    border: 1px solid rgba(6, 182, 212, 0.2);
    color: #06b6d4;
    border-radius: 6px;
    padding: 0.75rem 1rem;
    font-size: 0.85rem;
    line-height: 1.4;
    margin: 1rem 0;
  }

  .service-note-box {
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.75rem;
    color: #94a3b8;
    background-color: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 4px;
    line-height: 1.4;
    text-align: left;
  }
</style>
