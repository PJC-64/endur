<script lang="ts">
  export interface SelectorEntry {
    name: string;
    path: string;
    is_repo: boolean;
  }

  let {
    currentDir = "",
    entries = [],
    loading = false,
    error = "",
    baseRootDir = "",
    isCurrentDirRepo = false,
    watchedRepos = [],
    onNavigate,
    onGoUp,
    onWatch
  }: {
    currentDir: string;
    entries: SelectorEntry[];
    loading: boolean;
    error: string;
    baseRootDir: string;
    isCurrentDirRepo: boolean;
    watchedRepos: string[];
    onNavigate: (path: string) => void;
    onGoUp: () => void;
    onWatch: (path: string) => void;
  } = $props();
</script>

<div class="selector-component">
  <div class="selector-path-bar">
    <span class="path-label" title={currentDir}>Current: {currentDir || "Loading..."}</span>
    <div class="path-bar-actions">
      <button class="btn-sm" onclick={onGoUp} disabled={loading || !currentDir || currentDir === "/"}>↱ Up</button>
      <button class="btn-sm" onclick={() => onNavigate(baseRootDir)} disabled={loading || !baseRootDir} title="Go to default base root directory">🏠 Base</button>
    </div>
  </div>

  {#if loading}
    <div class="selector-status">Loading directory entries...</div>
  {:else if error}
    <div class="selector-status error">{error}</div>
  {:else}
    <div class="selector-list">
      {#if isCurrentDirRepo}
        <div class="selector-item current-dir-repo">
          <span class="folder-icon-repo">●</span>
          <span class="repo-name-label">Current Folder is Git Repo</span>
          <button 
            class="watch-action-btn" 
            onclick={() => onWatch(currentDir)}
            disabled={watchedRepos.includes(currentDir)}
          >
            {watchedRepos.includes(currentDir) ? "Watched" : "Watch Current"}
          </button>
        </div>
      {/if}

      {#if entries.length === 0}
        <div class="empty-selector">No subdirectories containing Git repositories found.</div>
      {:else}
        {#each entries as entry}
          <div class="selector-row">
            <div class="entry-info" onclick={() => onNavigate(entry.path)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === 'Enter') onNavigate(entry.path); }}>
              <span class="folder-icon">📁</span>
              <span class="entry-name">{entry.name}</span>
              {#if entry.is_repo}
                <span class="repo-badge">git</span>
              {/if}
            </div>
            {#if entry.is_repo}
              <button 
                class="watch-action-btn"
                onclick={() => onWatch(entry.path)}
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

<style>
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
</style>
