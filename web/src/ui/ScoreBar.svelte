<!--
  ScoreBar: 0..1 bar. Props: value, threshold (optional tick), label=true (numeric label), width (css, default 100%).
  Tone: >=0.5 good, >=0.2 accent, else warn. A bar below threshold is dimmed.
-->
<script>
  import { score } from '../lib/format.js';
  let { value, threshold = null, label = true, width = '100%', name = 'Score' } = $props();
  const v = $derived(Math.max(0, Math.min(1, value ?? 0)));
  const tone = $derived(v >= 0.5 ? 'good' : v >= 0.2 ? 'accent' : 'warn');
</script>
<div class="sb" style="width:{width}">
  <div class="track" role="meter" aria-label={name} aria-valuemin="0" aria-valuemax="1" aria-valuenow={v} aria-valuetext={score(v)}>
    <div class="fill {tone}" class:dim={threshold != null && v < threshold} style="width:{v * 100}%"></div>
    {#if threshold != null}<div class="tick" style="left:{Math.max(0, Math.min(1, threshold)) * 100}%"></div>{/if}
  </div>
  {#if label}<span class="n">{score(value)}</span>{/if}
</div>
<style>
  .sb { display: flex; align-items: center; gap: var(--s-2); }
  .track { position: relative; flex: 1; height: 6px; border-radius: 3px; background: rgba(255,255,255,.08); }
  .fill { height: 100%; border-radius: 3px; transition: width var(--med) var(--ease); }
  .good { background: var(--good); } .accent { background: var(--accent); } .warn { background: var(--warn); }
  .dim { opacity: .45; }
  .tick { position: absolute; top: -3px; bottom: -3px; width: 2px; margin-left: -1px; border-radius: 1px; background: var(--text); opacity: .7; }
  .n { font-family: var(--mono); font-size: var(--t-xs); color: var(--text-2); min-width: 32px; text-align: right; }
</style>
