<!-- LoginSheet: shown when auth.needed. Submits the token to /api/login, then reloads. -->
<script>
  import Sheet from '../ui/Sheet.svelte';
  import Button from '../ui/Button.svelte';
  import { login } from '../lib/api.js';
  import { auth } from '../lib/auth.svelte.js';

  let token = $state('');
  let error = $state('');
  let busy = $state(false);

  async function submit(e) {
    e?.preventDefault();
    if (!token.trim() || busy) return;
    busy = true;
    error = '';
    try {
      await login(token.trim());
      auth.needed = false;
      location.reload();
    } catch (err) {
      error = /unauthor|401|token/i.test(err.message) ? 'That token was not accepted.' : err.message;
      auth.needed = true;
    } finally {
      busy = false;
    }
  }
</script>

<Sheet open title="Connect to Mímir" dismissible={false} width={400}>
  <form onsubmit={submit}>
    <p class="hint">Enter the server token to browse skills and activity.</p>
    <p class="hint">Find it on the server with <code>grep MIMIR_TOKEN ~/.config/mimir/env</code>.</p>
    <input class="field" class:bad={!!error} type="password" bind:value={token} placeholder="Token" aria-label="Token"
      autocomplete="current-password" spellcheck="false" />
    {#if error}<p class="err" role="alert">{error}</p>{/if}
    <Button variant="primary" type="submit" loading={busy} disabled={!token.trim()}>Connect</Button>
  </form>
</Sheet>

<style>
  form { display: flex; flex-direction: column; gap: var(--s-3); }
  .hint { color: var(--text-2); font-size: var(--t-sm); }
  code { font-family: var(--mono); font-size: var(--t-xs); padding: 1px 5px; border-radius: var(--r-xs); background: rgba(255,255,255,.08); user-select: all; overflow-wrap: anywhere; }
  .field { height: 36px; padding: 0 12px; border-radius: var(--r-sm); border: 1px solid var(--glass-border); background: var(--glass-fill); box-shadow: var(--glass-highlight); color: var(--text); font-family: var(--mono); transition: border-color var(--fast); }
  .field:focus { border-color: var(--accent); outline: 0; }
  .field.bad { border-color: var(--bad); }
  .err { color: var(--bad); font-size: var(--t-sm); }
</style>
