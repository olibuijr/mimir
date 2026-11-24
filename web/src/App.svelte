<script>
  import { route } from './lib/router.svelte.js';
  import { auth } from './lib/auth.svelte.js';
  import { cfg, loadConfig } from './lib/config.svelte.js';
  import { connect, disconnect, onEvent } from './lib/live.svelte.js';
  import RootLayout from './layout/RootLayout.svelte';
  import Toast from './ui/Toast.svelte';
  import Library from './features/Library.svelte';
  import SkillDetail from './features/SkillDetail.svelte';
  import Playground from './features/Playground.svelte';
  import Activity from './features/Activity.svelte';
  import LoginSheet from './features/LoginSheet.svelte';

  // Load config on boot and again whenever a login completes (auth.needed flips back to false).
  $effect(() => {
    if (auth.needed) { disconnect(); return; }
    loadConfig().then(() => connect());
  });
  // Skill count, model status, backup and re-embed state follow skill changes live.
  $effect(() => onEvent((m) => {
    if (m.type === 'skill' || m.type === 'resync') loadConfig();
  }));
</script>

<RootLayout config={cfg.data}>
  {#if route.name === 'skill'}
    {#key route.params.name}<SkillDetail name={route.params.name} />{/key}
  {:else if route.name === 'playground'}
    <Playground />
  {:else if route.name === 'activity'}
    <Activity />
  {:else}
    <Library />
  {/if}
</RootLayout>

{#if auth.needed}<LoginSheet />{/if}
<Toast />
