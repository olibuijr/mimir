<!--
  Segmented: glass segmented control. Props: options [{value,label}], value (bindable), label (aria), onchange(value).
  Keyboard: arrow keys move selection. On narrow screens it scrolls horizontally (thin custom scrollbar).
-->
<script>
  let { options, value = $bindable(), label = '', onchange } = $props();
  function pick(v) { value = v; onchange?.(v); }
  function key(e, i) {
    let j = null;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') j = (i + 1) % options.length;
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') j = (i - 1 + options.length) % options.length;
    if (j === null) return;
    e.preventDefault();
    pick(options[j].value);
    e.currentTarget.parentElement.children[j].focus();
  }
</script>

<div class="seg" role="radiogroup" aria-label={label}>
  {#each options as o, i}
    <button type="button" role="radio" aria-checked={value === o.value} tabindex={value === o.value ? 0 : -1}
      class:on={value === o.value} onclick={() => pick(o.value)} onkeydown={(e) => key(e, i)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg { display: inline-flex; max-width: 100%; padding: 2px; gap: 2px; border-radius: var(--r-sm); background: var(--glass-fill); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); overflow-x: auto; scrollbar-width: thin; }
  .seg::-webkit-scrollbar { height: 4px; }
  button { flex: none; white-space: nowrap; height: 26px; padding: 0 12px; border: 0; border-radius: 7px; background: transparent; color: var(--text-2); font-size: var(--t-sm); font-weight: 550; transition: background var(--fast) var(--ease), color var(--fast); }
  button:hover { color: var(--text); }
  button.on { background: rgba(255,255,255,.14); color: var(--text); box-shadow: 0 1px 3px rgba(0,0,0,.3), inset 0 1px 0 rgba(255,255,255,.12); }
</style>
