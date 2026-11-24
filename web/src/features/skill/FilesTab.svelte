<!-- Files tab: tree left, content right. Props: skill. Text fetched lazily from fileUrl(). -->
<script>
  import FileTree from '../../ui/FileTree.svelte';
  import CodeView from '../../ui/CodeView.svelte';
  import Spinner from '../../ui/Spinner.svelte';
  import Empty from '../../ui/Empty.svelte';
  import Chip from '../../ui/Chip.svelte';
  import { fileUrl } from '../../lib/api.js';
  import { bytes } from '../../lib/format.js';

  let { skill, selected = '', onselect } = $props();
  let state = $state({ status: 'idle', text: '', error: '' });
  let seq = 0;
  const cur = $derived(skill.files.find((f) => f.path === selected));

  // Default selection when none (or an unknown one) is given.
  $effect(() => {
    if (skill.files.length && !skill.files.some((f) => f.path === selected)) {
      onselect?.((skill.files.find((f) => f.path === 'SKILL.md') || skill.files[0]).path);
    }
  });

  $effect(() => {
    const path = selected;
    if (!path) return;
    const my = ++seq;
    state = { status: 'loading', text: '', error: '' };
    (async () => {
      try {
        const r = await fetch(fileUrl(skill.name, path), { credentials: 'same-origin' });
        if (!r.ok) throw new Error(r.status === 404 ? 'File not found' : `HTTP ${r.status}`);
        const ct = r.headers.get('content-type') || '';
        if (!ct.startsWith('text/')) { if (my === seq) state = { status: 'binary', text: '', error: '' }; return; }
        const text = await r.text();
        if (my === seq) state = { status: 'ok', text, error: '' };
      } catch (e) { if (my === seq) state = { status: 'error', text: '', error: e.message }; }
    })();
  });
</script>

{#if skill.files.length === 0}
  <Empty icon="folder" title="No files" />
{:else}
  <div class="fx">
    <aside><FileTree files={skill.files} {selected} {onselect} /></aside>
    <div class="view">
      {#if cur}<div class="meta"><span class="p">{cur.path}</span><Chip mono>{cur.mode.toString(8)}</Chip><Chip>{bytes(cur.size)}</Chip></div>{/if}
      {#if state.status === 'loading'}
        <div class="c"><Spinner size={18} /></div>
      {:else if state.status === 'error'}
        <Empty icon="warning" title="Could not load file" hint={state.error} />
      {:else if state.status === 'binary'}
        <Empty icon="file" title="binary" hint={cur ? bytes(cur.size) : ''} />
      {:else if state.status === 'ok'}
        {#if state.text === ''}<Empty icon="file" title="Empty file" />{:else}<CodeView code={state.text} maxHeight="600px" />{/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .fx { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: var(--s-4); align-items: start; }
  aside { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-2); min-width: 0; }
  .view { display: flex; flex-direction: column; gap: var(--s-2); min-width: 0; }
  .meta { display: flex; align-items: center; gap: 6px; }
  .p { font-family: var(--mono); font-size: var(--t-sm); color: var(--text-2); margin-right: auto; overflow: hidden; text-overflow: ellipsis; }
  .c { display: grid; place-items: center; padding: var(--s-6); }
  @media (max-width: 720px) { .fx { grid-template-columns: 1fr; } }
</style>
