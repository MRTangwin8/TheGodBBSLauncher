<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../../api";
import type { QuickButton } from "../../api/types";
import { useButtons } from "../../buttons";
import { describeQuick } from "../../buttons-logic";
import { useLauncher } from "../../composables/useLauncher";

// 首页的自定义快捷按钮（主题中心 → 首页快捷按钮）
const { buttons } = useButtons();
const { launch, versions } = useLauncher();
const error = ref("");

async function run(b: QuickButton) {
  error.value = "";
  const a = b.action;
  try {
    switch (a.type) {
      case "openFolder":
        await api.quickOpenFolder(a.path);
        break;
      case "openUrl":
        if (inTauri) await openUrl(a.url);
        else window.open(a.url, "_blank", "noopener,noreferrer");
        break;
      case "launch":
        if (!versions.installed.some((v) => v.id === a.versionId)) {
          error.value = `版本「${a.versionId}」已经不存在了，到「主题中心 → 首页快捷按钮」里改一下`;
          return;
        }
        await launch(a.versionId, a.server || null);
        break;
      case "runProgram":
        // 每次运行前都让用户看一眼完整路径；后端还会再核对这个路径确实保存在按钮里
        if (!window.confirm(`要运行这个程序吗？\n\n${a.path}\n\n只运行你自己添加的程序。`)) return;
        await api.quickRunProgram(a.path);
        break;
    }
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <div v-if="buttons.quick.length" class="quick-buttons">
    <button v-for="b in buttons.quick" :key="b.id" class="btn small" :title="describeQuick(b.action)" @click="run(b)">
      <span v-if="b.icon" class="qicon">{{ b.icon }}</span>{{ b.label }}
    </button>
    <span v-if="error" class="error-text small">{{ error }}</span>
  </div>
</template>

<style scoped>
.quick-buttons { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 10px 0 0; }
.qicon { margin-right: 4px; }
.small { font-size: 12px; }
</style>
