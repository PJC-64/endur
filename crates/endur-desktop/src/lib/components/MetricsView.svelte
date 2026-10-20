<script lang="ts">
  let {
    rawMetrics = [],
    metricsText = "",
    onRefresh
  }: {
    rawMetrics: any[];
    metricsText: string;
    onRefresh: () => void;
  } = $props();

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
</script>

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
      <button class="console-refresh" onclick={onRefresh}>Refresh Stats</button>
    </div>
    <pre class="metrics-output">{metricsText || "Generating metrics. Ensure backup logs are active."}</pre>
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

  .text-muted {
    color: #64748b;
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
</style>
