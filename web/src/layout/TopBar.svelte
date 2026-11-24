<!--
  TopBar: sticky glass header inside the scrolling main area.
  Props: title (string), subtitle (string, optional), back (hash path like "/", shows a back chevron),
         children (snippet: actions / search, right-aligned).
-->
<script>
  import Icon from '../ui/Icon.svelte';
  import { href } from '../lib/router.svelte.js';
  import { live } from '../lib/live.svelte.js';
  let { title = '', subtitle = '', back = '', children } = $props();
</script>

<header class="bar">
  {#if back}<a class="back" href={href(back)} aria-label="Back"><Icon name="chevron" size={18} /></a>{/if}
  <div class="t">
    <h1>{title}</h1>
    {#if subtitle}<span class="sub">{subtitle}</span>{/if}
  </div>
  <span class="lv" class:ok={live.connected} role="status" aria-label={live.connected ? 'Live' : 'Offline'}><span class="d"></span>{live.connected ? 'Live' : 'Offline'}</span>
  <div class="act">{@render children?.()}</div>
</header>

<style>
  .bar {
    position: sticky; top: 0; z-index: 20; display: flex; align-items: center; gap: var(--s-3);
    min-height: 60px; padding: var(--s-3) var(--s-5);
    background: var(--glass-fill-strong); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
    border-bottom: 1px solid var(--glass-border); box-shadow: var(--glass-highlight);
  }
  .t { display: flex; align-items: baseline; gap: var(--s-3); min-width: 0; }
  h1 { font-size: var(--t-lg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { color: var(--text-3); font-size: var(--t-sm); white-space: nowrap; }
  .act { margin-left: auto; display: flex; align-items: center; gap: var(--s-2); min-width: 0; }
  .back { display: grid; place-items: center; width: 30px; height: 30px; border-radius: var(--r-sm); color: var(--text-2); transform: rotate(180deg); transition: background var(--fast); }
  .back:hover { background: rgba(255,255,255,.08); text-decoration: none; }
  .lv { display: none; align-items: center; gap: 5px; font-size: var(--t-xs); color: var(--text-3); }
  .lv .d { width: 7px; height: 7px; border-radius: 50%; background: var(--bad); }
  .lv.ok .d { background: var(--good); box-shadow: 0 0 6px var(--good); }
  @media (max-width: 720px) {
    .lv { display: inline-flex; margin-left: auto; order: 2; } .act { margin-left: 0; order: 3; flex: 1 1 100%; } .bar { padding: var(--s-3) var(--s-4); flex-wrap: wrap; } .sub { display: none; } }
</style>
