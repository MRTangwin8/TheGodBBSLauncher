<script setup lang="ts">
import { computed } from "vue";

// 原创像素头像：按 UUID 生成固定的配色，不去拉取真实皮肤（以后再做）
const props = withDefaults(defineProps<{ seed: string; size?: number }>(), { size: 72 });

const palette = computed(() => {
  let h = 0;
  for (const c of props.seed) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const hair = ["#6b4a2b", "#2b1d10", "#c9a227", "#8a3b12", "#3d3d3d"][h % 5];
  const skin = ["#c69c6d", "#e0b48a", "#a4764c", "#f1c9a5"][(h >> 3) % 4];
  const eye = ["#3b5ba8", "#2f7d32", "#6b3fa0", "#5a3a20"][(h >> 5) % 4];
  return { hair, skin, eye };
});
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 8 8" shape-rendering="crispEdges" class="head" aria-hidden="true">
    <rect width="8" height="8" :fill="palette.hair" />
    <rect y="2" width="8" height="6" :fill="palette.skin" />
    <rect x="1" y="4" width="2" height="1" fill="#fff" /><rect x="2" y="4" width="1" height="1" :fill="palette.eye" />
    <rect x="5" y="4" width="2" height="1" fill="#fff" /><rect x="5" y="4" width="1" height="1" :fill="palette.eye" />
    <rect x="3" y="6" width="2" height="1" fill="#7a4a2a" />
  </svg>
</template>

<style scoped>
.head { border: 2px solid var(--border-strong); image-rendering: pixelated; display: block; }
</style>
