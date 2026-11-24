<!-- Versions tab: history list, rendered old version and line diff against the current body. Props: skill. Read-only. -->
<script>
  import Chip from '../../ui/Chip.svelte';
  import Spinner from '../../ui/Spinner.svelte';
  import Empty from '../../ui/Empty.svelte';
  import Segmented from '../../ui/Segmented.svelte';
  import { api } from '../../lib/api.js';
  import { ago, bytes } from '../../lib/format.js';
  import { diffLines } from './diff.js';

  let { skill } = $props();
  let list = $state(null);
  let error = $state('');
  let sel = $state(null);
  let detail = $state(null);
  let derr = $state('');
  let view = $state('diff');
  let seq = 0;

  const base = $derived(`/api/skills/${encodeURIComponent(skill.name)}/history`);

  // Refetch whenever the skill changes (the page reloads it on 'skill' events).
  $effect(() => {
    skill.version; skill.updated_at;
    api(base).then((l) => { list = l; error = ''; }).catch((e) => (error = e.message));
  });

  $effect(() => {
    const v = sel;
    detail = null; derr = '';
    if (v == null) return;
    const my = ++seq;
    api(`${base}/${v}`).then((d) => { if (my === seq) detail = d; }).catch((e) => { if (my === seq) derr = e.message; });
  });

  const chosen = $derived(list?.find((r) => r.version === sel));
  const diff = $derived(detail ? diffLines(detail.body, skill.body ?? '') : null);
  const changed = (i) => list[i + 1] && list[i].description !== list[i + 1].description;
</script>

{#if error}
  <Empty icon="warning" title="Could not load history" hint={error} />
{:else if !list}
  <div class="c"><Spinner size={18} /></div>
{:else if !list.length}
  <Empty icon="file" title="No history" />
{:else}
  <div class="fx">
    <div class="list" role="listbox" aria-label="Versions">
      {#each list as r, i (r.version)}
        <button type="button" class="r" class:sel={sel === r.version} role="option" aria-selected={sel === r.version}
          onclick={() => (sel = r.version)}>
          <span class="v">v{r.version}</span>
          {#if r.current}<Chip tone="good">current</Chip>{/if}
          {#if changed(i)}<Chip tone="accent">description changed</Chip>{/if}
          <span class="sp"></span>
          <span class="m">{bytes(r.bytes)}</span>
          <span class="m">{ago(r.updated_at)}</span>
        </button>
      {/each}
    </div>

    <div class="view">
      {#if sel == null}
        <Empty icon="file" title="Select a version" hint="See how it rendered and what changed." />
      {:else if derr}
        <Empty icon="warning" title="Could not load version" hint={derr} />
      {:else if !detail}
        <div class="c"><Spinner size={18} /></div>
      {:else}
        <div class="hd">
          <span class="t">v{detail.version} · {ago(detail.updated_at)}</span>
          {#if chosen?.current}<Chip tone="good">current</Chip>{/if}
          <span class="sp"></span>
          <Segmented options={[{ value: 'diff', label: 'Diff vs current' }, { value: 'render', label: 'Rendered' }]} bind:value={view} label="Version view" />
        </div>
        <p class="desc">{detail.description}</p>
        {#if view === 'render'}
          {#if detail.html?.trim()}<article class="prose body">{@html detail.html}</article>{:else}<Empty icon="file" title="Empty body" />{/if}
        {:else if chosen?.current}
          <Empty icon="file" title="This is the current version" />
        {:else if diff === null}
          <Empty icon="warning" title="Too large to diff" hint="More than 2,000 lines on a side." />
        {:else}
          <pre class="diff">{#each diff as l}<span class="ln {l.t === '+' ? 'add' : l.t === '-' ? 'del' : ''}">{l.t} {l.s}
</span>{/each}</pre>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .c { display: grid; place-items: center; padding: var(--s-6); }
  .fx { display: grid; grid-template-columns: 300px minmax(0, 1fr); gap: var(--s-4); align-items: start; }
  .list { display: flex; flex-direction: column; gap: 2px; background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-2); min-width: 0; }
  .r { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 7px 8px; border: 0; border-radius: var(--r-xs); background: none; color: var(--text-2); text-align: left; transition: background var(--fast); }
  .r:hover { background: rgba(255,255,255,.06); color: var(--text); }
  .r.sel { background: var(--accent-soft); color: var(--accent); }
  .v { font-family: var(--mono); font-weight: 600; }
  .sp { flex: 1; }
  .m { color: var(--text-3); font-size: var(--t-xs); }
  .view { display: flex; flex-direction: column; gap: var(--s-3); min-width: 0; }
  .hd { display: flex; align-items: center; gap: var(--s-2); flex-wrap: wrap; }
  .t { font-weight: 600; }
  .desc { color: var(--text-2); line-height: 1.5; }
  .body { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-4) var(--s-5); overflow-wrap: anywhere; }
  .diff { margin: 0; padding: var(--s-2) 0; background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); font-family: var(--mono); font-size: var(--t-sm); line-height: 1.5; overflow: auto; max-height: 600px; }
  .ln { display: block; padding: 0 var(--s-3); white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text-2); }
  .add { background: var(--good-soft); color: var(--good); }
  .del { background: var(--bad-soft); color: var(--bad); }
  @media (max-width: 720px) { .fx { grid-template-columns: 1fr; } }
</style>
