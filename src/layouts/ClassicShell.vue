<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { useButtons } from "../buttons";
import { inTauri } from "../api";
import { isMac } from "../platform";
import { useVersionsStore } from "../stores/versions";
import { useAccountsStore } from "../stores/accounts";
import { useGameStore } from "../stores/game";
import BackgroundLayer from "../components/BackgroundLayer.vue";
import WindowControls from "../components/WindowControls.vue";

// 第一版：论坛横幅 + 顶部导航 + 面包屑。标题栏融进横幅（横幅本身可拖动窗口，右上角是窗口按钮）
const route = useRoute();
const { nav } = useButtons();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const game = useGameStore();
const title = computed(() => (route.meta.title as string) ?? "");
</script>

<template>
  <div class="classic">
    <BackgroundLayer kind="checker" />
    <header class="banner" data-tauri-drag-region>
      <div class="logo" data-tauri-drag-region>
        <span class="logo-main" data-tauri-drag-region>神权论坛</span>
        <span class="logo-sub" data-tauri-drag-region>启动器 · MCBBS 纪念版</span>
      </div>
      <div class="banner-user">
        <span v-if="accounts.selected">欢迎回来，<b>{{ accounts.selected.name }}</b></span>
        <span v-else>游客，<RouterLink to="/account" class="login-link">登录</RouterLink></span>
        <span class="sep">|</span>
        <span>用户组：<span class="group-god">神权</span></span>
      </div>
      <WindowControls v-if="inTauri && !isMac" class="win" />
    </header>

    <nav class="nav">
      <RouterLink v-for="n in nav" :key="n.path" :to="n.path" class="nav-item">{{ n.name }}</RouterLink>
      <span class="nav-right">
        <RouterLink v-if="game.running.length" to="/log" class="nav-playing">🎮 {{ game.running.length }} 个游戏运行中</RouterLink>
        <template v-if="versions.running.length">⏳ {{ versions.running.length }} 个任务进行中</template>
      </span>
    </nav>

    <div class="scroll">
      <div class="page-wrap">
        <div class="crumb">
          <span>⌂</span> <RouterLink to="/">神权论坛</RouterLink> <span class="muted">›</span> {{ title }}
          <span v-if="!inTauri" class="preview-hint">浏览器预览模式 · 数据是假的</span>
        </div>
        <RouterView v-slot="{ Component, route: r }">
          <Transition name="page" mode="out-in">
            <div :key="r.path" class="page"><component :is="Component" /></div>
          </Transition>
        </RouterView>
        <footer class="footer">
          <p>神权论坛启动器 · 非官方粉丝纪念作品，与 MCBBS / Mojang Studios / Microsoft 无关</p>
          <p class="muted">Minecraft 游戏文件全部从官方源或 BMCLAPI 镜像下载，本程序不分发任何游戏文件</p>
        </footer>
      </div>
    </div>
  </div>
</template>

<style scoped>
.classic { display: flex; flex-direction: column; height: 100vh; }

.banner {
  position: relative;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  height: 96px;
  flex-shrink: 0;
  padding: 0 28px 12px;
  /* 天空 → 草地 → "木头"，纯 CSS 绘制，跟随配色方案 */
  background:
    linear-gradient(to bottom, transparent 70%, var(--grass-top) 70%, var(--grass-top) 80%, var(--wood) 80%),
    linear-gradient(to bottom, var(--sky-top), var(--sky-bottom));
  border-bottom: 3px solid var(--wood-dark);
  user-select: none;
}
.logo { display: flex; align-items: baseline; gap: 10px; padding-bottom: 14px; }
.logo-main {
  font-size: 34px;
  font-weight: 900;
  letter-spacing: 4px;
  color: #fff;
  text-shadow: 3px 3px 0 var(--wood-dark), -1px -1px 0 var(--wood-dark), 1px -1px 0 var(--wood-dark), -1px 1px 0 var(--wood-dark);
}
.logo-sub { font-size: 13px; color: #fff; font-weight: bold; text-shadow: 1px 1px 0 var(--wood-dark); }
.banner-user { color: #fff; font-size: 12px; text-shadow: 1px 1px 0 var(--wood-dark); padding-bottom: 2px; }
.banner-user .sep { margin: 0 6px; opacity: 0.6; }
.group-god, .login-link { color: var(--gold); font-weight: bold; }
.win { position: absolute; top: 0; right: 0; height: 32px; }

.nav { display: flex; flex-wrap: wrap; flex-shrink: 0; background: var(--wood-light); border-bottom: 1px solid var(--wood-dark); padding: 0 16px; }
.nav-item { padding: 9px 16px; color: #fff7e6; font-weight: bold; border-right: 1px solid rgba(0, 0, 0, 0.15); white-space: nowrap; }
.nav-item:first-child { border-left: 1px solid rgba(0, 0, 0, 0.15); }
.nav-item:hover { background: rgba(255, 255, 255, 0.12); color: #fff; text-decoration: none; }
.nav-item.router-link-exact-active { background: var(--bg-box-solid); color: var(--brown-dark); }
.nav-right { margin-left: auto; align-self: center; padding: 0 4px; color: #fff7e6; font-size: 12px; white-space: nowrap; }
.nav-playing { color: #c8f59a; margin-right: 10px; }

.scroll { flex: 1; overflow-y: auto; }
.page-wrap { max-width: 1120px; margin: 0 auto; padding: 0 16px 24px; }
.crumb { display: flex; align-items: center; gap: 4px; margin: 10px 0; color: var(--text-muted); }
.preview-hint { margin-left: auto; font-size: 12px; padding: 1px 8px; border-radius: 10px; background: var(--red); color: #fff; }
.footer { text-align: center; font-size: 12px; margin-top: 20px; color: var(--text-muted); }
.footer p { margin: 4px 0; }
@media (max-width: 680px) {
  .banner { height: auto; min-height: 112px; padding: 14px 16px 12px; align-items: flex-start; flex-direction: column; gap: 6px; }
  .logo { flex-wrap: wrap; gap: 6px; padding-bottom: 0; }
  .logo-main { font-size: 28px; letter-spacing: 0; }
  .banner-user { padding-bottom: 0; }
  .nav { padding: 0 8px; }
  .nav-item { padding: 9px 12px; }
  .nav-right { white-space: normal; }
}
</style>
