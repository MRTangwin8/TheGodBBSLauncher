import { createRouter, createWebHashHistory } from "vue-router";

export const navItems = [
  { path: "/", name: "首页", component: () => import("./views/HomeView.vue") },
  { path: "/download", name: "下载中心", component: () => import("./views/DownloadView.vue") },
  { path: "/versions", name: "版本管理", component: () => import("./views/VersionsView.vue") },
  { path: "/log", name: "游戏日志", component: () => import("./views/LogView.vue") },
  { path: "/account", name: "个人空间", component: () => import("./views/AccountView.vue") },
  { path: "/settings", name: "设置", component: () => import("./views/SettingsView.vue") },
];

export default createRouter({
  history: createWebHashHistory(),
  routes: navItems.map((n) => ({ path: n.path, component: n.component, meta: { title: n.name } })),
});
