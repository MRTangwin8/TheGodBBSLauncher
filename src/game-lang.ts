// 第一次启动某个版本时给游戏设的语言（写进 options.txt 的 lang），纯逻辑，方便单测。
// 用户在启动器里手动选了和系统不同的界面语言，就跟启动器走；否则按系统语言，能分出繁体中文、英式英语这些。
// 只在游戏目录里还没有 options.txt 时生效（后端判断），不会改掉玩家在游戏里自己选的语言。

const UI_TO_GAME: Record<string, string> = { "zh-CN": "zh_cn", en: "en_us", ja: "ja_jp" };

/** 只写语言、不带地区时的默认地区 */
const BARE: Record<string, string> = {
  zh: "zh_cn", ja: "ja_jp", en: "en_us", ko: "ko_kr", de: "de_de", fr: "fr_fr", es: "es_es",
  ru: "ru_ru", pt: "pt_br", it: "it_it", pl: "pl_pl", tr: "tr_tr", uk: "uk_ua", vi: "vi_vn", th: "th_th",
};

function uiOf(tag: string): string {
  if (tag === "zh" || tag.startsWith("zh-")) return "zh-CN";
  if (tag === "ja" || tag.startsWith("ja-")) return "ja";
  return "en";
}

function fromSystem(tag: string): string | null {
  const parts = tag.split(/[-_]/);
  const lang = parts[0];
  if (lang === "zh") {
    if (parts.includes("hk") || parts.includes("mo")) return "zh_hk";
    if (parts.includes("hant") || parts.includes("tw")) return "zh_tw";
    return "zh_cn";
  }
  const region = parts.slice(1).find((p) => /^[a-z]{2}$/.test(p));
  return region ? `${lang}_${region}` : (BARE[lang] ?? null);
}

/** uiLocale：启动器当前界面语言；systemTags：navigator.languages */
export function minecraftLang(uiLocale: string, systemTags: readonly string[]): string {
  const fallback = UI_TO_GAME[uiLocale] ?? "en_us";
  const tag = (systemTags[0] ?? "").toLowerCase();
  if (!tag || uiOf(tag) !== uiLocale) return fallback;
  const code = fromSystem(tag);
  return code && /^[a-z]{2,3}_[a-z]{2,3}$/.test(code) ? code : fallback;
}
