<!-- Activity: usage stats + searchable match log. -->
<script>
  import TopBar from '../layout/TopBar.svelte';
  import Section from '../ui/Section.svelte';
  import StatTile from '../ui/StatTile.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import SearchField from '../ui/SearchField.svelte';
  import Menu from '../ui/Menu.svelte';
  import Chip from '../ui/Chip.svelte';
  import Button from '../ui/Button.svelte';
  import Empty from '../ui/Empty.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Disclosure from '../ui/Disclosure.svelte';
  import LogRow from './activity/LogRow.svelte';
  import ClearSheet from './activity/ClearSheet.svelte';
  import { getStats, getLog, listSkills } from '../lib/api.js';
  import { onEvent, debounce } from '../lib/live.svelte.js';
  import { go } from '../lib/router.svelte.js';
  import { ms, pct } from '../lib/format.js';

  const RANGES = [{ value: '86400', label: '24 h' }, { value: '604800', label: '7 d' }, { value: '2592000', label: '30 d' }];
  let range = $state('86400');
  let stats = $state(null);
  let statsErr = $state('');

  let q = $state('');
  let mode = $state('all');
  let skill = $state('');
  let skills = $state([]);
  let rows = $state([]);
  let next = $state(null);
  let loading = $state(false);
  let logErr = $state('');
  let clearOpen = $state(false);
  let gen = 0;

  const now = () => Math.floor(Date.now() / 1000);

  async function loadStats() {
    try { stats = await getStats(now() - Number(range)); statsErr = ''; } catch (e) { statsErr = e.message; }
  }
  $effect(() => { range; loadStats(); });
  $effect(() => { listSkills().then((s) => (skills = s)).catch(() => {}); });

  const params = (extra = {}) => ({ skill, nohit: mode === 'nohit', q: q.trim(), limit: 50, ...extra });

  async function reload() {
    const g = ++gen;
    loading = true;
    try {
      const r = await getLog(params());
      if (g !== gen) return;
      rows = r.rows; next = r.next; logErr = '';
    } catch (e) { if (g === gen) logErr = e.message; }
    finally { if (g === gen) loading = false; }
  }
  $effect(() => { mode; skill; reload(); });

  async function more() {
    if (next == null || loading) return;
    const g = gen;
    loading = true;
    try {
      const r = await getLog(params({ before: next }));
      if (g !== gen) return;
      rows = [...rows, ...r.rows]; next = r.next;
    } catch (e) { logErr = e.message; }
    finally { loading = false; }
  }

  // Live: new matches appear at the top as they happen, loads tick the row, stats follow.
  const statsLater = debounce(loadStats, 500);
  const fits = (r) => (!skill || r.hits.some((h) => h.name === skill))
    && (mode !== 'nohit' || r.hits.length === 0)
    && (!q.trim() || r.prompt.toLowerCase().includes(q.trim().toLowerCase()));
  $effect(() => onEvent((m) => {
    if (m.type === 'match') {
      if (fits(m.row) && !rows.some((x) => x.id === m.row.id)) rows = [m.row, ...rows];
      statsLater();
    } else if (m.type === 'load') {
      // Same rule as the server: a load counts for matches of that skill in the 120 s before it.
      rows = rows.map((r) => (m.ts - r.ts >= 0 && m.ts - r.ts <= 120 && r.hits.some((h) => h.name === m.skill) && !r.loaded.includes(m.skill))
        ? { ...r, loaded: [...r.loaded, m.skill] } : r);
      statsLater();
    } else if (m.type === 'skill') {
      listSkills().then((x) => (skills = x)).catch(() => {});
    } else if (m.type === 'log_cleared' || m.type === 'resync') {
      reload(); loadStats();
    }
  }));

  const skillOpts = $derived([{ value: '', label: 'All skills' }, ...skills.map((s) => ({ value: s.name, label: s.name }))]);
  const spark = (k) => (stats?.per_day || []).map((d) => d[k]);
  const sparkRate = $derived((stats?.per_day || []).map((d) => (d.matches ? d.with_hit / d.matches : 0)));
  const maxTop = $derived(Math.max(1, ...(stats?.top || []).map((t) => t.matches)));
  const rangeLabel = $derived(RANGES.find((r) => r.value === range)?.label.replace(' ', '\u00a0') ?? '');
  const stale = $derived(skills.filter((x) => x.stale).map((x) => x.name));
  const band = (b) => `${b.lo.toFixed(2)}–${b.hi.toFixed(2)}`;
</script>

<TopBar title="Activity" subtitle="How skills are matched and used">
  <Segmented options={RANGES} bind:value={range} label="Range" />
</TopBar>

<div class="page">
  {#if statsErr}<div class="err" role="alert">{statsErr}</div>{/if}
  {#if stats}
    <div class="tiles">
      <StatTile label="Matches {rangeLabel}" value={stats.matches} spark={spark('matches')} />
      <StatTile label="Hit rate {rangeLabel}" value={pct(stats.hit_rate)} spark={sparkRate} sub="{stats.with_hit} hit" tone={stats.hit_rate >= 0.5 ? 'good' : 'warn'} />
      <StatTile label="MCP searches {rangeLabel}" value={stats.mcp_searches ?? 0} spark={spark('mcp')} />
      <StatTile label="Latency p50 {rangeLabel}" value={ms(stats.p50_ms)} />
      <StatTile label="Latency p95 {rangeLabel}" value={ms(stats.p95_ms)} />
    </div>

    <div class="two">
      <Section title="Top skills">
        <div class="card">
          {#each stats.top as t}
            <button type="button" class="top" onclick={() => go(`/skills/${t.name}`)}>
              <span class="nm">{t.name}</span>
              <span class="bar"><span style="width:{(t.matches / maxTop) * 100}%"></span></span>
              <span class="n">{t.matches} matched</span><span class="n">{t.loads} loaded</span>{#if t.mcp}<span class="n">{t.mcp} mcp</span>{/if}
            </button>
          {:else}
            <div class="none">No matches in this range.</div>
          {/each}
        </div>
      </Section>

      <Section title="Score bands" hint="30 days">
        <div class="card">
          <div class="bt th"><span>Score</span><span>Suggested</span><span>Loaded</span><span>Precision</span></div>
          {#each stats.score_bands as b}
            <div class="bt"><span class="mono">{band(b)}</span><span>{b.suggested}</span><span>{b.loaded}</span>
              <span class="mono">{b.suggested ? pct(b.loaded / b.suggested) : '-'}</span></div>
          {/each}
        </div>
      </Section>
    </div>

    <Section title="Never matched" hint="30 days">
      {#snippet staleChips()}
        <div class="chips">
          {#each stale as n}
            <button type="button" class="lnk" onclick={() => go(`/skills/${n}`)}><Chip tone="warn" mono>{n}</Chip></button>
          {/each}
        </div>
      {/snippet}
      {#if stale.length === 0}
        <span class="none">Every skill matched recently, or the log is too short to tell.</span>
      {:else if stale.length > 12}
        <Disclosure label="{stale.length} skills unused for 30 days">{@render staleChips()}</Disclosure>
      {:else}
        {@render staleChips()}
      {/if}
    </Section>
  {/if}

  <Section title="Match log">
    {#snippet actions()}
      <Button variant="plain" size="sm" onclick={() => (clearOpen = true)}>Clear history…</Button>
    {/snippet}
    <div class="filters">
      <div class="q"><SearchField bind:value={q} placeholder="Search prompts" label="Search prompts" onsubmit={reload} /></div>
      <Segmented options={[{ value: 'all', label: 'All' }, { value: 'nohit', label: 'No match' }]} bind:value={mode} label="Filter" />
      <Menu options={skillOpts} bind:value={skill} label="Skill" align="right" />
    </div>

    {#if logErr}<div class="err" role="alert">{logErr}</div>{/if}
    <div class="card log">
      {#each rows as r (r.id)}<LogRow row={r} />{/each}
      {#if !rows.length && !loading}
        <Empty icon="activity" title="No log entries" hint="Matches made by the hook and agents' skill_search calls appear here." />
      {/if}
      {#if loading && !rows.length}<div class="none c"><Spinner /></div>{/if}
    </div>
    {#if next != null}
      <div class="more"><Button onclick={more} loading={loading}>Load more</Button></div>
    {/if}
  </Section>
</div>

<ClearSheet open={clearOpen} onclose={() => (clearOpen = false)} ondone={() => { reload(); loadStats(); }} />

<style>
  .page { padding: var(--s-5); display: flex; flex-direction: column; gap: var(--s-5); max-width: 1200px; }
  .tiles { display: flex; flex-wrap: nowrap; gap: var(--s-2); }
  .two { display: grid; grid-template-columns: 3fr 2fr; gap: var(--s-5); }
  .card { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); overflow: hidden; }
  .top { display: grid; grid-template-columns: minmax(80px, 1.2fr) 1fr auto auto; gap: var(--s-3); align-items: center; width: 100%; padding: 8px var(--s-4); border: 0; border-bottom: 1px solid var(--line); background: none; text-align: left; }
  .top:last-child { border-bottom: 0; }
  .top:hover { background: var(--surface-hover); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 550; }
  .bar { height: 6px; border-radius: 3px; background: rgba(255,255,255,.08); }
  .bar span { display: block; height: 100%; border-radius: 3px; background: var(--accent); }
  .n { color: var(--text-3); font-size: var(--t-xs); white-space: nowrap; }
  .bt { display: grid; grid-template-columns: minmax(0, 1.3fr) repeat(3, minmax(0, 1fr)); gap: var(--s-2); padding: 8px var(--s-4); border-bottom: 1px solid var(--line); font-variant-numeric: tabular-nums; }
  .bt:last-child { border-bottom: 0; }
  .th { color: var(--text-3); font-size: var(--t-xs); text-transform: uppercase; letter-spacing: .04em; }
  .th span { overflow: hidden; text-overflow: ellipsis; }
  .mono { font-family: var(--mono); font-size: var(--t-sm); }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .lnk { border: 0; background: none; padding: 0; }
  .lnk:hover { filter: brightness(1.2); }
  .none { color: var(--text-3); font-size: var(--t-sm); padding: var(--s-3) var(--s-4); }
  .c { display: grid; place-items: center; padding: var(--s-5); }
  .filters { display: flex; gap: var(--s-3); align-items: center; flex-wrap: wrap; }
  .q { flex: 1; min-width: 200px; max-width: 420px; }
  .more { display: flex; justify-content: center; }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-sm); background: var(--bad-soft); color: var(--bad); }
  @media (max-width: 480px) { .bt { padding: 8px var(--s-3); } .th { font-size: 10px; letter-spacing: 0; } .top { grid-template-columns: minmax(60px, 1fr) 0.8fr auto; } .top .n:last-child { display: none; } }
  @media (max-width: 900px) { .two { grid-template-columns: 1fr; } .page { padding: var(--s-4); } }
</style>
