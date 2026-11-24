<!-- Playground: run a prompt through the matcher, inspect candidates, optionally compare with/without reranker. -->
<script>
  import TopBar from '../layout/TopBar.svelte';
  import Button from '../ui/Button.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Menu from '../ui/Menu.svelte';
  import Empty from '../ui/Empty.svelte';
  import Icon from '../ui/Icon.svelte';
  import ResultColumn from './playground/ResultColumn.svelte';
  import { match, getConfig } from '../lib/api.js';
  import { route } from '../lib/router.svelte.js';
  import { toast } from '../lib/toast.svelte.js';

  const KEY = 'mimir.prompts';
  let prompt = $state('');
  let compare = $state(false);
  let lexical = $state(false);
  let cfgError = $state('');
  let busy = $state(false);
  let error = $state('');
  let config = $state(null);
  let main = $state(null);   // rerank:true run
  let plain = $state(null);  // rerank:false run (compare)
  let expected = $state('-');
  let ta;

  let recent = $state(load());
  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]').slice(0, 10); } catch { return []; } }
  function remember(p) {
    recent = [p, ...recent.filter((x) => x !== p)].slice(0, 10);
    try { localStorage.setItem(KEY, JSON.stringify(recent)); } catch {}
  }

  $effect(() => { getConfig().then((c) => (config = c)).catch((e) => (cfgError = `Could not load config: ${e.message}`)); });

  // Replay from Activity: #/playground?prompt=...
  let replayed = '';
  $effect(() => {
    const q = route.query?.prompt;
    if (q && config && q !== replayed) { replayed = q; prompt = q; run(); }
  });

  const min = $derived(config?.min);
  const top = $derived((main?.hits || []).find((h) => min != null && h.score >= min)?.name ?? '-');
  const options = $derived([{ value: '-', label: 'No skill (-)' }, ...(main?.hits || []).map((h) => ({ value: h.name, label: h.name }))]);
  $effect(() => { expected = top; });

  function fit() {
    if (!ta) return;
    ta.style.height = 'auto';
    ta.style.height = Math.min(ta.scrollHeight, 320) + 'px';
  }
  $effect(() => { prompt; fit(); });

  async function timed(rerank) {
    const t = performance.now();
    const r = await match({ prompt, k: config.candidates, min: -10, rerank, lexical: lexical || undefined });
    return { hits: r.hits, ms: r.ms ?? Math.round(performance.now() - t) };
  }

  async function run() {
    const p = prompt.trim();
    if (!p || busy || !config) return;
    busy = true;
    error = '';
    try {
      const [a, b] = await Promise.all([timed(true), compare ? timed(false) : Promise.resolve(null)]);
      main = a;
      plain = b;
      remember(p);
    } catch (e) {
      error = e.message;
    } finally {
      busy = false;
    }
  }

  function onkey(e) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); run(); }
  }

  async function copyCase() {
    const line = `${prompt.trim()}\t${expected}`;
    try { await navigator.clipboard.writeText(line); toast('Copied eval case', 'good'); }
    catch { toast('Clipboard unavailable', 'warn'); }
  }
</script>

<TopBar title="Playground" subtitle="Trace a prompt through the matcher" />

<div class="page">
  <div class="compose">
    <textarea bind:this={ta} bind:value={prompt} onkeydown={onkey} rows="3" spellcheck="false"
      placeholder="Type a prompt as the user would..." aria-label="Prompt"></textarea>
    <div class="bar">
      <Toggle bind:checked={compare} label="Compare without reranker" />
      <Toggle bind:checked={lexical} label="Lexical rescue (experimental)" />
      <span class="sp"></span>
      <span class="kbd">Ctrl/Cmd + Enter</span>
      <Button variant="primary" onclick={run} loading={busy} disabled={!prompt.trim() || !config}>Run</Button>
    </div>
    {#if recent.length}
      <div class="chips">
        {#each recent as p}
          <button type="button" class="rc" onclick={() => { prompt = p; ta?.focus(); }}>{p}</button>
        {/each}
      </div>
    {/if}
  </div>

  {#if cfgError}<div class="err" role="alert">{cfgError}</div>{/if}
  {#if error}<div class="err" role="alert">{error}</div>{/if}

  {#if main}
    <div class="grid" class:two={!!plain}>
      <ResultColumn title={plain ? 'With reranker' : 'Pipeline'} run={main} {min} scoreLabel="Reranker" />
      {#if plain}<ResultColumn title="Cosine only" run={plain} {min} scoreLabel="Cosine" cosineCol={false} />{/if}
    </div>
    <div class="eval">
      <span class="lbl">Expected skill</span>
      <Menu {options} bind:value={expected} label="Expected skill" />
      <Button onclick={copyCase}><Icon name="copy" size={14} /> Copy as eval case</Button>
    </div>
  {:else if !busy}
    <Empty icon="flask" title="Nothing run yet" hint="Candidates, cosine and reranker scores, and the final suggestions appear here." />
  {/if}
</div>

<style>
  .page { padding: var(--s-5); display: flex; flex-direction: column; gap: var(--s-4); max-width: 1200px; }
  .compose { display: flex; flex-direction: column; gap: var(--s-3); }
  textarea { width: 100%; resize: none; min-height: 96px; padding: var(--s-3) var(--s-4); font-size: var(--t-md); line-height: 1.5; border-radius: var(--r-md); border: 1px solid var(--glass-border); background: var(--glass-fill); box-shadow: var(--glass-highlight); backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); transition: border-color var(--fast); }
  textarea:focus { border-color: var(--accent); outline: 0; }
  textarea::placeholder { color: var(--text-3); }
  .bar { display: flex; align-items: center; gap: var(--s-3); flex-wrap: wrap; }
  .sp { flex: 1; }
  .kbd { color: var(--text-3); font-size: var(--t-xs); }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .rc { max-width: 280px; height: 24px; padding: 0 10px; border: 1px solid var(--line); border-radius: var(--r-pill); background: rgba(255,255,255,.05); color: var(--text-2); font-size: var(--t-xs); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rc:hover { background: rgba(255,255,255,.1); color: var(--text); }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-sm); background: var(--bad-soft); color: var(--bad); }
  .grid { display: grid; gap: var(--s-4); grid-template-columns: 1fr; }
  .grid.two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .eval { display: flex; align-items: center; gap: var(--s-3); flex-wrap: wrap; }
  .lbl { color: var(--text-3); font-size: var(--t-sm); }
  @media (max-width: 900px) { .grid.two { grid-template-columns: 1fr; } .page { padding: var(--s-4); } }
</style>
