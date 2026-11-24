<!--
  Tooltip: custom replacement for title=. Wraps children; shows on hover, keyboard focus and touch tap.
  Never leaves the viewport (flips above/below, clamps horizontally) and wraps long text.
  Props: text, side='top'|'bottom', focusable=false (make the wrapper a tab stop when the child is not), children.
-->
<script>
  let { text, side = 'top', focusable = false, children } = $props();
  let show = $state(false);
  let wrap = $state(), tip = $state();
  let timer;
  let touch = false;
  let pos = $state({ left: 0, top: null, bottom: null, ready: false });
  const id = `tip-${Math.random().toString(36).slice(2, 8)}`;

  const on = () => { clearTimeout(timer); timer = setTimeout(() => (show = true), 350); };
  const now = () => { clearTimeout(timer); show = true; };
  const off = () => { clearTimeout(timer); show = false; };

  $effect(() => {
    if (!show || !wrap || !tip) { pos.ready = false; return; }
    const w = wrap.getBoundingClientRect();
    const t = tip.getBoundingClientRect();
    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    const gap = 6, m = 8;
    let above = side === 'top';
    if (above && w.top - t.height - gap < m && vh - w.bottom - t.height - gap >= m) above = false;
    else if (!above && w.bottom + t.height + gap > vh - m && w.top - t.height - gap >= m) above = true;
    const want = Math.min(Math.max(w.left + w.width / 2 - t.width / 2, m), Math.max(m, vw - m - t.width));
    pos = { left: want - w.left, top: above ? null : '100%', bottom: above ? '100%' : null, above, ready: true };
  });
</script>

<svelte:window onpointerdown={(e) => { if (show && wrap && !wrap.contains(e.target)) off(); }} onscroll={off} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<span class="tw" bind:this={wrap} onmouseenter={on} onmouseleave={off} onfocusin={now} onfocusout={off}
  onpointerdown={(e) => (touch = e.pointerType === 'touch')}
  onclick={() => { if (touch) show = !show; }}
  onkeydown={(e) => e.key === 'Escape' && off()}
  tabindex={focusable ? 0 : undefined} role={focusable ? 'button' : 'presentation'} aria-describedby={show ? id : undefined}>
  {@render children?.()}
  {#if show && text}
    <span class="tip" {id} role="tooltip" bind:this={tip} class:ready={pos.ready}
      style="left:{pos.left}px;{pos.top ? `top:calc(${pos.top} + 6px);` : ''}{pos.bottom ? `bottom:calc(${pos.bottom} + 6px);` : ''}">{text}</span>
  {/if}
</span>

<style>
  .tw { position: relative; display: inline-flex; }
  .tip {
    position: absolute; z-index: 90; pointer-events: none; visibility: hidden;
    width: max-content; max-width: min(320px, calc(100vw - 16px)); white-space: normal; overflow-wrap: anywhere; text-align: left;
    padding: 5px 10px; border-radius: var(--r-xs); font-size: var(--t-xs); line-height: 1.4; color: var(--text);
    background: var(--glass-fill-strong); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight), var(--shadow-md);
    backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
  }
  .tip.ready { visibility: visible; animation: in var(--fast) var(--ease); }
  @keyframes in { from { opacity: 0; } }
</style>
