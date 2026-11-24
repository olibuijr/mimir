<!--
  Icon: inline stroke SVG. Props: name, size=16.
  Names: search library flask activity file folder key chevron close copy check warning refresh logout external
-->
<script>
  let { name, size = 16 } = $props();
  const P = {
    search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4',
    library: 'M5 4h4v16H5zM11 4h4v16h-4zM17.5 5.5l3.5 1-3.5 13-3.5-1z',
    flask: 'M9 3h6M10 3v6L5 19a1.5 1.5 0 0 0 1.4 2h11.2a1.5 1.5 0 0 0 1.4-2l-5-10V3M7.5 15h9',
    activity: 'M3 12h4l3-8 4 16 3-8h4',
    file: 'M7 3h7l5 5v13H7zM14 3v5h5',
    folder: 'M3 6.5A1.5 1.5 0 0 1 4.5 5H10l2 2.5h7.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z',
    key: 'M14 9a5 5 0 1 1-1.5 3.5L4 21v-3h3v-3h3l2.5-2.5A5 5 0 0 1 14 9zM17 8.5h.01',
    chevron: 'M9 5l7 7-7 7',
    close: 'M6 6l12 12M18 6L6 18',
    copy: 'M9 9h10v11H9zM5 15V4h10',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    warning: 'M12 4l9.5 16h-19zM12 10v4.5M12 17.5h.01',
    refresh: 'M20 11a8 8 0 0 0-14.5-4M4 4v4h4M4 13a8 8 0 0 0 14.5 4M20 20v-4h-4',
    logout: 'M10 4H5v16h5M15 8l4 4-4 4M19 12H9',
    external: 'M14 4h6v6M20 4l-9 9M18 14v5H5V6h5',
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" style="flex:none"><path d={P[name]} /></svg>
