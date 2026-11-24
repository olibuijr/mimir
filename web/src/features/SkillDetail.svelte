<!-- Skill detail (#/skills/<name>). Props: name. -->
<script>
  import { onMount } from 'svelte';
  import TopBar from '../layout/TopBar.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Chip from '../ui/Chip.svelte';
  import Icon from '../ui/Icon.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Empty from '../ui/Empty.svelte';
  import { getSkill } from '../lib/api.js';
  import { onEvent, debounce } from '../lib/live.svelte.js';
  import SpecTab from './skill/SpecTab.svelte';
  import { ago } from '../lib/format.js';
  import FilesTab from './skill/FilesTab.svelte';
  import ActivityTab from './skill/ActivityTab.svelte';
  import SimilarTab from './skill/SimilarTab.svelte';
  import VersionsTab from './skill/VersionsTab.svelte';
  import Sparkline from '../ui/Sparkline.svelte';
  import { route } from '../lib/router.svelte.js';

  let { name } = $props();
  let skill = $state(null);
  let error = $state('');
  let file = $state(route.query?.file || '');
  let tab = $state(route.query?.file ? 'files' : 'instructions');

  // Shareable link: #/skills/<name>?file=<path>. replaceState avoids a router re-parse.
  function pick(p) {
    file = p;
    try { history.replaceState(null, '', `#/skills/${encodeURIComponent(name)}?file=${encodeURIComponent(p)}`); } catch {}
  }
  // Relative links in the rendered SKILL.md open the Files tab instead of navigating.
  function onProse(e) {
    const a = e.target.closest?.('a');
    if (!a) return;
    const h = a.getAttribute('href') || '';
    if (!h || /^(https?:|mailto:|#|\/)/i.test(h)) return;
    e.preventDefault();
    let p = h.split('#')[0].split('?')[0];
    try { p = decodeURIComponent(p); } catch {}
    p = p.replace(/^(\.\/)+/, '');
    if (skill.files.some((f) => f.path === p)) { pick(p); tab = 'files'; }
  }
  const days = $derived(skill?.per_day || []);
  const sum = (k) => days.reduce((n, d) => n + d[k], 0);

  const tabs = $derived([
    { value: 'instructions', label: 'Instructions' },
    { value: 'files', label: `Files${skill ? ` (${skill.files.length})` : ''}` },
    { value: 'versions', label: 'Versions' },
    { value: 'activity', label: 'Activity' },
    { value: 'similar', label: 'Similar' },
    { value: 'spec', label: skill && !skill.spec.errors.length ? 'Spec ✓' : `Spec${skill ? ` (${skill.spec.errors.length})` : ''}` },
  ]);

  const load = () => getSkill(name).then((s) => { skill = s; error = ''; }).catch((e) => (error = e.message));
  onMount(load);
  // Live: refetch when this skill is matched, loaded or changed.
  const later = debounce(load, 300);
  $effect(() => onEvent((m) => {
    const mine = (m.type === 'match' && m.row.hits.some((h) => h.name === name))
      || (m.type === 'load' && m.skill === name)
      || (m.type === 'skill' && (m.name === name || m.action === 'reembed'))
      || m.type === 'resync' || m.type === 'log_cleared';
    if (mine) later();
  }));
</script>

<TopBar title={name} back="/" subtitle={skill ? `updated ${ago(skill.updated_at)}` : ''}>
  {#if skill}
    <Chip tone={skill.spec.errors.length ? 'warn' : 'good'}>{skill.spec.errors.length ? 'not spec compliant' : 'Agent Skills spec'}</Chip>
    <Chip mono>v{skill.version}</Chip>
  {/if}
</TopBar>

{#if error}
  <Empty icon="warning" title="Could not load skill" hint={error} />
{:else if !skill}
  <div class="center"><Spinner size={20} /></div>
{:else}
  <div class="page">
    <p class="desc">{skill.description}</p>

    <div class="secrets">
      <span class="lbl"><Icon name="key" size={13} /> Secrets</span>
      {#if skill.secret_keys.length}
        {#each skill.secret_keys as k (k)}<Chip mono tone="accent">{k}</Chip>{/each}
        <span class="hid">value hidden</span>
      {:else}
        <span class="hid">none</span>
      {/if}
    </div>

    {#if days.length}
      <div class="trend">
        <span class="lbl">30 d</span>
        <span class="tr"><Sparkline values={days.map((d) => d.matches)} width={120} height={28} /><span class="n">{sum('matches')} matches</span></span>
        <span class="tr"><Sparkline values={days.map((d) => d.loads)} width={120} height={28} color="var(--good)" /><span class="n">{sum('loads')} loads</span></span>
      </div>
    {/if}

    <div><Segmented options={tabs} bind:value={tab} label="Skill sections" /></div>

    {#if tab === 'instructions'}
      {#if skill.html?.trim()}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <article class="prose body" onclick={onProse}>{@html skill.html}</article>
      {:else}
        <Empty icon="file" title="Empty SKILL.md" hint="This skill has no instructions body." />
      {/if}
    {:else if tab === 'files'}
      <FilesTab {skill} selected={file} onselect={pick} />
    {:else if tab === 'versions'}
      <VersionsTab {skill} />
    {:else if tab === 'activity'}
      <ActivityTab {skill} />
    {:else if tab === 'similar'}
      <SimilarTab {skill} />
    {:else}
      <SpecTab {skill} />
    {/if}
  </div>
{/if}

<style>
  .page { padding: var(--s-5); display: flex; flex-direction: column; gap: var(--s-4); max-width: 1100px; }
  .center { display: grid; place-items: center; padding: var(--s-7); }
  .desc { color: var(--text-2); line-height: 1.55; max-width: 80ch; }
  .secrets { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .lbl { display: inline-flex; align-items: center; gap: 5px; color: var(--text-3); font-size: var(--t-sm); margin-right: 4px; }
  .hid { color: var(--text-3); font-size: var(--t-xs); font-style: italic; }
  .trend { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-4); }
  .tr { display: inline-flex; align-items: center; gap: 8px; }
  .n { color: var(--text-2); font-size: var(--t-sm); }
  .body { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-4) var(--s-5); overflow-wrap: anywhere; }
  @media (max-width: 720px) { .page { padding: var(--s-4); } .body { padding: var(--s-3) var(--s-4); } }
</style>
