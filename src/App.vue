<script setup lang="ts">
import { computed, onMounted } from "vue";
import { inTauri } from "./api";
import { useSettingsStore } from "./stores/settings";
import { useVersionsStore } from "./stores/versions";
import { useAccountsStore } from "./stores/accounts";
import { useModpackStore } from "./stores/modpack";
import { currentLayout, pendingThemeImport, setupAppearance } from "./appearance";
import ProgressFloat from "./components/ProgressFloat.vue";
import ModpackImportDialog from "./components/ModpackImportDialog.vue";
import ThemeImportDialog from "./components/ThemeImportDialog.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import { checkForUpdate, pendingUpdate, skipUpdate } from "./update";
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
  // 设置读到之后，等界面稳定几秒再静默查一次更新，不拖慢启动
  settings.load().then(() => {
    if (settings.settings?.autoCheckUpdate !== false) setTimeout(() => checkForUpdate(false), 4000);
  });
  versions.loadInstalled();
  accounts.load();
  // 把整合包或主题文件拖进窗口直接导入
  if (inTauri) {
    getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type !== "drop") return;
      const theme = e.payload.paths.find((p) => /\.godtheme$/i.test(p));
      if (theme) {
        pendingThemeImport.value = theme;
        return;
      }
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
  <ThemeImportDialog v-if="pendingThemeImport" :path="pendingThemeImport" @close="pendingThemeImport = null" />
  <UpdateDialog v-if="pendingUpdate" :info="pendingUpdate" @close="pendingUpdate = null" @skip="skipUpdate" />
</template>
