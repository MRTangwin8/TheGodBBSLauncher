<script setup lang="ts">
import { ask } from "../../utils";
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../../api";
import type { QuickButton } from "../../api/types";
import { useButtons } from "../../buttons";
import { describeQuick } from "../../buttons-logic";
import { useLauncher } from "../../composables/useLauncher";
import { errText, t } from "../../i18n";

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
          error.value = t("quickRun.versionGone", { version: a.versionId });
          return;
        }
        await launch(a.versionId, a.server || null);
        break;
      case "runProgram":
        // 每次运行前都让用户看一眼完整路径；后端还会再核对这个路径确实保存在按钮里
        if (!(await ask(t("quickRun.confirm", { path: a.path })))) return;
        await api.quickRunProgram(a.path);
        break;
    }
  } catch (e) {
    error.value = errText(e);
  }
}
</script>

<template>
  <div v-if="buttons.quick.length" class="quick-buttons">
    <button v-for="b in buttons.quick" :key="b.id" class="btn small" :title="$t(describeQuick(b.action).key, describeQuick(b.action).params)" @click="run(b)">
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
