<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useButtons } from "../buttons";
import { inTauri } from "../api";
import { isMac } from "../platform";
import { useVersionsStore } from "../stores/versions";
import { useAccountsStore, kindLabel } from "../stores/accounts";
import { useGameStore } from "../stores/game";
import PixelIcon from "../components/PixelIcon.vue";
import AccountAvatar from "../components/AccountAvatar.vue";
import WindowControls from "../components/WindowControls.vue";
import BackgroundLayer from "../components/BackgroundLayer.vue";

// 第二版：木板侧边栏 + 像素风景
const route = useRoute();
const { nav } = useButtons();
const versions = useVersionsStore();
const accounts = useAccountsStore();
const game = useGameStore();
const title = computed(() => (route.meta.title as string) ?? "");

// 侧边栏可以收起成图标条，记在本地
const collapsed = ref(readCollapsed());
function readCollapsed() {
  try {
    return localStorage.getItem("sidebar-collapsed") === "1";
  } catch {
    return false;
  }
}
watch(collapsed, (v) => {
  try {
    localStorage.setItem("sidebar-collapsed", v ? "1" : "0");
  } catch {
    /* 存不了就算了 */
  }
});
</script>

<template>
  <div class="shell" :class="{ collapsed }">
    <BackgroundLayer kind="scene" />

    <aside class="sidebar">
      <!-- 和右侧标题栏同高的拖动条，logo 跟标题对齐 -->
      <div class="drag-strip" data-tauri-drag-region />
      <RouterLink to="/" class="logo" :title="collapsed ? '神权论坛' : undefined">
        <template v-if="!collapsed">
          <span class="logo-main">神权论坛</span>
          <span class="logo-sub">启动器 · MCBBS 纪念版</span>
        </template>
        <span v-else class="logo-main mini">神</span>
      </RouterLink>

      <nav class="nav">
        <RouterLink v-for="n in nav" :key="n.path" :to="n.path" class="nav-item" :title="collapsed ? n.name : undefined">
          <PixelIcon :name="n.icon" />
          <span class="label">{{ n.name }}</span>
          <span v-if="n.path === '/log' && game.running.length" class="badge playing" title="游戏运行中">{{ game.running.length }}</span>
          <span v-if="n.path === '/download' && versions.running.length" class="badge" title="任务进行中">{{ versions.running.length }}</span>
        </RouterLink>
      </nav>

      <button class="collapse" :title="collapsed ? '展开侧边栏' : '收起侧边栏'" @click="collapsed = !collapsed">
        {{ collapsed ? "»" : "« 收起" }}
      </button>

      <RouterLink to="/account" class="me" :title="collapsed ? accounts.selected?.name ?? '游客' : undefined">
        <AccountAvatar :account="accounts.selected" :size="34" />
        <div v-if="!collapsed" class="me-text">
          <b>{{ accounts.selected?.name ?? "游客" }}</b>
          <span v-if="accounts.selected">{{ kindLabel[accounts.selected.kind] }} · <span class="god">神权</span></span>
          <span v-else class="god">点我登录</span>
        </div>
      </RouterLink>
    </aside>

    <div class="main-col">
      <!-- 顶部 32px 拖动条放窗口按钮，标题另起一行，不再贴着窗口顶边 -->
      <div class="drag-strip top" data-tauri-drag-region>
        <span v-if="!inTauri" class="preview-hint">浏览器预览模式 · 数据是假的</span>
        <WindowControls v-if="inTauri && !isMac" class="win" />
      </div>
      <header class="title-row" data-tauri-drag-region>
        <h1 data-tauri-drag-region>{{ title }}</h1>
      </header>
      <main class="content">
        <!-- 套一层 div：有的页面有多个根节点（卡片 + 弹窗），Transition 没法直接给它们做动画，会卡住不切换 -->
        <RouterView v-slot="{ Component, route: r }">
          <Transition name="page" mode="out-in">
            <div :key="r.path" class="page">
              <component :is="Component" />
            </div>
          </Transition>
        </RouterView>
        <footer class="footer">
          神权论坛启动器 · 非官方粉丝纪念作品，与 MCBBS / Mojang Studios / Microsoft 无关 · 游戏文件全部来自官方源或镜像
        </footer>
      </main>
    </div>
  </div>
</template>

<style scoped>
.shell { position: relative; display: flex; height: 100vh; }

/* ---------- 侧边栏（木板） ---------- */
.sidebar {
  width: 200px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: linear-gradient(to right, var(--wood), var(--wood-dark));
  border-right: 3px solid var(--wood-dark);
  box-shadow: 2px 0 12px rgba(0, 0, 0, 0.25);
  color: #f6ead2;
  transition: width 0.18s ease;
}
.collapsed .sidebar { width: 64px; }
.drag-strip { height: 32px; flex-shrink: 0; }
.logo { display: flex; flex-direction: column; justify-content: center; height: 52px; padding: 0 16px 10px; text-decoration: none; border-bottom: 1px solid rgba(0, 0, 0, 0.25); white-space: nowrap; }
.collapsed .logo { align-items: center; padding: 0 0 10px; }
.logo:hover { text-decoration: none; }
.logo-main {
  font-size: 26px;
  line-height: 1.1;
  font-weight: 900;
  letter-spacing: 3px;
  color: #fff;
  text-shadow: 2px 2px 0 #2b1d10, -1px -1px 0 #2b1d10, 1px -1px 0 #2b1d10, -1px 1px 0 #2b1d10;
}
.logo-main.mini { letter-spacing: 0; }
.logo-sub { margin-top: 2px; font-size: 11px; color: #e8c98a; }

.nav { flex: 1; padding: 10px 8px; display: flex; flex-direction: column; gap: 2px; overflow-y: auto; overflow-x: hidden; }
.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 6px;
  color: #f1e3c4;
  font-size: 14px;
  white-space: nowrap;
  text-decoration: none;
  transition: background 0.15s, color 0.15s;
}
.collapsed .nav-item { justify-content: center; padding: 10px 0; }
.collapsed .nav-item .label { display: none; }
.nav-item:hover { background: rgba(255, 255, 255, 0.1); color: #fff; text-decoration: none; }
.nav-item.router-link-exact-active {
  background: var(--bg-box-solid);
  color: var(--brown-dark);
  font-weight: bold;
  box-shadow: inset 3px 0 0 var(--grass-light);
}
.badge { margin-left: auto; min-width: 18px; padding: 0 5px; border-radius: 9px; font-size: 11px; line-height: 18px; text-align: center; background: var(--gold); color: #3d2f1f; }
.badge.playing { background: var(--grass-light); }
.collapsed .badge { position: absolute; top: 2px; right: 6px; min-width: 14px; line-height: 14px; font-size: 10px; padding: 0 3px; }

.collapse {
  margin: 0 8px;
  padding: 4px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #e8c98a;
  font-size: 12px;
  cursor: pointer;
}
.collapse:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }

.me {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 8px;
  padding: 10px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: #f6ead2;
  text-decoration: none;
}
.collapsed .me { justify-content: center; padding: 8px 0; }
.me:hover { background: rgba(0, 0, 0, 0.3); text-decoration: none; }
.me-text { display: flex; flex-direction: column; min-width: 0; font-size: 12px; }
.me-text b { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.god { color: var(--gold); font-weight: bold; }

/* ---------- 内容区 ---------- */
.main-col { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.drag-strip.top { display: flex; align-items: center; justify-content: flex-end; gap: 8px; user-select: none; }
.win { align-self: stretch; }
.title-row { display: flex; align-items: center; height: 52px; padding: 0 24px 10px; flex-shrink: 0; user-select: none; }
.title-row h1 { font-size: 22px; color: #fff; text-shadow: 1px 1px 0 rgba(43, 29, 16, 0.8), 0 0 8px rgba(43, 29, 16, 0.4); }
.preview-hint { margin-right: 12px; font-size: 12px; padding: 2px 8px; border-radius: 10px; background: rgba(192, 57, 43, 0.85); color: #fff; }
.content { flex: 1; overflow-y: auto; padding: 0 24px 16px; }
/* 底部声明：放在半透明深色胶囊里，压在天空和泥土上都看得清 */
.footer {
  width: fit-content;
  max-width: 100%;
  margin: 14px auto 70px;
  padding: 4px 14px;
  border-radius: 12px;
  font-size: 11px;
  text-align: center;
  color: rgba(255, 255, 255, 0.9);
  background: rgba(43, 29, 16, 0.45);
  backdrop-filter: blur(4px);
}
</style>
