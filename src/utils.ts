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
