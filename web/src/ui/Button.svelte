<!--
  Button. Props: variant='glass'|'primary'|'plain'|'danger', size='sm'|'md'|'lg', disabled, loading,
  type='button', onclick, children. Extra attributes (aria-*) are passed through.
-->
<script>
  import Spinner from './Spinner.svelte';
  let { variant = 'glass', size = 'md', disabled = false, loading = false, type = 'button', onclick, children, ...rest } = $props();
</script>

<button class="btn {variant} {size}" {type} disabled={disabled || loading} {onclick} {...rest}>
  {#if loading}<Spinner size={14} />{/if}
  {@render children?.()}
</button>

<style>
  .btn {
    display: inline-flex; align-items: center; justify-content: center; gap: 7px; white-space: nowrap;
    border: 1px solid transparent; border-radius: var(--r-sm); font-weight: 550; letter-spacing: -.005em;
    transition: background var(--fast) var(--ease), border-color var(--fast) var(--ease), transform var(--fast) var(--ease), opacity var(--fast);
    -webkit-tap-highlight-color: transparent; user-select: none;
  }
  .btn:active:not(:disabled) { transform: scale(.97); }
  .btn:disabled { opacity: .45; cursor: default; }
  .sm { height: 26px; padding: 0 10px; font-size: var(--t-sm); border-radius: var(--r-xs); }
  .md { height: 32px; padding: 0 14px; font-size: var(--t-base); }
  .lg { height: 40px; padding: 0 20px; font-size: var(--t-md); border-radius: var(--r-md); }
  .glass { background: var(--glass-fill); border-color: var(--glass-border); box-shadow: var(--glass-highlight); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); }
  .glass:hover:not(:disabled) { background: rgba(255,255,255,.10); }
  .primary { background: linear-gradient(180deg, #7ea5ff, #5b8cff); color: #07101f; box-shadow: inset 0 1px 0 rgba(255,255,255,.4), 0 2px 10px rgba(91,140,255,.3); }
  .primary:hover:not(:disabled) { background: linear-gradient(180deg, #92b3ff, #6b97ff); }
  .plain { background: transparent; color: var(--accent); }
  .plain:hover:not(:disabled) { background: var(--accent-soft); }
  .danger { background: var(--bad-soft); color: var(--bad); border-color: rgba(255,107,122,.25); }
  .danger:hover:not(:disabled) { background: rgba(255,107,122,.24); }
</style>
