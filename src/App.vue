<script setup lang="ts">
import { computed, onMounted } from "vue";
import { inTauri } from "./api";
import { useSettingsStore } from "./stores/settings";
import { useVersionsStore } from "./stores/versions";
import { useAccountsStore } from "./stores/accounts";
import { useModpackStore } from "./stores/modpack";
import { currentLayout, setupAppearance } from "./appearance";
import ProgressFloat from "./components/ProgressFloat.vue";
import ModpackImportDialog from "./components/ModpackImportDialog.vue";
import ClassicShell from "./layouts/ClassicShell.vue";
import SidebarShell from "./layouts/SidebarShell.vue";
import ModernShell from "./layouts/ModernShell.vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const settings = useSettingsStore();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const modpack = useModpackStore();

setupAppearance();

// 布局在主题中心切换：经典（第一版，默认）/ 侧边栏 / 现代
const shell = computed(() => ({ classic: ClassicShell, sidebar: SidebarShell, modern: ModernShell })[currentLayout.value]);

onMounted(() => {
  settings.load();
  versions.loadInstalled();
  accounts.load();
  // 把整合包文件拖进窗口直接导入
  if (inTauri) {
    getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type !== "drop") return;
      const file = e.payload.paths.find((p) => /\.(mrpack|zip)$/i.test(p));
      if (file) modpack.importPath = file;
    });
  }
});
</script>

<template>
  <component :is="shell" />
  <ProgressFloat />
  <ModpackImportDialog v-if="modpack.importPath" :path="modpack.importPath" @close="modpack.importPath = null" />
</template>
