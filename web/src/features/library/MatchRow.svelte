<!-- One ranked search result. Props: hit {name, description, score, cosine}, min (threshold or null). -->
<script>
  import ListRow from '../../ui/ListRow.svelte';
  import ScoreBar from '../../ui/ScoreBar.svelte';
  import { score } from '../../lib/format.js';
  let { hit, min = null } = $props();
  const below = $derived(min != null && hit.score < min);
</script>

<ListRow href={`/skills/${hit.name}`}>
  <div class="w" class:below>
    <div class="top"><span class="nm">{hit.name}</span><span class="cos">cos {score(hit.cosine)}</span></div>
    <div class="desc">{hit.description}</div>
    <ScoreBar value={hit.score} threshold={min} />
  </div>
</ListRow>

<style>
  .w { display: flex; flex-direction: column; gap: 4px; }
  .below { opacity: .5; }
  .top { display: flex; align-items: baseline; gap: var(--s-3); }
  .nm { font-weight: 600; font-size: var(--t-md); }
  .cos { margin-left: auto; font-family: var(--mono); font-size: var(--t-xs); color: var(--text-3); }
  .desc { color: var(--text-2); font-size: var(--t-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
