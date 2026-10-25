import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "./api";

export type CommunitySite = "memorialResources" | "redstone";

export const communitySites: { id: CommunitySite; name: string; url: string; category: string }[] = [
  { id: "memorialResources", name: "MCBBS 纪念版资源中心", url: "https://www.mcbbs.co/portal.php?v3page=resources&type=modpack", category: "整合包 · 社区发布" },
  { id: "redstone", name: "红石中继站", url: "https://www.mczwlt.net/", category: "模组 · 整合包 · 资源包 · 光影" },
];

export async function browseCommunity(site: CommunitySite) {
  const entry = communitySites.find((entry) => entry.id === site);
  if (!entry) throw new Error("未知社区");
  if (inTauri) await openUrl(entry.url);
  else window.open(entry.url, "_blank", "noopener,noreferrer");
}
