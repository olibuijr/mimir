<!-- Similar tab: nearest skills by embedding cosine. Props: skill. -->
<script>
  import ListGroup from '../../ui/ListGroup.svelte';
  import ListRow from '../../ui/ListRow.svelte';
  import ScoreBar from '../../ui/ScoreBar.svelte';
  import Chip from '../../ui/Chip.svelte';
  import Empty from '../../ui/Empty.svelte';
  let { skill } = $props();
</script>

{#if !skill.similar?.length}
  <Empty title="No similar skills" hint="Nothing else to compare against." />
{:else}
  <ListGroup>
    {#each skill.similar as s (s.name)}
      <ListRow href={`/skills/${s.name}`}>
        <div class="top"><span class="nm">{s.name}</span>{#if s.co_hits > 0}<Chip tone="warn">{s.co_hits} co-hits</Chip>{:else}<Chip>0 co-hits</Chip>{/if}</div>
        <ScoreBar value={s.cosine} width="260px" />
      </ListRow>
    {/each}
  </ListGroup>
{/if}

<style>
  .top { display: flex; align-items: center; gap: var(--s-3); margin-bottom: 6px; }
  .nm { font-weight: 600; }
</style>
