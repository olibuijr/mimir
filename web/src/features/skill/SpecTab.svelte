<!-- Spec tab: how the skill measures up to the Agent Skills specification. Props: skill (from /api/skills/{name}). -->
<script>
  import Section from '../../ui/Section.svelte';
  import Chip from '../../ui/Chip.svelte';
  import Icon from '../../ui/Icon.svelte';
  import { bytes } from '../../lib/format.js';
  let { skill } = $props();

  const fm = $derived(skill.frontmatter || {});
  const errors = $derived(skill.spec?.errors || []);
  const warnings = $derived(skill.spec?.warnings || []);
  const descLen = $derived((fm.description || '').trim().length);
  const lines = $derived((skill.body || '').split('\n').length);
  const meta = $derived(Object.entries(fm.metadata || {}));

  // The spec's directory layout: SKILL.md, then scripts/, references/, assets/ and anything else.
  const DIRS = ['scripts', 'references', 'assets'];
  const groups = $derived.by(() => {
    const g = { scripts: [], references: [], assets: [], other: [] };
    for (const f of skill.files || []) {
      const top = f.path.split('/')[0];
      (DIRS.includes(top) && f.path.includes('/') ? g[top] : g.other).push(f);
    }
    return g;
  });

  const fields = $derived([
    { k: 'name', v: fm.name, need: true, note: 'must match the directory name' },
    { k: 'description', v: fm.description ? `${descLen} / 1024 characters` : null, need: true, note: 'what it does and when to use it' },
    { k: 'license', v: fm.license },
    { k: 'compatibility', v: fm.compatibility },
    { k: 'allowed-tools', v: fm['allowed-tools'] },
  ]);
</script>

<div class="wrap">
  <div class="verdict" class:ok={!errors.length}>
    <Icon name={errors.length ? 'warning' : 'check'} size={18} />
    <div>
      <strong>{errors.length ? `${errors.length} spec violation${errors.length === 1 ? '' : 's'}` : 'Follows the Agent Skills specification'}</strong>
      <span>Checked against <a href="https://agentskills.io/specification" target="_blank" rel="noreferrer">agentskills.io/specification</a>{warnings.length ? `, ${warnings.length} recommendation${warnings.length === 1 ? '' : 's'} not met` : ''}.</span>
    </div>
  </div>

  {#if errors.length}
    <Section title="Errors" hint="Mímir refuses new versions until these are fixed">
      <ul class="msgs">{#each errors as e}<li class="err">{e}</li>{/each}</ul>
    </Section>
  {/if}
  {#if warnings.length}
    <Section title="Recommendations">
      <ul class="msgs">{#each warnings as w}<li>{w}</li>{/each}</ul>
    </Section>
  {/if}

  <Section title="Frontmatter">
    <div class="grid">
      {#each fields as f}
        <span class="k mono">{f.k}{#if f.need}<span class="req">required</span>{/if}</span>
        <span class="v">{#if f.v}{f.v}{:else}<span class="none">—</span>{/if}{#if f.note}<span class="note">{f.note}</span>{/if}</span>
      {/each}
      <span class="k mono">metadata</span>
      <span class="v">
        {#if meta.length}
          <span class="meta">{#each meta as [k, v]}<Chip mono>{k}: {v}</Chip>{/each}</span>
        {:else}<span class="none">—</span>{/if}
      </span>
    </div>
  </Section>

  <Section title="Layout" hint={`SKILL.md ${lines} lines (recommended under 500)`}>
    <div class="tree mono">
      <div class="row"><Icon name="folder" size={14} />{skill.name}/</div>
      <div class="row in"><Icon name="file" size={14} />SKILL.md</div>
      {#each DIRS as d}
        {#if groups[d].length}
          <div class="row in"><Icon name="folder" size={14} />{d}/ <span class="cnt">{groups[d].length} file{groups[d].length === 1 ? '' : 's'}, {bytes(groups[d].reduce((a, f) => a + f.size, 0))}</span></div>
        {/if}
      {/each}
      {#each groups.other as f}
        <div class="row in"><Icon name="file" size={14} />{f.path} <span class="cnt">{bytes(f.size)}</span></div>
      {/each}
    </div>
  </Section>
</div>

<style>
  .wrap { display: flex; flex-direction: column; gap: var(--s-4); }
  .verdict { display: flex; gap: var(--s-3); align-items: flex-start; padding: var(--s-3) var(--s-4); border-radius: var(--r-md); background: var(--warn-soft); color: var(--warn); }
  .verdict.ok { background: var(--good-soft); color: var(--good); }
  .verdict div { display: flex; flex-direction: column; gap: 2px; }
  .verdict span { color: var(--text-2); font-size: var(--t-sm); }
  .verdict a { color: var(--accent); }
  .msgs { margin: 0; padding-left: 1.2em; display: flex; flex-direction: column; gap: 6px; color: var(--text-2); font-size: var(--t-sm); line-height: 1.5; }
  .msgs .err { color: var(--text); }
  .grid { display: grid; grid-template-columns: max-content 1fr; gap: 10px var(--s-4); align-items: baseline; font-size: var(--t-sm); }
  .k { color: var(--text-3); display: inline-flex; gap: 6px; align-items: baseline; }
  .req { font-family: var(--font); font-size: var(--t-xs); color: var(--accent); }
  .v { color: var(--text); overflow-wrap: anywhere; display: flex; flex-wrap: wrap; gap: 6px 10px; align-items: baseline; }
  .note { color: var(--text-3); font-size: var(--t-xs); }
  .none { color: var(--text-3); }
  .meta { display: flex; flex-wrap: wrap; gap: 6px; min-width: 0; }
  .meta :global(.chip) { white-space: normal; overflow-wrap: anywhere; height: auto; line-height: 1.45; padding-top: 2px; padding-bottom: 2px; }
  .tree { display: flex; flex-direction: column; gap: 6px; font-size: var(--t-sm); color: var(--text-2); }
  .row { display: flex; align-items: center; gap: 8px; }
  .row.in { padding-left: 22px; }
  .cnt { color: var(--text-3); font-family: var(--font); font-size: var(--t-xs); }
  .mono { font-family: var(--mono); }
  @media (max-width: 720px) { .grid { grid-template-columns: 1fr; gap: 4px; } .k { margin-top: 8px; } }
</style>
