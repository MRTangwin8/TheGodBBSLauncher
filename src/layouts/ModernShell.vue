<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { useButtons } from "../buttons";
import { inTauri } from "../api";
import { isMac } from "../platform";
import { useVersionsStore } from "../stores/versions";
import { useAccountsStore } from "../stores/accounts";
import { useGameStore } from "../stores/game";
import ModernIcon from "../components/ModernIcon.vue";
import AccountAvatar from "../components/AccountAvatar.vue";
import WindowControls from "../components/WindowControls.vue";
import BackgroundLayer from "../components/BackgroundLayer.vue";

// 完全现代：图标导航条 + 透明标题栏 + 亚克力卡片
const route = useRoute();
const { nav } = useButtons();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const game = useGameStore();
const title = computed(() => (route.meta.title as string) ?? "");
</script>

<template>
  <div class="modern">
    <BackgroundLayer kind="mesh" />

    <aside class="rail">
      <div class="rail-drag" data-tauri-drag-region />
      <RouterLink to="/" class="brand" title="神权论坛启动器">神</RouterLink>
      <nav class="rail-nav">
        <RouterLink v-for="n in nav" :key="n.path" :to="n.path" class="rail-item">
          <ModernIcon :name="n.icon" :size="20" />
          <span class="rail-label">{{ n.name.length > 2 ? n.name.replace(/中心|管理|空间|游戏/, "") : n.name }}</span>
          <i v-if="n.path === '/log' && game.running.length" class="dot playing" />
          <i v-if="n.path === '/download' && versions.running.length" class="dot" />
        </RouterLink>
      </nav>
      <RouterLink to="/account" class="rail-me" :title="accounts.selected?.name ?? '登录'">
        <AccountAvatar :account="accounts.selected" :size="32" />
      </RouterLink>
    </aside>

    <div class="main">
      <header class="titlebar" data-tauri-drag-region>
        <span class="app-name" data-tauri-drag-region>神权论坛启动器</span>
        <span v-if="!inTauri" class="preview-hint">浏览器预览 · 假数据</span>
        <div class="space" data-tauri-drag-region />
        <WindowControls v-if="inTauri && !isMac" tone="auto" />
      </header>
      <main class="content">
        <h1 class="page-title">{{ title }}</h1>
        <RouterView v-slot="{ Component, route: r }">
          <Transition name="page" mode="out-in">
            <div :key="r.path" class="page"><component :is="Component" /></div>
          </Transition>
        </RouterView>
        <footer class="footer">与 MCBBS / Mojang Studios / Microsoft 无关 · 游戏文件来自官方源或镜像</footer>
      </main>
    </div>
  </div>
</template>

<style scoped>
.modern { display: flex; height: 100vh; }

.rail {
  width: 72px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  background: var(--rail-bg);
  border-right: 1px solid var(--border-soft);
  backdrop-filter: blur(24px) saturate(1.4);
}
.rail-drag { height: 12px; align-self: stretch; flex-shrink: 0; }
.brand {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  margin-bottom: 12px;
  border-radius: 12px;
  background: var(--grass);
  color: #fff;
  font-size: 20px;
  font-weight: 800;
  text-decoration: none;
  box-shadow: 0 4px 14px color-mix(in srgb, var(--grass) 45%, transparent);
}
.brand:hover { text-decoration: none; }
.rail-nav { flex: 1; display: flex; flex-direction: column; gap: 4px; overflow-y: auto; }
.rail-item {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  width: 58px;
  padding: 8px 0 6px;
  border-radius: 10px;
  color: var(--text-muted);
  text-decoration: none;
  transition: background 0.15s, color 0.15s;
}
.rail-item:hover { background: color-mix(in srgb, var(--text) 7%, transparent); color: var(--text); text-decoration: none; }
.rail-item.router-link-exact-active { background: color-mix(in srgb, var(--grass) 16%, transparent); color: var(--grass); }
.rail-item.router-link-exact-active::before {
  content: "";
  position: absolute;
  left: -7px;
  top: 14px;
  bottom: 14px;
  width: 3px;
  border-radius: 2px;
  background: var(--grass);
}
.rail-label { font-size: 11px; white-space: nowrap; }
.dot { position: absolute; top: 6px; right: 12px; width: 8px; height: 8px; border-radius: 50%; background: var(--gold); }
.dot.playing { background: var(--grass-light); }
.rail-me { margin: 10px 0 14px; border-radius: 50%; }

.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.titlebar { display: flex; align-items: center; gap: 10px; height: 36px; padding-left: 20px; flex-shrink: 0; user-select: none; }
.app-name { font-size: 12px; color: var(--text-muted); }
.space { flex: 1; align-self: stretch; }
.preview-hint { font-size: 11px; padding: 1px 8px; border-radius: 10px; background: var(--red); color: #fff; }
.content { flex: 1; overflow-y: auto; padding: 4px 32px 16px; }
.page-title { margin: 6px 0 16px; font-size: 28px; font-weight: 650; letter-spacing: 0.5px; color: var(--text); }
.footer { margin: 18px 0 8px; font-size: 11px; text-align: center; color: var(--text-muted); }
</style>
