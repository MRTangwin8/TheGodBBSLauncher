<script setup lang="ts">
// 现代布局用的线条图标（原创，24×24 描边）
const PATHS: Record<string, string[]> = {
  home: ["M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"],
  download: ["M12 3v12", "m7.5 10.5 4.5 4.5 4.5-4.5", "M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"],
  versions: ["M12 3 2.5 8 12 13l9.5-5z", "m2.5 12.5 9.5 5 9.5-5", "m2.5 17 9.5 5 9.5-5"],
  resources: ["M21 8 12 3 3 8v8l9 5 9-5z", "m3 8 9 5 9-5", "M12 13v8"],
  log: ["M4 4h16a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z", "m7 9 3 3-3 3", "M12.5 15H17"],
  user: ["M12 12a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9z", "M3.5 21a8.5 8.5 0 0 1 17 0"],
  settings: ["M4 6h9", "M17 6h3", "M4 12h3", "M11 12h9", "M4 18h11", "M19 18h1", "M15 4v4", "M9 10v4", "M17 16v4"],
  palette: [
    "M12 3a9 9 0 1 0 0 18c1.2 0 1.6-.8 1.6-1.6 0-.5-.2-.8-.5-1.2a1.6 1.6 0 0 1 1.2-2.7H17a4.5 4.5 0 0 0 4-4.5C21 6.6 17 3 12 3z",
    "M7.5 11h.01",
    "M10 7h.01",
    "M14.5 7h.01",
    "M17 11h.01",
  ],
  folder: ["M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"],
};

withDefaults(defineProps<{ name: string; size?: number }>(), { size: 20 });
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path v-for="d in PATHS[name] ?? PATHS.home" :key="d" :d="d" />
  </svg>
</template>
