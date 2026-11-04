import { createRouter, createWebHashHistory } from "vue-router";
import { t } from "./i18n";
import { isStore } from "./edition";

const nav = (path: string, key: string, icon: string, component: () => Promise<unknown>) => ({
  path,
  key,
  icon,
  component,
  /** 导航名，跟着界面语言变 */
  get name() {
    return t(`nav.${key}`);
  },
  /** 现代布局图标条下面的短名 */
  get short() {
    return t(`navShort.${key}`);
  },
});

export const navItems = [
  nav("/", "home", "home", () => import("./views/HomeView.vue")),
  nav("/download", "download", "download", () => import("./views/DownloadView.vue")),
  nav("/versions", "versions", "versions", () => import("./views/VersionsView.vue")),
  nav("/resources", "resources", "resources", () => import("./views/ResourcesView.vue")),
  // 联机用陶瓦联机；商店版暂不提供（见 docs/MULTIPLAYER.md）
  ...(isStore ? [] : [nav("/multiplayer", "multiplayer", "multiplayer", () => import("./views/MultiplayerView.vue"))]),
  nav("/log", "log", "log", () => import("./views/LogView.vue")),
  nav("/account", "account", "user", () => import("./views/AccountView.vue")),
  nav("/theme", "theme", "palette", () => import("./views/ThemeView.vue")),
  nav("/settings", "settings", "settings", () => import("./views/SettingsView.vue")),
];

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    // meta.title 是语言包里的 key，布局显示时再翻译
    ...navItems.map((n) => ({ path: n.path, component: n.component, meta: { title: `nav.${n.key}` } })),
    // 不在导航栏里的子页面
    { path: "/versions/:id", component: () => import("./views/InstanceView.vue"), meta: { title: "nav.versions" } },
  ],
});
