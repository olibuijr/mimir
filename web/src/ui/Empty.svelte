<!-- Empty: empty/error state. Props: title, hint, icon='search', children (optional action). -->
<script>
  import Icon from './Icon.svelte';
  let { title, hint = '', icon = 'search', children } = $props();
</script>
<div class="em">
  <div class="ic"><Icon name={icon} size={22} /></div>
  <h3>{title}</h3>
  {#if hint}<p>{hint}</p>{/if}
  {@render children?.()}
</div>
<style>
  .em { display: flex; flex-direction: column; align-items: center; gap: var(--s-2); padding: var(--s-7) var(--s-5); text-align: center; color: var(--text-2); }
  .ic { display: grid; place-items: center; width: 48px; height: 48px; border-radius: 50%; background: rgba(255,255,255,.06); color: var(--text-3); margin-bottom: var(--s-2); }
  h3 { color: var(--text); }
  p { max-width: 38ch; color: var(--text-3); font-size: var(--t-sm); }
</style>
