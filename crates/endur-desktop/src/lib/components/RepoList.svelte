<script lang="ts">
  import FileSelector, { type SelectorEntry } from "./FileSelector.svelte";

  let {
    watchedRepos = [],
    selectorCurrentDir = "",
    selectorEntries = [],
    selectorLoading = false,
    selectorError = "",
    baseRootDir = "",
    isCurrentDirRepo = false,
    watchlistMessage = "",
    watchlistError = "",
    onRemoveRepo,
    onAddRepo,
    onWatchPath,
    onNavigateSelector,
    onGoUpSelector
  }: {
    watchedRepos: string[];
    selectorCurrentDir: string;
    selectorEntries: SelectorEntry[];
    selectorLoading: boolean;
    selectorError: string;
    baseRootDir: string;
    isCurrentDirRepo: boolean;
    watchlistMessage: string;
    watchlistError: string;
    onRemoveRepo: (path: string) => void;
    onAddRepo: (path: string) => void;
    onWatchPath: (path: string) => void;
    onNavigateSelector: (path: string) => void;
    onGoUpSelector: () => void;
  } = $props();

  let newRepoPath = $state("");

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (newRepoPath.trim()) {
      onAddRepo(newRepoPath.trim());
      newRepoPath = "";
    }
  }
</script>

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
              <button class="unwatch-btn" onclick={() => onRemoveRepo(path)}>Unwatch</button>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Right Column: Interactive File Selector & Manual Form -->
    <div class="card glass add-repo-card">
      <h2>Monitor New Repository</h2>
      
      <FileSelector
        currentDir={selectorCurrentDir}
        entries={selectorEntries}
        loading={selectorLoading}
        error={selectorError}
        baseRootDir={baseRootDir}
        isCurrentDirRepo={isCurrentDirRepo}
        watchedRepos={watchedRepos}
        onNavigate={onNavigateSelector}
        onGoUp={onGoUpSelector}
        onWatch={onWatchPath}
      />

      <!-- Fallback Manual Input -->
      <div class="manual-input-box">
        <h3>Or enter path manually:</h3>
        <form class="repo-form" onsubmit={handleSubmit}>
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

  .card h2 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-top: 0;
    margin-bottom: 1rem;
    color: #f8fafc;
  }

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

  .folder-icon {
    font-size: 0.9rem;
    flex-shrink: 0;
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
</style>
