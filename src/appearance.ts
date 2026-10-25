// 外观：布局、配色方案、深浅色主题、外观细调和背景图
import { computed, ref, watch } from "vue";
import { api } from "./api";
import type { Tweaks } from "./api/types";
import { useSettingsStore } from "./stores/settings";
import { accentVars, paletteVars } from "./colors";

/** 当前背景图的 data URL；null 表示用布局自带的背景 */
export const backgroundUrl = ref<string | null>(null);

/** 拖进窗口、等待确认导入的主题文件 */
export const pendingThemeImport = ref<string | null>(null);

export const LAYOUTS = ["classic", "sidebar", "modern"] as const;
export type Layout = (typeof LAYOUTS)[number];
export const PALETTES = ["mcbbs", "grass", "nether", "end", "ocean", "cherry", "stone", "custom"] as const;

export const DEFAULT_TWEAKS: Tweaks = {
  radius: null,
  cardOpacity: null,
  cardBlur: null,
  bgBlur: 0,
  bgDim: 0,
  fontSize: 13,
  font: "",
  animations: true,
};

/** 和 mcbbs-classic.css 里 --font 的默认值一致，自定义字体找不到时回落到它 */
const FONT_FALLBACK = '"Microsoft YaHei UI", "Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", "Hiragino Sans GB", sans-serif';

/** 当前布局；设置还没读到时按默认的经典布局 */
export const currentLayout = computed<Layout>(() => {
  const l = useSettingsStore().settings?.layout as Layout | undefined;
  return l && LAYOUTS.includes(l) ? l : "classic";
});

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const systemIsDark = ref(systemDark.matches);
// 跟随系统时，系统切换深浅色立刻生效
systemDark.addEventListener("change", () => (systemIsDark.value = systemDark.matches));

export async function reloadBackground() {
  try {
    backgroundUrl.value = await api.backgroundImage();
  } catch {
    backgroundUrl.value = null;
  }
}

/** 自定义配色和细调都写成 <html> 上的内联 CSS 变量，优先级高于所有主题样式表 */
function tweakVars(t: Tweaks): Record<string, string> {
  const vars: Record<string, string> = {
    "--font-size": `${t.fontSize}px`,
    "--bg-blur": `${t.bgBlur}px`,
    "--bg-filter": t.bgBlur ? `blur(${t.bgBlur}px)` : "none",
    "--bg-dim": `${t.bgDim / 100}`,
  };
  if (t.radius !== null) {
    vars["--radius"] = `${t.radius}px`;
    vars["--radius-sm"] = `${Math.max(2, Math.round(t.radius / 2))}px`;
  }
  if (t.cardOpacity !== null) vars["--bg-box"] = `color-mix(in srgb, var(--bg-box-solid) ${t.cardOpacity}%, transparent)`;
  if (t.cardBlur !== null) vars["--blur"] = t.cardBlur ? `blur(${t.cardBlur}px) saturate(1.2)` : "none";
  if (t.font.trim()) vars["--font"] = `${/[,"']/.test(t.font) ? t.font : `"${t.font.trim()}"`}, ${FONT_FALLBACK}`;
  return vars;
}

export function setupAppearance() {
  const settings = useSettingsStore();
  const root = document.documentElement;

  const isDark = computed(() => {
    const theme = settings.settings?.theme;
    return theme === "dark" || (theme === "system" && systemIsDark.value);
  });
  watch(isDark, (d) => (root.dataset.theme = d ? "dark" : "light"), { immediate: true });
  watch(currentLayout, (l) => (root.dataset.layout = l), { immediate: true });

  const palette = computed(() => {
    const p = settings.settings?.palette;
    return p && (PALETTES as readonly string[]).includes(p) ? p : "mcbbs";
  });
  watch(palette, (p) => (root.dataset.palette = p), { immediate: true });

  let applied: string[] = [];
  watch(
    () => {
      const s = settings.settings;
      const tweaks = { ...DEFAULT_TWEAKS, ...s?.tweaks };
      let vars = tweakVars(tweaks);
      if (palette.value === "custom" && s?.customPalette) {
        // 现代布局的表面是中性色，只换强调色
        const colors = currentLayout.value === "modern" ? accentVars(s.customPalette, isDark.value) : paletteVars(s.customPalette, isDark.value);
        // 细调里的卡片透明度要基于自定义底色，所以放在后面覆盖
        vars = { ...colors, ...vars };
      }
      return { vars, motion: tweaks.animations };
    },
    ({ vars, motion }) => {
      for (const k of applied) if (!(k in vars)) root.style.removeProperty(k);
      for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
      applied = Object.keys(vars);
      if (motion) delete root.dataset.motion;
      else root.dataset.motion = "off";
    },
    { immediate: true, deep: true },
  );

  watch(() => settings.settings?.backgroundImage, reloadBackground);
}
