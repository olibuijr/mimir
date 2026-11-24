<!--
  Sheet: modal glass sheet with focus trap. Esc / backdrop click closes (unless dismissible=false).
  Props: open, title, onclose, dismissible=true, width=440, children, actions (snippet, footer buttons).
-->
<script>
  import IconButton from './IconButton.svelte';
  let { open = false, title = '', onclose, dismissible = true, width = 440, children, actions } = $props();
  let el = $state();
  let prev;
  const sel = 'a[href],button:not([disabled]),input:not([disabled]),textarea,[tabindex]:not([tabindex="-1"])';

  $effect(() => {
    if (!open || !el) return;
    prev = document.activeElement;
    (el.querySelector('input,textarea') || el.querySelector(sel) || el).focus();
    return () => { if (prev && prev.focus) prev.focus(); };
  });

  function key(e) {
    if (e.key === 'Escape' && dismissible) { e.stopPropagation(); onclose?.(); return; }
    if (e.key !== 'Tab') return;
    const f = [...el.querySelectorAll(sel)].filter((n) => n.offsetParent !== null);
    if (!f.length) { e.preventDefault(); return; }
    const a = f[0], z = f[f.length - 1];
    if (e.shiftKey && (document.activeElement === a || document.activeElement === el)) { e.preventDefault(); z.focus(); }
    else if (!e.shiftKey && document.activeElement === z) { e.preventDefault(); a.focus(); }
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onpointerdown={(e) => { if (e.target === e.currentTarget && dismissible) onclose?.(); }}>
    <div class="sheet" bind:this={el} role="dialog" aria-modal="true" aria-label={title} tabindex="-1" style="max-width:{width}px" onkeydown={key}>
      <div class="hd">
        <h2>{title}</h2>
        {#if dismissible}<IconButton icon="close" label="Close" size={28} onclick={() => onclose?.()} />{/if}
      </div>
      <div class="bd">{@render children?.()}</div>
      {#if actions}<div class="ft">{@render actions()}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; z-index: 70; display: grid; place-items: center; padding: var(--s-4); background: rgba(4,6,10,.55); backdrop-filter: blur(4px); -webkit-backdrop-filter: blur(4px); animation: fade var(--med) var(--ease); }
  .sheet { width: 100%; max-height: 88vh; display: flex; flex-direction: column; border-radius: var(--r-lg); outline: 0;
    background: var(--glass-fill-strong); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight), var(--shadow-lg); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); animation: rise var(--med) var(--ease); }
  .hd { display: flex; align-items: center; justify-content: space-between; padding: var(--s-4) var(--s-4) var(--s-2) var(--s-5); }
  .bd { padding: var(--s-2) var(--s-5) var(--s-5); overflow-y: auto; display: flex; flex-direction: column; gap: var(--s-3); }
  .ft { display: flex; justify-content: flex-end; gap: var(--s-2); padding: var(--s-3) var(--s-5) var(--s-4); border-top: 1px solid var(--line); }
  @keyframes fade { from { opacity: 0; } }
  @keyframes rise { from { opacity: 0; transform: translateY(10px) scale(.98); } }
</style>
