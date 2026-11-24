<!-- Spinner. Props: size=16. -->
<script>
  let { size = 16 } = $props();
</script>
<svg class="sp" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" role="status" aria-label="Loading"><path d="M12 3a9 9 0 1 1-9 9" opacity=".9" /></svg>
<style>
  .sp { animation: spin .8s linear infinite; color: var(--text-2); flex: none; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
