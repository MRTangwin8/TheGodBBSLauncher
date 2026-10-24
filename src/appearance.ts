// 外观：布局、配色方案、深浅色主题和背景图
import { computed, ref, watch } from "vue";
import { api } from "./api";
import { useSettingsStore } from "./stores/settings";

/** 当前背景图的 data URL；null 表示用布局自带的背景 */
export const backgroundUrl = ref<string | null>(null);

export const LAYOUTS = ["classic", "sidebar", "modern"] as const;
export type Layout = (typeof LAYOUTS)[number];
export const PALETTES = ["mcbbs", "grass", "nether", "end", "ocean", "cherry", "stone"] as const;

/** 当前布局；设置还没读到时按默认的经典布局 */
export const currentLayout = computed<Layout>(() => {
  const l = useSettingsStore().settings?.layout as Layout | undefined;
  return l && LAYOUTS.includes(l) ? l : "classic";
});

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

function applyTheme(theme: string | undefined) {
  const dark = theme === "dark" || (theme === "system" && systemDark.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

export async function reloadBackground() {
  try {
    backgroundUrl.value = await api.backgroundImage();
  } catch {
    backgroundUrl.value = null;
  }
}

export function setupAppearance() {
  const settings = useSettingsStore();
  const root = document.documentElement;
  watch(() => settings.settings?.theme, applyTheme, { immediate: true });
  watch(currentLayout, (l) => (root.dataset.layout = l), { immediate: true });
  watch(
    () => settings.settings?.palette,
    (p) => (root.dataset.palette = p && (PALETTES as readonly string[]).includes(p) ? p : "mcbbs"),
    { immediate: true },
  );
  watch(() => settings.settings?.backgroundImage, reloadBackground);
  // 跟随系统时，系统切换深浅色立刻生效
  systemDark.addEventListener("change", () => applyTheme(settings.settings?.theme));
}
