<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useRoute } from "vue-router";
import { navItems } from "./router";
import { inTauri } from "./api";
import { useSettingsStore } from "./stores/settings";
import { useVersionsStore } from "./stores/versions";
import { useAccountsStore } from "./stores/accounts";
import { useGameStore } from "./stores/game";
import ProgressFloat from "./components/ProgressFloat.vue";
import ModpackImportDialog from "./components/ModpackImportDialog.vue";
import { useModpackStore } from "./stores/modpack";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const route = useRoute();
const settings = useSettingsStore();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const game = useGameStore();
const modpack = useModpackStore();
const title = computed(() => (route.meta.title as string) ?? "");

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
  <div class="page">
    <header class="banner">
      <div class="logo">
        <span class="logo-main">神权论坛</span>
        <span class="logo-sub">启动器 · MCBBS 纪念版</span>
      </div>
      <div class="banner-user">
        <span v-if="accounts.selected">欢迎回来，<b>{{ accounts.selected.name }}</b></span>
        <span v-else>游客，<RouterLink to="/account" class="login-link">登录</RouterLink></span>
        <span class="sep">|</span>
        <span>用户组：<span class="group-god">神权</span></span>
      </div>
    </header>

    <nav class="nav">
      <RouterLink v-for="n in navItems" :key="n.path" :to="n.path" class="nav-item">{{ n.name }}</RouterLink>
      <span class="nav-right">
        <RouterLink v-if="game.running.length" to="/log" class="nav-playing">🎮 {{ game.running.length }} 个游戏运行中</RouterLink>
        <template v-if="versions.running.length">⏳ {{ versions.running.length }} 个任务进行中</template>
      </span>
    </nav>

    <div class="crumb">
      <span>⌂</span> <RouterLink to="/">神权论坛</RouterLink> <span class="muted">›</span> {{ title }}
    </div>

    <div v-if="!inTauri" class="preview-hint mod-tip">
      【版主提示】当前是浏览器预览模式，数据都是假的。完整功能请用 <code>npm run tauri dev</code> 启动。
    </div>

    <main class="main">
      <RouterView />
    </main>

    <footer class="footer">
      <p>神权论坛启动器 · 非官方粉丝纪念作品，与 MCBBS / Mojang Studios / Microsoft 无关</p>
      <p class="muted">Minecraft 游戏文件全部从官方源或 BMCLAPI 镜像下载，本程序不分发任何游戏文件</p>
    </footer>

    <ProgressFloat />
    <ModpackImportDialog v-if="modpack.importPath" :path="modpack.importPath" @close="modpack.importPath = null" />
  </div>
</template>

<style scoped>
.page { max-width: 1120px; margin: 0 auto; padding: 0 12px 24px; }

.banner {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  height: 96px;
  padding: 0 16px 12px;
  margin: 0 -12px;
  /* 天空 → 草地 → 泥土，纯 CSS 绘制 */
  background:
    linear-gradient(to bottom, transparent 70%, var(--grass) 70%, var(--grass) 80%, var(--brown) 80%),
    linear-gradient(to bottom, #8ec5f0, #cfe9fb);
  border-bottom: 3px solid var(--brown-dark);
}
.logo { display: flex; align-items: baseline; gap: 10px; padding-bottom: 14px; }
.logo-main {
  font-size: 34px;
  font-weight: 900;
  letter-spacing: 4px;
  color: #fff;
  text-shadow: 3px 3px 0 var(--brown-dark), -1px -1px 0 var(--brown-dark), 1px -1px 0 var(--brown-dark), -1px 1px 0 var(--brown-dark);
}
.logo-sub { font-size: 13px; color: var(--brown-dark); font-weight: bold; text-shadow: 0 1px 0 #fff; }
.banner-user { color: #fff; font-size: 12px; text-shadow: 1px 1px 0 var(--brown-dark); padding-bottom: 2px; }
.banner-user .sep { margin: 0 6px; opacity: 0.6; }
.group-god { color: var(--gold); font-weight: bold; }
.login-link { color: var(--gold); font-weight: bold; }
.nav-playing { color: #c8f59a; margin-right: 10px; }

.nav {
  display: flex;
  align-items: stretch;
  background: var(--nav-bg);
  border: 1px solid var(--brown-dark);
  border-top: none;
  margin: 0 -12px;
}
.nav-item {
  padding: 9px 18px;
  color: var(--nav-text);
  font-weight: bold;
  border-right: 1px solid rgba(0, 0, 0, 0.15);
}
.nav-item:hover { background: var(--nav-bg-hover); color: #fff; text-decoration: none; }
.nav-item.router-link-exact-active { background: var(--bg-box); color: var(--brown-dark); }
.nav-right { margin-left: auto; align-self: center; padding: 0 12px; color: var(--nav-text); font-size: 12px; }

.crumb { margin: 10px 0; color: var(--text-muted); }
.preview-hint { margin-bottom: 10px; }
.footer { text-align: center; font-size: 12px; margin-top: 20px; color: var(--text-muted); }
.footer p { margin: 4px 0; }
</style>
