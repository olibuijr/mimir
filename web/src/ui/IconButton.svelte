<!--
  IconButton: round-rect icon-only button. Props: icon, label (aria-label + tooltip), size=32, variant='plain'|'glass', active, onclick.
-->
<script>
  import Icon from './Icon.svelte';
  import Tooltip from './Tooltip.svelte';
  let { icon, label, size = 32, variant = 'plain', active = false, onclick, ...rest } = $props();
</script>

<Tooltip text={label}>
  <button class="ib {variant}" class:active type="button" aria-label={label} style="width:{size}px;height:{size}px" {onclick} {...rest}>
    <Icon name={icon} size={Math.round(size * .55)} />
  </button>
</Tooltip>

<style>
  .ib { display: grid; place-items: center; border: 1px solid transparent; border-radius: var(--r-sm); background: transparent; color: var(--text-2); transition: background var(--fast) var(--ease), color var(--fast), transform var(--fast); }
  .ib:hover { background: rgba(255,255,255,.08); color: var(--text); }
  .ib:active { transform: scale(.94); }
  .ib.active { color: var(--accent); background: var(--accent-soft); }
  .glass { background: var(--glass-fill); border-color: var(--glass-border); box-shadow: var(--glass-highlight); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); }
</style>
