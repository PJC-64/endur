<script lang="ts">
  let {
    selectedRepo = "",
    watchedRepos = [],
    snapshots = [],
    selectedSnapshot = null,
    snapshotFiles = [],
    selectedFiles = new Set(),
    activeFileDiff = "",
    selectedFileForDiff = "",
    backupMessage = "",
    backupError = "",
    showAllSnapshots = false,
    onSelectRepo,
    onToggleSnapshotFilter,
    onPruneSnapshots,
    onSelectSnapshot,
    onRestoreSelected,
    onRestoreFull,
    onToggleFileSelection,
    onViewFileDiff
  }: {
    selectedRepo: string;
    watchedRepos: string[];
    snapshots: any[];
    selectedSnapshot: any;
    snapshotFiles: [string, string][];
    selectedFiles: Set<string>;
    activeFileDiff: string;
    selectedFileForDiff: string;
    backupMessage: string;
    backupError: string;
    showAllSnapshots: boolean;
    onSelectRepo: (repo: string) => void;
    onToggleSnapshotFilter: () => void;
    onPruneSnapshots: () => void;
    onSelectSnapshot: (snap: any) => void;
    onRestoreSelected: () => void;
    onRestoreFull: () => void;
    onToggleFileSelection: (filePath: string) => void;
    onViewFileDiff: (filePath: string) => void;
  } = $props();
</script>

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
            onclick={onToggleSnapshotFilter}
            title={showAllSnapshots ? 'Showing all snapshots — click to show only current HEAD' : 'Showing snapshots since last commit — click to show all'}
          >
            {showAllSnapshots ? '🕰 All' : '📌 HEAD'}
          </button>
          <button
            class="filter-toggle-btn prune-btn"
            onclick={onPruneSnapshots}
            disabled={!selectedRepo}
            title="Prune historical snapshots to reclaim disk space"
          >
            ✂️ Prune
          </button>
          <select value={selectedRepo} onchange={(e) => onSelectRepo(e.currentTarget.value)} class="repo-select">
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
              onclick={() => onSelectSnapshot(snap)}
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
            <button class="pane-action-btn" onclick={onRestoreSelected} disabled={selectedFiles.size === 0}>
              Restore Selected ({selectedFiles.size})
            </button>
            <button class="pane-action-btn danger" onclick={onRestoreFull}>
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
                onchange={() => onToggleFileSelection(filePath)}
                class="file-checkbox"
              />
              <button class="file-path-btn" onclick={() => onViewFileDiff(filePath)}>
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

<style>
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

  .card.glass {
    background: rgba(15, 23, 42, 0.6);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.5rem;
    box-sizing: border-box;
  }

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
</style>
