<!-- One library row. Props: skill (from /api/skills). -->
<script>
  import ListRow from '../../ui/ListRow.svelte';
  import Chip from '../../ui/Chip.svelte';
  import Icon from '../../ui/Icon.svelte';
  import Tooltip from '../../ui/Tooltip.svelte';
  import { ago } from '../../lib/format.js';
  let { skill: s } = $props();
</script>

<ListRow href={`/skills/${s.name}`}>
  <div class="top"><span class="nm">{s.name}</span><span class="last">{s.last_matched ? ago(s.last_matched) : 'never matched'}</span></div>
  <div class="desc">{s.description}</div>
  <div class="chips">
    <Chip mono>v{s.version}</Chip>
    {#if s.compliant === false}<Tooltip text="Breaks the Agent Skills spec; open the skill for details" focusable><Chip tone="warn">not spec compliant</Chip></Tooltip>{/if}
    <Chip>{s.files} {s.files === 1 ? 'file' : 'files'}</Chip>
    {#if s.secrets > 0}<Chip tone="accent"><Icon name="key" size={11} /> {s.secrets}</Chip>{/if}
    <Chip tone={s.matches_7d > 0 ? 'good' : 'neutral'}>{s.matches_7d} {s.matches_7d === 1 ? "match" : "matches"} 7d</Chip>
    <Chip>{s.loads_7d} {s.loads_7d === 1 ? "load" : "loads"} 7d</Chip>
    {#if s.stale}<Tooltip text="No match in the last 30 days" focusable><Chip>unused 30 d</Chip></Tooltip>{/if}
    {#if s.warnings?.length}
      <Tooltip text={s.warnings.join(' / ')} focusable><Chip tone="warn"><Icon name="warning" size={11} /> {s.warnings.length}</Chip></Tooltip>
    {/if}
  </div>
</ListRow>

<style>
  .top { display: flex; align-items: baseline; gap: var(--s-3); }
  .nm { font-weight: 600; font-size: var(--t-md); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .last { margin-left: auto; color: var(--text-3); font-size: var(--t-xs); white-space: nowrap; }
  .desc { color: var(--text-2); font-size: var(--t-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 2px 0 6px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
</style>
