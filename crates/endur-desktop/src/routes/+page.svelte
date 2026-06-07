<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  // Tab State
  let activeTab: "dashboard" | "repos" | "backups" | "metrics" = $state("dashboard");

  // Daemon Status State
  let daemonStatus = $state({ running: false, pid: null, uptime_secs: null });

  // Watchlist State
  let watchedRepos: string[] = $state([]);
  let newRepoPath = $state("");
  let watchlistError = $state("");
  let watchlistMessage = $state("");

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

  // Metrics State
  let metricsText = $state("");

  // Real-time Logs State
  let logsText = $state("");

  // Polling timers
  let statusInterval: any;
  let logsInterval: any;

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
      snapshots = await invoke("get_snapshots", { repoPath: selectedRepo });
      selectedSnapshot = null;
      snapshotFiles = [];
      activeFileDiff = "";
      selectedFiles.clear();
    } catch (e) {
      console.error("Failed to load snapshots", e);
    }
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

  async function loadMetrics() {
    try {
      metricsText = await invoke("get_metrics_summary", { humanReadable: true });
    } catch (e) {
      console.error("Failed to load metrics", e);
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
    loadDaemonStatus();
    loadRepos();
    loadLogs();
    
    statusInterval = setInterval(loadDaemonStatus, 2000);
    logsInterval = setInterval(() => {
      if (activeTab === "dashboard") {
        loadLogs();
      }
    }, 1500);

    return () => {
      clearInterval(statusInterval);
      clearInterval(logsInterval);
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
      <div class="status-indicator" class:running={daemonStatus.running}>
        <span class="dot"></span>
        <span class="status-text">{daemonStatus.running ? "Daemon Active" : "Daemon Inactive"}</span>
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
            <div class="status-metric">
              <span class="metric-label">Status:</span>
              <span class="metric-val status-badge" class:active={daemonStatus.running}>
                {daemonStatus.running ? "RUNNING" : "STOPPED"}
              </span>
            </div>
            <div class="status-metric">
              <span class="metric-label">Process ID:</span>
              <span class="metric-val">{daemonStatus.pid || "N/A"}</span>
            </div>
            <div class="status-metric">
              <span class="metric-label">Uptime:</span>
              <span class="metric-val">{formatUptime(daemonStatus.uptime_secs)}</span>
            </div>
            <button 
              class="action-btn" 
              class:stop={daemonStatus.running}
              onclick={toggleDaemon}
            >
              {daemonStatus.running ? "Terminate Daemon" : "Launch Daemon"}
            </button>
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

        <!-- Watch Repo Form -->
        <div class="card glass add-repo-card">
          <h2>Monitor New Repository</h2>
          <form class="repo-form" onsubmit={(e) => { e.preventDefault(); addRepo(); }}>
            <input 
              type="text" 
              placeholder="Enter absolute repository path..." 
              bind:value={newRepoPath} 
              class="path-input"
            />
            <button type="submit" class="action-btn">Watch Folder</button>
          </form>
          {#if watchlistMessage}
            <p class="msg-success">{watchlistMessage}</p>
          {/if}
          {#if watchlistError}
            <p class="msg-error">{watchlistError}</p>
          {/if}
        </div>

        <!-- Repos List -->
        <div class="card glass repos-list-card">
          <h2>Active Repositories</h2>
          {#if watchedRepos.length === 0}
            <p class="empty-text">No active repositories registered. Use the watch input above to add folders.</p>
          {:else}
            <div class="repos-list">
              {#each watchedRepos as path}
                <div class="repo-item">
                  <div class="repo-info">
                    <span class="folder-icon">📁</span>
                    <span class="repo-path">{path}</span>
                  </div>
                  <button class="unwatch-btn" onclick={() => removeRepo(path)}>Unwatch</button>
                </div>
              {/each}
            </div>
          {/if}
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
              <h3>Snapshots</h3>
              <select bind:value={selectedRepo} onchange={loadSnapshots} class="repo-select">
                {#each watchedRepos as path}
                  <option value={path}>{path.split('/').pop() || path}</option>
                {/each}
              </select>
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

  .repos-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
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
</style>
