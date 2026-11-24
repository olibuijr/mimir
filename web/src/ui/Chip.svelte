<!-- Chip: small label pill. Props: tone='neutral'|'accent'|'warn'|'good', mono, children. -->
<script>
  let { tone = 'neutral', mono = false, children } = $props();
</script>
<span class="chip {tone}" class:mono>{@render children?.()}</span>
<style>
  .chip { display: inline-flex; align-items: center; gap: 4px; height: 20px; padding: 0 8px; border-radius: var(--r-pill); font-size: var(--t-xs); font-weight: 550; white-space: nowrap; background: rgba(255,255,255,.07); color: var(--text-2); }
  .mono { font-family: var(--mono); font-weight: 450; }
  .accent { background: var(--accent-soft); color: var(--accent); }
  .warn { background: var(--warn-soft); color: var(--warn); }
  .good { background: var(--good-soft); color: var(--good); }
</style>
