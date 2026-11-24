<!-- StatTile: compact stat badge. Badges share one row equally and shrink to fit; never scroll.
     Props: label, value, sub, spark (number[] optional), tone='neutral'|'good'|'warn'. -->
<script>
  import Sparkline from './Sparkline.svelte';
  let { label, value, sub = '', spark = null, tone = 'neutral' } = $props();
</script>
<div class="badge" role="group" aria-label="{label}: {value}{sub ? `, ${sub}` : ''}">
  <span class="l">{label}</span>
  <span class="v {tone}">{value}</span>
  {#if sub}<span class="s">{sub}</span>{/if}
  {#if spark && spark.length > 1}<span class="sp"><Sparkline values={spark} width={44} height={16} /></span>{/if}
</div>
<style>
  .badge {
    flex: 1 1 0; min-width: 0;
    display: flex; align-items: center; gap: 8px;
    height: 34px; padding: 0 12px; border-radius: var(--r-xs);
    background: var(--surface); border: 1px solid var(--line);
    white-space: nowrap; overflow: hidden;
  }
  .l { color: var(--text-3); font-size: var(--t-xs); font-weight: 550; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .v { font-size: var(--t-md); font-weight: 650; letter-spacing: -.01em; font-variant-numeric: tabular-nums; color: var(--text); flex: none; }
  .good { color: var(--good); } .warn { color: var(--warn); }
  .s { color: var(--text-2); font-size: var(--t-xs); font-variant-numeric: tabular-nums; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .sp { display: inline-flex; margin-left: auto; flex: none; opacity: .9; }
  /* Narrow screens: label above value, extras dropped, so four badges still fit one row. */
  @media (max-width: 720px) {
    .badge { flex-direction: column; align-items: flex-start; justify-content: center; gap: 0; height: 44px; padding: 0 9px; }
    .v { font-size: var(--t-base); }
    .s, .sp { display: none; }
  }
</style>
