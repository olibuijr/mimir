<!-- Section: titled content block. Props: title, hint (small text right of title), children, actions (snippet). -->
<script>
  let { title = '', hint = '', children, actions } = $props();
</script>
<section>
  {#if title || actions}
    <div class="hd"><h3>{title}</h3>{#if hint}<span class="hint">{hint}</span>{/if}<div class="act">{@render actions?.()}</div></div>
  {/if}
  {@render children?.()}
</section>
<style>
  section { display: flex; flex-direction: column; gap: var(--s-3); }
  .hd { display: flex; align-items: baseline; gap: var(--s-3); }
  h3 { font-size: var(--t-sm); font-weight: 600; color: var(--text-2); text-transform: uppercase; letter-spacing: .06em; }
  .hint { color: var(--text-3); font-size: var(--t-sm); }
  .act { margin-left: auto; display: flex; gap: var(--s-2); align-items: center; }
</style>
