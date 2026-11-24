<!-- CodeView: monospace block with line numbers, custom horizontal scroll and a copy button. Props: code, maxHeight='480px'. -->
<script>
  import IconButton from './IconButton.svelte';
  import { toast } from '../lib/toast.svelte.js';
  let { code = '', maxHeight = '480px' } = $props();
  const lines = $derived(code.replace(/\n$/, '').split('\n'));
  async function copy() {
    try { await navigator.clipboard.writeText(code); toast('Copied', 'good'); } catch { toast('Copy failed', 'warn'); }
  }
</script>
<div class="cv">
  <div class="cp"><IconButton icon="copy" label="Copy" size={28} variant="glass" onclick={copy} /></div>
  <!-- scrollable code must be keyboard-scrollable -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="sc" style="max-height:{maxHeight}" tabindex="0" role="region" aria-label="Code">
    <table><tbody>
      {#each lines as l, i}<tr><td class="n">{i + 1}</td><td class="c">{l || ' '}</td></tr>{/each}
    </tbody></table>
  </div>
</div>
<style>
  .cv { position: relative; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r-md); }
  .cp { position: absolute; top: 8px; right: 8px; z-index: 2; opacity: 0; transition: opacity var(--fast); }
  .cv:hover .cp, .cv:focus-within .cp { opacity: 1; }
  .sc { overflow: auto; border-radius: var(--r-md); padding: var(--s-3) 0; }
  table { border-collapse: collapse; font-family: var(--mono); font-size: 12.5px; line-height: 1.6; }
  td { padding: 0; vertical-align: top; }
  .n { position: sticky; left: 0; padding: 0 var(--s-3) 0 var(--s-4); text-align: right; color: var(--text-3); user-select: none; background: var(--bg-2); }
  .c { padding: 0 var(--s-5) 0 var(--s-2); white-space: pre; color: var(--text); }
</style>
