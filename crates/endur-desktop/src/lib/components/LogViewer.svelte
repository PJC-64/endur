<script lang="ts">
  let {
    logsText = "",
    onRefresh
  }: {
    logsText: string;
    onRefresh: () => void;
  } = $props();
</script>

<div class="console-box card glass">
  <div class="console-header">
    <h3>Live Daemon Logs</h3>
    <button class="console-refresh" onclick={onRefresh}>Refresh</button>
  </div>
  <pre class="console-body">{logsText || "No logs available. Ensure daemon is serving."}</pre>
</div>

<style>
  .card.glass {
    background: rgba(15, 23, 42, 0.6);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.5rem;
    box-sizing: border-box;
  }

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
    transition: all 0.2s ease;
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
</style>
