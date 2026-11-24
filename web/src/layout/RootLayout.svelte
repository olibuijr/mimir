<!--
  RootLayout: glass sidebar (nav, version, models status) + scrolling main area.
  Under 720px the sidebar becomes a glass bottom tab bar.
  Props: config (from /api/config or null), children.
-->
<script>
  import { route, href } from '../lib/router.svelte.js';
  import { logout } from '../lib/api.js';
  import { auth } from '../lib/auth.svelte.js';
  import Icon from '../ui/Icon.svelte';
  import Tooltip from '../ui/Tooltip.svelte';

  import { live, retry } from '../lib/live.svelte.js';
  import { ago } from '../lib/format.js';
  import Button from '../ui/Button.svelte';
  let { config = null, children } = $props();

  const nav = [
    { name: 'library', label: 'Library', icon: 'library', path: '/' },
    { name: 'playground', label: 'Playground', icon: 'flask', path: '/playground' },
    { name: 'activity', label: 'Activity', icon: 'activity', path: '/activity' },
  ];
  const current = $derived(route.name === 'skill' ? 'library' : route.name);

  async function signOut() {
    try { await logout(); } catch {}
    auth.needed = true;
  }
</script>

<div class="root">
  <nav class="side" aria-label="Main">
    <div class="brand"><span class="logo" aria-hidden="true"></span><span class="bname">Mímir</span></div>
    <div class="items">
      {#each nav as n}
        <a class="item" class:on={current === n.name} href={href(n.path)} aria-current={current === n.name ? 'page' : undefined}>
          <Icon name={n.icon} size={18} /><span>{n.label}</span>
        </a>
      {/each}
    </div>
    <div class="foot">
      {#if config}
        <Tooltip text={config.models_ok ? 'Models reachable' : 'Models unreachable'}>
          <span class="status"><span class="dot" class:ok={config.models_ok} class:bad={!config.models_ok}></span>Models</span>
        </Tooltip>
        <Tooltip text={live.connected ? 'Live: updates appear as they happen' : 'Reconnecting to live updates'}>
          <span class="status"><span class="dot" class:ok={live.connected} class:pulse={live.connected}></span>Live</span>
        </Tooltip>
        {#if config.reembed_needed}
          <Tooltip text="Stored embeddings were made with a different model or size. Re-embed from the Library."><span class="status"><span class="dot warn"></span>Re-embed</span></Tooltip>
        {/if}
        <span class="ver">v{config.version}</span>
      {/if}
      <Tooltip text="Sign out"><button class="out" onclick={signOut} aria-label="Sign out"><Icon name="logout" size={16} /></button></Tooltip>
    </div>
    {#if config}
      <div class="bk">{config.backup_at ? `Backup ${ago(config.backup_at)}` : 'No backup yet'}</div>
    {/if}
  </nav>
  {#if live.down && !auth.needed}
    <div class="recon" role="status">
      <span class="dot bad"></span>Reconnecting…<Button size="sm" variant="plain" onclick={retry}>Retry</Button>
    </div>
  {/if}
  <main class="main">{@render children?.()}</main>
</div>

<style>
  .root { display: flex; height: 100%; }
  .side {
    width: var(--sidebar-w); flex: none; display: flex; flex-direction: column; gap: var(--s-4);
    padding: var(--s-4) var(--s-3);
    background: var(--glass-fill); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
    border-right: 1px solid var(--glass-border); box-shadow: var(--glass-highlight);
  }
  .brand { display: flex; align-items: center; gap: 10px; padding: var(--s-2) var(--s-3) var(--s-3); font-weight: 650; font-size: var(--t-md); letter-spacing: -.01em; }
  .logo { width: 22px; height: 22px; border-radius: 7px; background: radial-gradient(circle at 35% 30%, #a9c1ff, #5b8cff 60%, #3b5fd0); box-shadow: 0 0 0 1px rgba(255,255,255,.18) inset, 0 2px 10px rgba(91,140,255,.4); }
  .items { display: flex; flex-direction: column; gap: 2px; flex: 1; }
  .item {
    display: flex; align-items: center; gap: 10px; padding: 8px 12px; border-radius: var(--r-sm);
    color: var(--text-2); font-weight: 500; transition: background var(--fast) var(--ease), color var(--fast) var(--ease);
  }
  .item:hover { background: rgba(255,255,255,.06); color: var(--text); text-decoration: none; }
  .item.on { background: var(--accent-soft); color: var(--accent); }
  .foot { display: flex; align-items: center; gap: var(--s-2); padding: 0 var(--s-2); color: var(--text-3); font-size: var(--t-xs); }
  .status { display: inline-flex; align-items: center; gap: 6px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--text-3); }
  .dot.ok { background: var(--good); box-shadow: 0 0 8px var(--good); }
  .dot.pulse { animation: pulse 2.4s var(--ease) infinite; }
  @keyframes pulse { 50% { opacity: .45; } }
  @media (prefers-reduced-motion: reduce) { .dot.pulse { animation: none; } }
  .dot.warn { background: var(--warn); box-shadow: 0 0 8px var(--warn); }
  .bk { padding: 0 var(--s-3); color: var(--text-3); font-size: var(--t-xs); margin-top: calc(-1 * var(--s-2)); }
  .recon {
    position: fixed; z-index: 60; top: 12px; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: var(--s-2);
    padding: 4px 6px 4px 12px; border-radius: var(--r-pill); font-size: var(--t-sm); color: var(--text);
    background: var(--glass-fill-strong); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight), var(--shadow-md);
    backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur);
  }
  .dot.bad { background: var(--bad); box-shadow: 0 0 8px var(--bad); }
  .ver { margin-left: auto; font-family: var(--mono); }
  .out { display: grid; place-items: center; width: 26px; height: 26px; border: 0; border-radius: var(--r-xs); background: none; color: var(--text-3); transition: background var(--fast), color var(--fast); }
  .out:hover { background: rgba(255,255,255,.08); color: var(--text); }
  .main { flex: 1; min-width: 0; height: 100%; overflow-y: auto; position: relative; }

  @media (max-width: 720px) {
    .root { flex-direction: column-reverse; }
    .side {
      position: fixed; z-index: 30; left: 8px; right: 8px; bottom: calc(8px + env(safe-area-inset-bottom));
      width: auto; height: var(--tab-h); flex-direction: row; align-items: center; padding: 6px;
      border: 1px solid var(--glass-border); border-radius: var(--r-lg); box-shadow: var(--glass-highlight), var(--shadow-md);
    }
    .brand, .foot .ver, .foot .status, .bk { display: none; }
    .items { flex-direction: row; justify-content: space-around; }
    .item { flex: 1; flex-direction: column; gap: 2px; padding: 5px 4px; font-size: 10.5px; align-items: center; justify-content: center; }
    .foot { padding: 0; }
    .main { padding-bottom: calc(var(--tab-h) + 24px + env(safe-area-inset-bottom)); }
  }
</style>
