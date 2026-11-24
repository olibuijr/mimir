<!-- ClearSheet: confirm deleting log history. Props: open, onclose, ondone(deleted). -->
<script>
  import Sheet from '../../ui/Sheet.svelte';
  import Button from '../../ui/Button.svelte';
  import Segmented from '../../ui/Segmented.svelte';
  import { clearLog } from '../../lib/api.js';
  import { toast } from '../../lib/toast.svelte.js';

  let { open = false, onclose, ondone } = $props();
  let age = $state('30');
  let busy = $state(false);
  const opts = [{ value: '7', label: 'Older than 7 days' }, { value: '30', label: 'Older than 30 days' }, { value: '0', label: 'Everything' }];

  async function confirm() {
    busy = true;
    try {
      const days = Number(age);
      const r = await clearLog(Math.floor(Date.now() / 1000) - days * 86400 + (days === 0 ? 60 : 0));
      toast(`Deleted ${r?.deleted ?? 0} log rows`, 'good');
      ondone?.(r?.deleted ?? 0);
      onclose?.();
    } catch (e) {
      toast(e.message, 'warn');
    } finally {
      busy = false;
    }
  }
</script>

<Sheet {open} {onclose} title="Clear history">
  <p class="t">Permanently delete match log entries. Skills are not affected.</p>
  <Segmented options={opts} bind:value={age} label="Age" />
  {#snippet actions()}
    <Button variant="plain" onclick={onclose}>Cancel</Button>
    <Button variant="danger" loading={busy} onclick={confirm}>Delete</Button>
  {/snippet}
</Sheet>

<style>
  .t { color: var(--text-2); font-size: var(--t-sm); }
</style>
