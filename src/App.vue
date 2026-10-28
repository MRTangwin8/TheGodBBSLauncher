<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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
import OnboardingDialog from "./components/OnboardingDialog.vue";
import { checkForUpdate, pendingUpdate, skipUpdate } from "./update";
import { loadAnnouncements } from "./announcements";
import { isStore } from "./edition";
import { useLauncher } from "./composables/useLauncher";
import ClassicShell from "./layouts/ClassicShell.vue";
import SidebarShell from "./layouts/SidebarShell.vue";
import ModernShell from "./layouts/ModernShell.vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const settings = useSettingsStore();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const modpack = useModpackStore();
const launcher = useLauncher();
const onboardClosed = ref(false);

// Ctrl+Enter（macOS 是 ⌘+Enter）直接启动当前选中的版本。在输入框、弹窗里不触发，免得误启动
function onKey(e: KeyboardEvent) {
  if (e.key !== "Enter" || !(e.ctrlKey || e.metaKey) || e.repeat) return;
  if (settings.settings?.buttons?.launchShortcut === false) return;
  const el = e.target;
  if (el instanceof HTMLElement && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName))) return;
  if (document.querySelector('[role="dialog"], .mask')) return;
  if (launcher.runningRun.value || launcher.game.launching) return;
  e.preventDefault();
  launcher.launch();
}

setupAppearance();

// 布局在主题中心切换：经典（第一版，默认）/ 侧边栏 / 现代
const shell = computed(() => ({ classic: ClassicShell, sidebar: SidebarShell, modern: ModernShell })[currentLayout.value]);

onMounted(() => {
  window.addEventListener("keydown", onKey);
  // 设置读到之后，等界面稳定几秒再静默查一次更新，不拖慢启动
  settings.load().then(() => {
    // 商店版由商店负责更新
    if (!isStore && settings.settings?.autoCheckUpdate !== false) setTimeout(() => checkForUpdate(false), 4000);
    // 公告只在启动时拉一次，运行期间不再拉
    if (settings.settings?.announcementsEnabled !== false) loadAnnouncements();
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
  <OnboardingDialog v-if="settings.settings && !settings.settings.onboarded && !onboardClosed" @close="onboardClosed = true" />
  <UpdateDialog v-if="pendingUpdate && settings.settings?.onboarded" :info="pendingUpdate" @close="pendingUpdate = null" @skip="skipUpdate" />
</template>
