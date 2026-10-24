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
  return isNaN(d.getTime()) ? iso : d.toLocaleDateString("zh-CN");
}

export const kindLabel: Record<string, string> = {
  release: "正式版",
  snapshot: "快照",
  old_beta: "远古 Beta",
  old_alpha: "远古 Alpha",
};

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
  if (secs < 60) return secs > 0 ? "不到 1 分钟" : "0 分钟";
  if (secs < 3600) return `${Math.round(secs / 60)} 分钟`;
  const h = secs / 3600;
  return `${h >= 100 ? Math.round(h) : h.toFixed(1)} 小时`;
}

/** unix 秒 → "3 小时前" / "昨天" / "2026/10/1" */
export function timeAgo(unix: number | null | undefined): string {
  if (!unix) return "从没玩过";
  const diff = Date.now() / 1000 - unix;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 2) return "昨天";
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)} 天前`;
  return new Date(unix * 1000).toLocaleDateString("zh-CN");
}
