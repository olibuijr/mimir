<!-- ListGroup: inset grouped list container (content surface, not glass). Holds ListRow children. Props: children. -->
<script>
  let { children } = $props();
</script>
<div class="lg">{@render children?.()}</div>
<style>
  .lg { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-md); overflow: hidden; }
  .lg :global(.row + .row) { border-top: 1px solid var(--line); }
</style>
