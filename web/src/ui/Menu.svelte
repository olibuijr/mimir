<!--
  Menu: custom dropdown (replaces <select>). Props: options [{value,label}], value (bindable), label (aria + placeholder),
  onchange(value), align='left'|'right'. Keyboard: Enter/Space/ArrowDown opens, arrows move, Enter picks, Esc closes, letters jump.
-->
<script>
  import Icon from './Icon.svelte';
  let { options, value = $bindable(), label = 'Select', onchange, align = 'left' } = $props();
  let open = $state(false);
  let idx = $state(0);
  let root = $state(), list = $state();
  const cur = $derived(options.find((o) => o.value === value));

  function show() { idx = Math.max(0, options.findIndex((o) => o.value === value)); open = true; queueMicrotask(() => list?.focus()); }
  function hide(refocus = true) { open = false; if (refocus) root?.querySelector('.trig')?.focus(); }
  function pick(o) { value = o.value; onchange?.(o.value); hide(); }
  function key(e) {
    if (e.key === 'Escape') { e.preventDefault(); hide(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); idx = (idx + 1) % options.length; }
    else if (e.key === 'ArrowUp') { e.preventDefault(); idx = (idx - 1 + options.length) % options.length; }
    else if (e.key === 'Home') { e.preventDefault(); idx = 0; }
    else if (e.key === 'End') { e.preventDefault(); idx = options.length - 1; }
    else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(options[idx]); }
    else if (e.key === 'Tab') hide(false);
    else if (e.key.length === 1) {
      const j = options.findIndex((o, k) => k > idx && o.label.toLowerCase().startsWith(e.key.toLowerCase()));
      const f = j >= 0 ? j : options.findIndex((o) => o.label.toLowerCase().startsWith(e.key.toLowerCase()));
      if (f >= 0) idx = f;
    }
  }
  function outside(e) { if (open && root && !root.contains(e.target)) hide(false); }
</script>

<svelte:window onpointerdown={outside} />

<div class="menu" bind:this={root}>
  <button type="button" class="trig" aria-haspopup="listbox" aria-expanded={open} aria-label={label}
    onclick={() => (open ? hide() : show())}
    onkeydown={(e) => { if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); show(); } }}>
    <span class="cur">{cur ? cur.label : label}</span>
    <span class="car" class:flip={open}><Icon name="chevron" size={13} /></span>
  </button>
  {#if open}
    <div class="pop {align}" role="listbox" tabindex="-1" bind:this={list} aria-label={label} aria-activedescendant="mo-{idx}" onkeydown={key}>
      {#each options as o, i}
        <div id="mo-{i}" class="opt" class:hi={i === idx} class:sel={o.value === value} role="option" aria-selected={o.value === value} tabindex="-1"
          onpointerenter={() => (idx = i)} onclick={() => pick(o)} onkeydown={() => {}}>
          <span>{o.label}</span>{#if o.value === value}<Icon name="check" size={14} />{/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu { position: relative; display: inline-block; }
  .trig { display: inline-flex; align-items: center; gap: 8px; height: 32px; padding: 0 10px 0 12px; border-radius: var(--r-sm); border: 1px solid var(--glass-border); background: var(--glass-fill); box-shadow: var(--glass-highlight); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); font-weight: 520; transition: background var(--fast); }
  .trig:hover { background: rgba(255,255,255,.1); }
  .car { display: inline-flex; transform: rotate(90deg); color: var(--text-3); transition: transform var(--fast) var(--ease); }
  .car.flip { transform: rotate(-90deg); }
  .pop { position: absolute; top: calc(100% + 6px); z-index: 80; min-width: 100%; max-height: 280px; overflow-y: auto; padding: 4px; border-radius: var(--r-md); outline: 0;
    background: var(--glass-fill-strong); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight), var(--shadow-lg); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); animation: pop var(--fast) var(--ease); }
  .left { left: 0; } .right { right: 0; }
  .opt { display: flex; align-items: center; justify-content: space-between; gap: 14px; padding: 6px 10px; border-radius: var(--r-xs); white-space: nowrap; cursor: pointer; }
  .opt.hi { background: var(--accent-strong); color: #fff; }
  .opt.sel:not(.hi) { color: var(--accent); }
  @keyframes pop { from { opacity: 0; transform: translateY(-4px) scale(.98); } }
</style>
