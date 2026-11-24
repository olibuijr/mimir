<!--
  FileTree: collapsible folder tree from flat paths.
  Props: files [{path, size?}] (or strings), selected (path), onselect(path).
-->
<script>
  import Icon from './Icon.svelte';
  import { bytes } from '../lib/format.js';
  let { files = [], selected = '', onselect } = $props();
  let closed = $state({});

  const rows = $derived.by(() => {
    const root = { kids: {} };
    for (const f of files) {
      const p = typeof f === 'string' ? f : f.path;
      const parts = p.split('/');
      let n = root;
      parts.forEach((s, i) => {
        n = n.kids[s] ??= { name: s, path: parts.slice(0, i + 1).join('/'), kids: {}, file: false };
        if (i === parts.length - 1) { n.file = true; n.size = typeof f === 'string' ? null : f.size; }
      });
    }
    const out = [];
    const walk = (n, d) => {
      const ks = Object.values(n.kids).sort((a, b) => (Object.keys(b.kids).length > 0) - (Object.keys(a.kids).length > 0) || a.name.localeCompare(b.name));
      for (const k of ks) {
        const dir = Object.keys(k.kids).length > 0;
        out.push({ ...k, dir, d });
        if (dir && !closed[k.path]) walk(k, d + 1);
      }
    };
    walk(root, 0);
    return out;
  });

  let focused = $state('');
  let tree;
  // Roving tabindex: the focused row, else the selected one, else the first.
  const stop = $derived(rows.some((r) => r.path === focused) ? focused : rows.some((r) => r.path === selected) ? selected : rows[0]?.path);

  function focusRow(i) {
    const r = rows[Math.max(0, Math.min(rows.length - 1, i))];
    if (!r) return;
    focused = r.path;
    tree.querySelectorAll('[role=treeitem]')[rows.indexOf(r)]?.focus();
  }
  function key(e, i) {
    const r = rows[i];
    if (e.key === 'ArrowDown') focusRow(i + 1);
    else if (e.key === 'ArrowUp') focusRow(i - 1);
    else if (e.key === 'Home') focusRow(0);
    else if (e.key === 'End') focusRow(rows.length - 1);
    else if (e.key === 'ArrowRight') {
      if (r.dir && closed[r.path]) closed[r.path] = false;
      else if (r.dir) focusRow(i + 1);
      else return;
    } else if (e.key === 'ArrowLeft') {
      if (r.dir && !closed[r.path]) closed[r.path] = true;
      else { const j = rows.findLastIndex((x, k) => k < i && x.dir && x.d < r.d); if (j >= 0) focusRow(j); else return; }
    } else return;
    e.preventDefault();
  }
</script>

<div class="ft" role="tree" aria-label="Files" bind:this={tree}>
  {#each rows as r, i (r.path)}
    <button type="button" class="r" class:sel={!r.dir && r.path === selected} style="padding-left:{8 + r.d * 16}px"
      role="treeitem" tabindex={r.path === stop ? 0 : -1} onkeydown={(e) => key(e, i)} onfocus={() => (focused = r.path)}
      aria-level={r.d + 1} aria-selected={r.path === selected} aria-expanded={r.dir ? !closed[r.path] : undefined}
      onclick={() => (r.dir ? (closed[r.path] = !closed[r.path]) : onselect?.(r.path))}>
      {#if r.dir}<span class="car" class:open={!closed[r.path]}><Icon name="chevron" size={12} /></span><Icon name="folder" size={15} />
      {:else}<span class="sp"></span><Icon name="file" size={15} />{/if}
      <span class="nm">{r.name}</span>
      {#if !r.dir && r.size != null}<span class="sz">{bytes(r.size)}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .ft { display: flex; flex-direction: column; gap: 1px; }
  .r { display: flex; align-items: center; gap: 6px; height: 28px; padding-right: 8px; border: 0; border-radius: var(--r-xs); background: none; color: var(--text-2); text-align: left; transition: background var(--fast); }
  .r:hover { background: rgba(255,255,255,.06); color: var(--text); }
  .r.sel { background: var(--accent-soft); color: var(--accent); }
  .car { display: inline-flex; width: 12px; transition: transform var(--fast) var(--ease); color: var(--text-3); }
  .car.open { transform: rotate(90deg); }
  .sp { width: 12px; }
  .nm { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sz { color: var(--text-3); font-size: var(--t-xs); font-family: var(--mono); }
</style>
