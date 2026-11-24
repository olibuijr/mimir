<!--
  ListRow: row inside ListGroup. Props: href (hash path, makes it a link), onclick (makes it a button),
  chevron (show on interactive rows, default true), children (main content), trailing (snippet).
-->
<script>
  import Icon from './Icon.svelte';
  import { href as mk } from '../lib/router.svelte.js';
  let { href = '', onclick, chevron = true, children, trailing } = $props();
</script>

{#if href}
  <a class="row act" href={mk(href)}>
    <div class="main">{@render children?.()}</div>{@render trailing?.()}{#if chevron}<span class="ch"><Icon name="chevron" size={15} /></span>{/if}
  </a>
{:else if onclick}
  <button class="row act" type="button" {onclick}>
    <div class="main">{@render children?.()}</div>{@render trailing?.()}{#if chevron}<span class="ch"><Icon name="chevron" size={15} /></span>{/if}
  </button>
{:else}
  <div class="row"><div class="main">{@render children?.()}</div>{@render trailing?.()}</div>
{/if}

<style>
  .row { display: flex; align-items: center; gap: var(--s-3); width: 100%; min-height: 48px; padding: 10px var(--s-4); border: 0; background: none; color: var(--text); text-align: left; text-decoration: none; transition: background var(--fast) var(--ease); }
  .act { cursor: pointer; }
  .act:hover { background: var(--surface-hover); text-decoration: none; }
  .act:focus-visible { outline-offset: -2px; }
  .main { flex: 1; min-width: 0; }
  .ch { color: var(--text-3); display: inline-flex; }
</style>
