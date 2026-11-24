<!-- LogRow: one match-log entry. Props: row {ts, prompt, hits, ms, loaded, client?, prompt_len?}. Click prompt to expand. -->
<script>
  import Chip from '../../ui/Chip.svelte';
  import Icon from '../../ui/Icon.svelte';
  import Button from '../../ui/Button.svelte';
  import { go } from '../../lib/router.svelte.js';
  import { ago, ms } from '../../lib/format.js';
  let { row } = $props();
  let open = $state(false);
  const clipped = $derived(row.prompt_len > [...row.prompt].length);
  // MCP skill_search rows are logged with the client prefixed "mcp:" (e.g. mcp:claude@mac).
  const mcp = $derived(row.client?.startsWith('mcp:') ?? false);
  const client = $derived(mcp ? row.client.slice(4) : row.client);
</script>

<div class="row">
  <span class="when">{ago(row.ts)}</span>
  <div class="main">
    <button type="button" class="prompt" class:clamp={!open} aria-expanded={open} onclick={() => (open = !open)}>{row.prompt}</button>
    <div class="hits">
      {#each row.hits as h}
        <button type="button" class="hit" onclick={() => go(`/skills/${h.name}`)}>
          <Chip tone={row.loaded.includes(h.name) ? 'good' : 'accent'} mono>
            {#if row.loaded.includes(h.name)}<Icon name="check" size={11} />{/if}{h.name} {h.score.toFixed(2)}
          </Chip>
        </button>
      {:else}
        <Chip>no match</Chip>
      {/each}
      {#if mcp}<Chip tone="warn" mono>mcp</Chip>{/if}
      {#if client}<Chip mono>{client}</Chip>{/if}
      {#if clipped}<span class="clip">clipped from {row.prompt_len} chars</span>{/if}
    </div>
  </div>
  <div class="side">
    <span class="ms">{ms(row.ms)}</span>
    <Button variant="plain" size="sm" onclick={() => go('/playground', { prompt: row.prompt })} aria-label="Replay this prompt in the Playground">Replay</Button>
  </div>
</div>

<style>
  .row { display: grid; grid-template-columns: 74px 1fr auto; gap: var(--s-4); padding: var(--s-3) var(--s-4); border-bottom: 1px solid var(--line); align-items: start; }
  .row:last-child { border-bottom: 0; }
  .when, .ms { color: var(--text-3); font-size: var(--t-sm); white-space: nowrap; padding-top: 1px; }
  .side { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; }
  .clip { color: var(--text-3); font-size: var(--t-xs); align-self: center; }
  .ms { font-family: var(--mono); font-size: var(--t-xs); }
  .main { min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .prompt { border: 0; background: none; padding: 0; text-align: left; color: var(--text); line-height: 1.45; white-space: pre-wrap; overflow-wrap: anywhere; }
  .clamp { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .hits { display: flex; gap: 6px; flex-wrap: wrap; }
  .hit { border: 0; background: none; padding: 0; }
  .hit:hover { filter: brightness(1.25); }
  @media (max-width: 600px) { .row { grid-template-columns: 1fr auto; } .when { grid-column: 1; } .side { grid-column: 2; grid-row: 1; flex-direction: row; align-items: center; } .main { grid-column: 1 / -1; } }
</style>
