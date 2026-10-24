<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

// 去掉系统标题栏后自己画的最小化 / 最大化 / 关闭。
// tone=light：白色带阴影，压在图片/天空上；tone=auto：跟随文字颜色（现代布局）
withDefaults(defineProps<{ tone?: "light" | "auto" }>(), { tone: "light" });
const win = getCurrentWindow();
const maximized = ref(false);
let unlisten: (() => void) | undefined;

async function refresh() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  await refresh();
  unlisten = await win.onResized(refresh);
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="controls" :class="tone">
    <button class="ctl" title="最小化" @click="win.minimize()">
      <svg viewBox="0 0 10 10"><rect x="1" y="5" width="8" height="1" /></svg>
    </button>
    <button class="ctl" :title="maximized ? '还原' : '最大化'" @click="win.toggleMaximize()">
      <svg v-if="maximized" viewBox="0 0 10 10">
        <path d="M3 1h6v6h-1V2H3z" />
        <path d="M1 3h6v6H1zm1 1v4h4V4z" fill-rule="evenodd" />
      </svg>
      <svg v-else viewBox="0 0 10 10"><path d="M1 1h8v8H1zm1 1v6h6V2z" fill-rule="evenodd" /></svg>
    </button>
    <button class="ctl close" title="关闭" @click="win.close()">
      <svg viewBox="0 0 10 10"><path d="M1.7 1 5 4.3 8.3 1l.7.7L5.7 5 9 8.3l-.7.7L5 5.7 1.7 9 1 8.3 4.3 5 1 1.7z" /></svg>
    </button>
  </div>
</template>

<style scoped>
.controls { display: flex; align-self: stretch; }
.ctl {
  width: 46px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #fff;
  cursor: pointer;
  filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.6));
  transition: background 0.12s;
}
.auto .ctl { color: var(--text); filter: none; }
.auto .ctl:hover { background: color-mix(in srgb, var(--text) 10%, transparent); }
.ctl svg { width: 10px; height: 10px; fill: currentColor; }
.ctl:hover { background: rgba(255, 255, 255, 0.18); }
.ctl:active { background: rgba(255, 255, 255, 0.28); }
.ctl.close:hover { background: #e81123; filter: none; }
.ctl.close:active { background: #b50e1d; }
</style>
