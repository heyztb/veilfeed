<script lang="ts">
  let { name, size = 16 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    inbox: "M3 4h18v16H3V4Zm0 11h5l2 2h4l2-2h5",
    unread: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Zm10-3a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z",
    star: "m12 2 3.1 6.3 6.9 1-5 4.8 1.2 6.8-6.2-3.3L5.8 21 7 14.1 2 9.3l6.9-1L12 2Z",
    folder: "M3 5h7l2 2h9v12H3V5Z",
    "folder-plus": "M3 6h7l2 2h9v11H3V6Zm12 5v6m-3-3h6",
    feed: "M5 5a14 14 0 0 1 14 14M5 11a8 8 0 0 1 8 8M6 19h.01",
    refresh: "M20 7v5h-5M4 17v-5h5M6.1 8A7 7 0 0 1 18 7l2 5M4 12l2 5a7 7 0 0 0 11.9-1",
    plus: "M12 5v14M5 12h14",
    settings: "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7ZM19 13.5l2 1.5-2 3.5-2.4-1a8 8 0 0 1-2.6 1.5l-.3 2.5h-4l-.3-2.5a8 8 0 0 1-2.6-1.5l-2.4 1-2-3.5 2-1.5a8 8 0 0 1 0-3l-2-1.5 2-3.5 2.4 1A8 8 0 0 1 9.4 5l.3-2.5h4L14 5a8 8 0 0 1 2.6 1.5l2.4-1 2 3.5-2 1.5a8 8 0 0 1 0 3Z",
    search: "m20 20-4.5-4.5M10.5 18a7.5 7.5 0 1 1 0-15 7.5 7.5 0 0 1 0 15Z",
    chevron: "m9 6 6 6-6 6",
    external: "M14 4h6v6M20 4l-9 9M18 13v7H4V6h7",
    check: "m5 12 4 4L19 6",
    trash: "M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6",
  };
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] ?? paths.feed}></path></svg>
