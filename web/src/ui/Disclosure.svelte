<!-- Disclosure: collapsible block. Props: label (summary text), open (bindable, default false), children. -->
<script>
  import Icon from './Icon.svelte';
  let { label = '', open = $bindable(false), children } = $props();
</script>
<div class="d">
  <button type="button" class="h" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="car" class:open><Icon name="chevron" size={12} /></span>{label}
  </button>
  {#if open}<div class="b">{@render children?.()}</div>{/if}
</div>
<style>
  .h { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; padding: 2px 0; color: var(--text-2); font-size: var(--t-sm); font-weight: 550; }
  .h:hover { color: var(--text); }
  .car { display: inline-flex; color: var(--text-3); transition: transform var(--fast) var(--ease); }
  .car.open { transform: rotate(90deg); }
  .b { margin-top: var(--s-2); }
</style>
