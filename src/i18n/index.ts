// 界面语言：简体中文（原文）/ English / 日本語。
// 设置里的 language = "auto" 时按系统语言选：中文（含繁体）→ 中文，日语 → 日文，其余一律英文。
import { computed, watch } from "vue";
import { createI18n } from "vue-i18n";
import zhCN from "./zh-CN";
import en from "./en";
import ja from "./ja";
import { useSettingsStore } from "../stores/settings";

export const LOCALES = ["zh-CN", "en", "ja"] as const;
export type Locale = (typeof LOCALES)[number];
export type LanguagePref = "auto" | Locale;

/** 各语言用自己的文字写名字，不随界面语言变 */
export const LOCALE_NAMES: Record<Locale, string> = { "zh-CN": "简体中文", en: "English", ja: "日本語" };

/** 系统语言标签（navigator.languages 的第一项）→ 界面语言 */
export function pickLocale(tags: readonly string[]): Locale {
  const tag = (tags[0] ?? "").toLowerCase();
  if (tag === "zh" || tag.startsWith("zh-")) return "zh-CN";
  if (tag === "ja" || tag.startsWith("ja-")) return "ja";
  return "en";
}

export function systemLocale(): Locale {
  return pickLocale(navigator.languages?.length ? navigator.languages : [navigator.language]);
}

export function resolveLocale(pref: string | null | undefined): Locale {
  return (LOCALES as readonly string[]).includes(pref ?? "") ? (pref as Locale) : systemLocale();
}

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  // 设置读到之前先按系统语言显示，免得英文系统上先闪一下中文
  locale: systemLocale(),
  fallbackLocale: "zh-CN",
  messages: { "zh-CN": zhCN, en, ja },
  missingWarn: false,
  fallbackWarn: false,
});

/** 在 .ts 文件里用（模板里直接用 $t） */
export const t = i18n.global.t;

/**
 * 后端错误 → 当前语言的文字。
 * 常见错误（启动、安装、登录）后端返回 JSON：{"i18n":"err.xxx","args":{...},"text":"中文原文"}，见 src-tauri/src/engine/error.rs；
 * 语言包里有这个 key 就翻译，没有就显示 text。其余错误还是中文字符串，原样显示。
 */
export function errText(e: unknown): string {
  let s = e instanceof Error ? e.message : String(e ?? "");
  if (s.startsWith("Error: ")) s = s.slice(7);
  if (!s.startsWith('{"i18n":')) return s;
  try {
    const { i18n: key, args, text } = JSON.parse(s) as { i18n: string; args?: Record<string, unknown>; text?: string };
    return i18n.global.te(key) || i18n.global.te(key, "zh-CN") ? t(key, args ?? {}) : (text ?? s);
  } catch {
    return s;
  }
}

/** 当前界面语言，组件里拿来格式化日期等 */
export const locale = computed(() => i18n.global.locale.value as Locale);

/** 引导页角落的语言选择：选了立刻生效，设置还没读到时先只切界面 */
export function setLocale(l: Locale) {
  i18n.global.locale.value = l;
}

export function setupI18n() {
  const settings = useSettingsStore();
  watch(
    () => resolveLocale(settings.settings?.language),
    (l) => {
      i18n.global.locale.value = l;
      document.documentElement.lang = l;
      document.title = t("app.name");
    },
    { immediate: true },
  );
}
