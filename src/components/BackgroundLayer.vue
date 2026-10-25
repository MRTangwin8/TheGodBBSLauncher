<script setup lang="ts">
import { backgroundUrl } from "../appearance";

// 背景：自定义图片优先；否则按布局用像素风景（侧边栏）、论坛点阵（经典）或渐变（现代）
defineProps<{ kind: "scene" | "checker" | "mesh" }>();
</script>

<template>
  <div v-if="backgroundUrl" class="bg custom" :style="{ backgroundImage: `url(${backgroundUrl})` }" />
  <div v-else-if="kind === 'scene'" class="bg scene"><div class="ground" /></div>
  <div v-else-if="kind === 'checker'" class="bg checker" />
  <div v-else class="bg mesh" />
  <!-- 主题中心的"背景压暗" -->
  <div class="dim" />
</template>

<style scoped>
.bg {
  position: fixed;
  /* 模糊会让边缘透出底色，往外多铺一圈 */
  inset: calc(var(--bg-blur, 0px) * -2);
  z-index: -1;
  background-size: cover;
  background-position: center;
  filter: var(--bg-filter, none);
}
.dim { position: fixed; inset: 0; z-index: -1; background: #000; opacity: var(--bg-dim, 0); pointer-events: none; }
.custom::after {
  /* 盖一层渐变，保证文字看得清 */
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0.05), rgba(0, 0, 0, 0.25));
}

.scene { background: linear-gradient(to bottom, var(--sky-top), var(--sky-bottom)); }
.ground {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 26vh;
  /* 草皮 + 原创 8×8 像素泥土纹理（放大 6 倍平铺），配色方案通过滤镜染成别的方块 */
  background:
    linear-gradient(to bottom, var(--grass-top) 0, var(--grass-top) 12px, rgba(0, 0, 0, 0.12) 12px, rgba(0, 0, 0, 0.12) 18px, transparent 18px),
    url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 8 8' shape-rendering='crispEdges'%3E%3Crect width='8' height='8' fill='%23866043'/%3E%3Crect x='1' y='1' width='1' height='1' fill='%236b4a2b'/%3E%3Crect x='5' y='0' width='1' height='1' fill='%239a6e47'/%3E%3Crect x='3' y='3' width='2' height='1' fill='%236b4a2b'/%3E%3Crect x='6' y='4' width='1' height='1' fill='%235a3a20'/%3E%3Crect x='0' y='5' width='1' height='1' fill='%239a6e47'/%3E%3Crect x='2' y='6' width='1' height='1' fill='%235a3a20'/%3E%3Crect x='5' y='7' width='2' height='1' fill='%236b4a2b'/%3E%3Crect x='7' y='2' width='1' height='1' fill='%239a6e47'/%3E%3C/svg%3E")
      0 18px / 48px 48px;
  image-rendering: pixelated;
}
.ground::after {
  /* 滤镜只作用在泥土上，草皮色由 --grass-top 单独控制 */
  content: "";
  position: absolute;
  inset: 18px 0 0;
  backdrop-filter: var(--ground-filter);
}

/* 第一版的论坛点阵底 */
.checker {
  background-color: var(--page-bg);
  background-image:
    linear-gradient(45deg, var(--page-dot) 25%, transparent 25%, transparent 75%, var(--page-dot) 75%),
    linear-gradient(45deg, var(--page-dot) 25%, transparent 25%, transparent 75%, var(--page-dot) 75%);
  background-size: 16px 16px;
  background-position: 0 0, 8px 8px;
}

/* 现代：强调色晕染的渐变 */
.mesh {
  background:
    radial-gradient(at 15% 10%, color-mix(in srgb, var(--grass) 38%, transparent), transparent 55%),
    radial-gradient(at 85% 20%, color-mix(in srgb, var(--link) 30%, transparent), transparent 50%),
    radial-gradient(at 60% 95%, color-mix(in srgb, var(--gold) 26%, transparent), transparent 55%),
    var(--mesh-base);
}
</style>
