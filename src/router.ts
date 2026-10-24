import { createRouter, createWebHashHistory } from "vue-router";

export const navItems = [
  { path: "/", name: "首页", icon: "home", component: () => import("./views/HomeView.vue") },
  { path: "/download", name: "下载中心", icon: "download", component: () => import("./views/DownloadView.vue") },
  { path: "/versions", name: "版本管理", icon: "versions", component: () => import("./views/VersionsView.vue") },
  { path: "/resources", name: "资源中心", icon: "resources", component: () => import("./views/ResourcesView.vue") },
  { path: "/log", name: "游戏日志", icon: "log", component: () => import("./views/LogView.vue") },
  { path: "/account", name: "个人空间", icon: "user", component: () => import("./views/AccountView.vue") },
  { path: "/theme", name: "主题中心", icon: "palette", component: () => import("./views/ThemeView.vue") },
  { path: "/settings", name: "设置", icon: "settings", component: () => import("./views/SettingsView.vue") },
];

export default createRouter({
  history: createWebHashHistory(),
  routes: navItems.map((n) => ({ path: n.path, component: n.component, meta: { title: n.name } })),
});
