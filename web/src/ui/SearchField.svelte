<!--
  SearchField: custom text input with search icon + clear button.
  Props: value (bindable), placeholder, onsubmit(value), oninput(value), label (aria), autofocus.
-->
<script>
  import Icon from './Icon.svelte';
  let { value = $bindable(''), placeholder = 'Search', onsubmit, oninput, label = 'Search', autofocus = false } = $props();
  let el;
  $effect(() => { if (autofocus) el?.focus(); });
</script>

<div class="sf">
  <Icon name="search" size={15} />
  <input bind:this={el} bind:value type="text" {placeholder} aria-label={label} autocomplete="off" spellcheck="false"
    oninput={() => oninput?.(value)}
    onkeydown={(e) => { if (e.key === 'Enter') onsubmit?.(value); else if (e.key === 'Escape' && value) { value = ''; oninput?.(''); onsubmit?.(''); } }} />
  {#if value}
    <button type="button" class="x" aria-label="Clear" onclick={() => { value = ''; oninput?.(''); onsubmit?.(''); el?.focus(); }}><Icon name="close" size={13} /></button>
  {/if}
</div>

<style>
  .sf { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 8px 0 10px; min-width: 0; width: 100%; color: var(--text-3);
    background: var(--glass-fill); border: 1px solid var(--glass-border); border-radius: var(--r-sm); box-shadow: var(--glass-highlight);
    backdrop-filter: var(--glass-blur); -webkit-backdrop-filter: var(--glass-blur); transition: border-color var(--fast), background var(--fast); }
  .sf:focus-within { border-color: var(--accent); background: rgba(30,35,48,.8); }
  input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--text); font-size: var(--t-base); padding: 0; }
  input::placeholder { color: var(--text-3); }
  .x { display: grid; place-items: center; width: 20px; height: 20px; border: 0; border-radius: 50%; background: rgba(255,255,255,.12); color: var(--text-2); padding: 0; }
  .x:hover { background: rgba(255,255,255,.2); color: var(--text); }
</style>
