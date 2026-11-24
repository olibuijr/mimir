<!-- ResultColumn: one pipeline run. Props: title, run {hits, ms}, min, scoreLabel. Shows candidates + final suggestions. -->
<script>
  import ScoreBar from '../../ui/ScoreBar.svelte';
  import Chip from '../../ui/Chip.svelte';
  import { go } from '../../lib/router.svelte.js';
  import { ms as fmtMs } from '../../lib/format.js';

  let { title, run, min, scoreLabel = 'Score', cosineCol = true, auto = 0.9 } = $props();
  const hits = $derived(run.hits || []);
  const final = $derived(hits.filter((h) => h.score >= min).slice(0, 3));
</script>

<div class="col">
  <div class="hd"><h3>{title}</h3><Chip mono>{fmtMs(run.ms)}</Chip></div>

  <div class="final">
    <div class="cap">Hook would inject</div>
    {#if final.length}
      {#each final as h, i}
        <button type="button" class="sug" onclick={() => go(`/skills/${h.name}`)}>
          <span class="rk">{i + 1}</span><span class="nm">{h.name}</span>{#if h.score >= auto}<Chip tone="good">auto-loads</Chip>{/if}<span class="sc">{h.score.toFixed(2)}</span>
        </button>
      {/each}
    {:else}
      <div class="none">Nothing passes the threshold ({min}).</div>
    {/if}
  </div>

  <div class="tbl" role="table" aria-label="{title} candidates">
    <div class="tr th" class:one={!cosineCol} role="row"><span>Candidate</span>{#if cosineCol}<span>Cosine</span>{/if}<span>{scoreLabel}</span></div>
    {#each hits as h}
      <div class="tr" class:one={!cosineCol} class:pass={h.score >= min} role="row">
        <button type="button" class="nm" onclick={() => go(`/skills/${h.name}`)}>{h.name}</button>
        {#if cosineCol}<ScoreBar value={h.cosine} />{/if}
        <ScoreBar value={h.score} threshold={min} />
      </div>
    {/each}
    {#if !hits.length}<div class="none pad">No candidates.</div>{/if}
  </div>
</div>

<style>
  .col { display: flex; flex-direction: column; gap: var(--s-3); min-width: 0; background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-4); }
  .hd { display: flex; align-items: center; justify-content: space-between; }
  .cap { color: var(--text-3); font-size: var(--t-xs); text-transform: uppercase; letter-spacing: .06em; margin-bottom: 4px; }
  .final { display: flex; flex-direction: column; gap: 4px; }
  .sug { display: flex; align-items: center; gap: var(--s-3); padding: 7px 10px; border: 1px solid rgba(90,209,154,.25); border-radius: var(--r-sm); background: var(--good-soft); text-align: left; }
  .sug:hover { background: rgba(90,209,154,.22); }
  .rk { color: var(--good); font-weight: 650; width: 14px; }
  .nm { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 550; }
  .sc { font-family: var(--mono); font-size: var(--t-xs); color: var(--good); }
  .none { color: var(--text-3); font-size: var(--t-sm); }
  .pad { padding: var(--s-3); }
  .tbl { display: flex; flex-direction: column; }
  .tr { display: grid; grid-template-columns: minmax(90px, 1.3fr) 1fr 1fr; gap: var(--s-3); align-items: center; padding: 6px 8px; border-radius: var(--r-xs); border-left: 2px solid transparent; }
  .tr.one { grid-template-columns: minmax(90px, 1.3fr) 2fr; }
  .tr.pass { background: var(--accent-soft); border-left-color: var(--accent); }
  .th { color: var(--text-3); font-size: var(--t-xs); text-transform: uppercase; letter-spacing: .06em; padding-bottom: 4px; }
  button.nm { border: 0; background: none; padding: 0; text-align: left; color: var(--text); font-size: var(--t-sm); }
  button.nm:hover { color: var(--accent); }
</style>
