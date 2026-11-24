<!-- Activity tab: recent match log rows that hit this skill. Props: skill. -->
<script>
  import ListGroup from '../../ui/ListGroup.svelte';
  import ListRow from '../../ui/ListRow.svelte';
  import ScoreBar from '../../ui/ScoreBar.svelte';
  import Chip from '../../ui/Chip.svelte';
  import Empty from '../../ui/Empty.svelte';
  import { ago, ms } from '../../lib/format.js';
  let { skill } = $props();
  const rows = $derived((skill.recent || []).map((r) => ({
    ...r,
    mine: r.hits.find((h) => h.name === skill.name),
    loaded: (r.loaded || []).includes(skill.name),
  })));
</script>

{#if rows.length === 0}
  <Empty icon="activity" title="No recent activity" hint="This skill has not been suggested for any prompt yet." />
{:else}
  <ListGroup>
    {#each rows as r (r.id)}
      <ListRow>
        <div class="top">
          <span class="when">{ago(r.ts)}</span><span class="ms">{ms(r.ms)}</span>
          {#if r.loaded}<Chip tone="good">loaded</Chip>{/if}
        </div>
        <div class="prompt">{r.prompt}</div>
        {#if r.mine}<ScoreBar value={r.mine.score} width="220px" />{/if}
      </ListRow>
    {/each}
  </ListGroup>
{/if}

<style>
  .top { display: flex; align-items: center; gap: var(--s-3); margin-bottom: 2px; }
  .when { color: var(--text-3); font-size: var(--t-xs); }
  .ms { color: var(--text-3); font-size: var(--t-xs); font-family: var(--mono); margin-right: auto; }
  .prompt { color: var(--text); font-size: var(--t-sm); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; margin-bottom: 6px; overflow-wrap: anywhere; }
</style>
