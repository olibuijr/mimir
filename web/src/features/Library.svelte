<!-- Library (#/): skill list with sort, or ranked match results for a typed prompt. -->
<script>
  import { onMount } from 'svelte';
  import TopBar from '../layout/TopBar.svelte';
  import SearchField from '../ui/SearchField.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import StatTile from '../ui/StatTile.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Empty from '../ui/Empty.svelte';
  import ListGroup from '../ui/ListGroup.svelte';
  import { listSkills, getStats, match } from '../lib/api.js';
  import { cfg } from '../lib/config.svelte.js';
  import { onEvent, debounce } from '../lib/live.svelte.js';
  import { pct, ms } from '../lib/format.js';
  import SkillRow from './library/SkillRow.svelte';
  import MatchRow from './library/MatchRow.svelte';

  let skills = $state(null);
  let error = $state('');
  let stats = $state(null);
  let sort = $state('recent');
  let q = $state('');
  let res = $state(null);
  let searching = $state(false);
  let serr = $state('');
  let timer;
  let seq = 0;

  const SORTS = [{ value: 'recent', label: 'Recent' }, { value: 'name', label: 'Name' }, { value: 'matched', label: 'Most matched' }];
  const sorted = $derived.by(() => {
    const a = [...(skills || [])];
    if (sort === 'name') a.sort((x, y) => x.name.localeCompare(y.name));
    else if (sort === 'matched') a.sort((x, y) => y.matches_30d - x.matches_30d || y.matches_7d - x.matches_7d || x.name.localeCompare(y.name));
    else a.sort((x, y) => (y.last_matched ?? -1) - (x.last_matched ?? -1) || x.name.localeCompare(y.name));
    return a;
  });
  const config = $derived(cfg.data);
  const min = $derived(config?.min ?? null);

  const loadList = () => listSkills().then((s) => { skills = s; error = ''; }).catch((e) => (error = e.message));
  const loadStats = () => getStats(Math.floor(Date.now() / 1000) - 86400).then((s) => (stats = s)).catch(() => {});
  onMount(() => { loadList(); loadStats(); });

  // Live: matches and loads patch the rows in place; only skill/resync events refetch the list.
  const listLater = debounce(loadList, 400);
  const statsLater = debounce(loadStats, 2000);
  $effect(() => onEvent((m) => {
    if (m.type === 'match' && skills && m.row?.client?.startsWith('mcp:')) {
      statsLater();
    } else if (m.type === 'match' && skills) {
      const hit = new Set((m.row?.hits || []).map((h) => h.name));
      skills = skills.map((s) => hit.has(s.name)
        ? { ...s, matches_7d: s.matches_7d + 1, matches_30d: s.matches_30d + 1, last_matched: m.row.ts ?? s.last_matched, stale: false } : s);
      statsLater();
    } else if (m.type === 'load' && skills) {
      skills = skills.map((s) => (s.name === m.skill ? { ...s, loads_7d: s.loads_7d + 1 } : s));
      statsLater();
    } else if (m.type === 'skill' || m.type === 'resync') {
      listLater(); statsLater();
    } else if (m.type === 'log_cleared') {
      listLater(); statsLater();
    }
  }));

  function onInput(v) {
    clearTimeout(timer);
    const t = v.trim();
    if (t.length < 3) { seq++; res = null; searching = false; serr = ''; return; }
    timer = setTimeout(() => run(t), 250);
  }
  async function run(t) {
    const my = ++seq;
    searching = true; serr = '';
    try {
      const r = await match({ prompt: t, k: 12, min: -10 });
      if (my === seq) res = r;
    } catch (e) { if (my === seq) { serr = e.message; res = null; } }
    if (my === seq) searching = false;
  }
</script>

<TopBar title="Library" subtitle={skills ? `${skills.length} skills` : ''}>
  <div class="search"><SearchField bind:value={q} placeholder="Try a prompt to see what Claude would get" label="Search skills" oninput={onInput} onsubmit={(v) => { clearTimeout(timer); if (v.trim().length >= 3) run(v.trim()); }} /></div>
</TopBar>

<div class="page">
  <div class="tiles">
    <StatTile label="Skills" value={skills ? skills.length : (config?.skills ?? '-')} />
    <StatTile label="Matches 24 h" value={stats ? stats.matches : '-'} sub={stats?.mcp_searches ? `+${stats.mcp_searches} mcp` : ''} />
    <StatTile label="Hit rate" value={stats ? pct(stats.hit_rate) : '-'} tone={stats && stats.matches > 0 && stats.hit_rate < 0.5 ? 'warn' : 'good'} />
    <StatTile label="Latency p50" value={stats ? ms(stats.p50_ms) : '-'} sub={stats ? `p95 ${ms(stats.p95_ms)}` : ''} />
  </div>

  {#if q.trim().length >= 3}
    <div class="bar"><h3>What Claude would get</h3>{#if min != null}<span class="hint">threshold {min.toFixed(2)}, dimmed results fall below it</span>{/if}</div>
    {#if serr}
      <Empty icon="warning" title="Match failed" hint={serr} />
    {:else if !res}
      <div class="center"><Spinner size={20} /></div>
    {:else if res.hits.length === 0}
      <Empty title="No candidates" hint="Nothing matched this prompt at all." />
    {:else}
      <div class="fade" class:busy={searching}>
        <ListGroup>
          {#each res.hits as h (h.name)}<MatchRow hit={h} {min} />{/each}
        </ListGroup>
        <p class="foot">{ms(res.ms)}</p>
      </div>
    {/if}
  {:else}
    <div class="bar"><h3>All skills</h3><div class="sg"><Segmented options={SORTS} bind:value={sort} label="Sort" /></div></div>
    {#if error}
      <Empty icon="warning" title="Could not load skills" hint={error} />
    {:else if !skills}
      <div class="center"><Spinner size={20} /></div>
    {:else if skills.length === 0}
      <Empty icon="library" title="No skills yet" hint="Push a skill to Mimir and it will show up here." />
    {:else}
      <ListGroup>
        {#each sorted as s (s.name)}<SkillRow skill={s} />{/each}
      </ListGroup>
    {/if}
  {/if}
</div>

<style>
  .search { width: min(460px, 56vw); }
  .page { padding: var(--s-5); display: flex; flex-direction: column; gap: var(--s-4); }
  .tiles { display: flex; flex-wrap: nowrap; gap: var(--s-2); }
  .bar { display: flex; align-items: center; gap: var(--s-3); flex-wrap: wrap; }
  .bar h3 { font-size: var(--t-sm); color: var(--text-2); text-transform: uppercase; letter-spacing: .06em; }
  .hint { color: var(--text-3); font-size: var(--t-sm); }
  .sg { margin-left: auto; }
  .center { display: grid; place-items: center; padding: var(--s-7); }
  .fade { transition: opacity var(--fast); }
  .fade.busy { opacity: .6; }
  .foot { color: var(--text-3); font-size: var(--t-xs); margin-top: var(--s-2); text-align: right; font-family: var(--mono); }
  @media (max-width: 720px) { .page { padding: var(--s-4); } .search { width: 100%; } }
</style>
