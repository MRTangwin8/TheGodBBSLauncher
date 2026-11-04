import { locale, t } from "./i18n";

/**
 * 二次确认。所有确认框都要用它，不要直接用 confirm()：
 * Tauri 的对话框插件把 window.confirm 换成了返回 Promise 的异步版本，
 * `if (!confirm(...))` 拿到的 Promise 永远是真值，用户还没点就已经继续执行了。
 */
export async function ask(message: string): Promise<boolean> {
  return Boolean(await (window.confirm(message) as boolean | Promise<boolean>));
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(iso?: string): string {
  if (!iso) return "-";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleDateString(locale.value);
}

/** 版本类型的显示名；没有类型的算"自定义"，不认识的原样显示 */
export function kindLabel(kind?: string | null): string {
  if (!kind) return t("versionKind.custom");
  return ["release", "snapshot", "old_beta", "old_alpha"].includes(kind) ? t(`versionKind.${kind}`) : kind;
}

/** 和后端 default_version_name 保持一致：<MC>-<加载器>-<加载器版本> */
export function defaultVersionName(mc: string, kind: string, loaderId: string): string {
  let short = loaderId.startsWith(`${mc}-`) ? loaderId.slice(mc.length + 1) : loaderId;
  if (short.endsWith(`-${mc}`)) short = short.slice(0, -(mc.length + 1));
  return `${mc}-${loaderLabel[kind]}-${short}`;
}

export const loaderLabel: Record<string, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
};

/** 秒 → "3.5 小时" / "25 分钟" */
export function formatDuration(secs: number): string {
  if (secs < 60) return secs > 0 ? t("time.lessThanMinute") : t("time.zeroMinutes");
  if (secs < 3600) return t("time.minutes", { n: Math.round(secs / 60) });
  const h = secs / 3600;
  return t("time.hours", { n: h >= 100 ? Math.round(h) : h.toFixed(1) });
}

/** unix 秒 → "3 小时前" / "昨天" / "2026/10/1" */
export function timeAgo(unix: number | null | undefined): string {
  if (!unix) return t("time.never");
  const diff = Date.now() / 1000 - unix;
  if (diff < 60) return t("time.justNow");
  if (diff < 3600) return t("time.minutesAgo", { n: Math.floor(diff / 60) });
  if (diff < 86400) return t("time.hoursAgo", { n: Math.floor(diff / 3600) });
  if (diff < 86400 * 2) return t("time.yesterday");
  if (diff < 86400 * 30) return t("time.daysAgo", { n: Math.floor(diff / 86400) });
  return new Date(unix * 1000).toLocaleDateString(locale.value);
}

/** 资源版本类型：正式 / 测试 / 开发 */
export function releaseType(rt: string): string {
  return ["release", "beta", "alpha"].includes(rt) ? t(`releaseType.${rt}`) : rt;
}
