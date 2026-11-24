<!-- Sparkline: inline SVG polyline. Props: values number[], width=80, height=24, color='var(--accent)'. -->
<script>
  let { values = [], width = 80, height = 24, color = 'var(--accent)' } = $props();
  const pts = $derived.by(() => {
    if (values.length < 2) return '';
    const max = Math.max(...values), min = Math.min(...values), span = max - min || 1, p = 2;
    return values.map((v, i) => `${(p + (i * (width - 2 * p)) / (values.length - 1)).toFixed(1)},${(height - p - ((v - min) / span) * (height - 2 * p)).toFixed(1)}`).join(' ');
  });
</script>
<svg {width} {height} viewBox="0 0 {width} {height}" aria-hidden="true">
  {#if pts}<polyline points={pts} fill="none" stroke={color} stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round" />{/if}
</svg>
