<!-- Toggle: custom switch. Props: checked (bindable), label (visible text, also aria), disabled, onchange(checked). -->
<script>
  let { checked = $bindable(false), label = '', disabled = false, onchange } = $props();
  function flip() { checked = !checked; onchange?.(checked); }
</script>

<button type="button" class="tg" role="switch" aria-checked={checked} {disabled} onclick={flip}>
  <span class="track" class:on={checked}><span class="knob"></span></span>
  {#if label}<span class="lbl">{label}</span>{/if}
</button>

<style>
  .tg { display: inline-flex; align-items: center; gap: 10px; padding: 2px 0; border: 0; background: none; color: var(--text); }
  .tg:disabled { opacity: .45; }
  .track { position: relative; width: 38px; height: 22px; border-radius: 11px; background: rgba(255,255,255,.16); box-shadow: inset 0 0 0 1px var(--glass-border); transition: background var(--med) var(--ease); }
  .track.on { background: var(--accent-strong); }
  .knob { position: absolute; top: 2px; left: 2px; width: 18px; height: 18px; border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,.4); transition: transform var(--med) var(--ease); }
  .on .knob { transform: translateX(16px); }
  .lbl { font-size: var(--t-base); }
</style>
