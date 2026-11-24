<!-- Toast host: render once (App.svelte). Trigger with toast(message, tone) from lib/toast.svelte.js. -->
<script>
  import { toasts } from '../lib/toast.svelte.js';
  import Icon from './Icon.svelte';
</script>

<div class="host" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.tone}">
      {#if t.tone === 'good'}<Icon name="check" size={15} />{:else if t.tone === 'warn'}<Icon name="warning" size={15} />{/if}
      <span>{t.message}</span>
    </div>
  {/each}
</div>

<style>
  .host { position: fixed; z-index: 100; left: 50%; bottom: 24px; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 8px; pointer-events: none; }
  .toast { display: flex; align-items: center; gap: 8px; padding: 9px 16px; border-radius: var(--r-pill); font-weight: 500; background: var(--glass-fill-strong); border: 1px solid var(--glass-border); box-shadow: var(--glass-highlight), var(--shadow-md); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); animation: up var(--med) var(--ease); }
  .good { color: var(--good); } .warn { color: var(--warn); }
  @keyframes up { from { opacity: 0; transform: translateY(8px); } }
  @media (max-width: 720px) { .host { bottom: calc(var(--tab-h) + 24px); } }
</style>
