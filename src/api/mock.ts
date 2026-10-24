// 浏览器预览用的假数据，只为看界面，不做任何真实下载
import type { InstalledVersion, Settings, VersionManifest } from "./types";

const ids: [string, VersionManifest["versions"][number]["type"], string][] = [
  ["24w44a", "snapshot", "2024-10-30"],
  ["1.21.3", "release", "2024-10-23"],
  ["1.21.1", "release", "2024-08-08"],
  ["1.20.6", "release", "2024-04-29"],
  ["1.20.1", "release", "2023-06-12"],
  ["1.16.5", "release", "2021-01-14"],
  ["1.12.2", "release", "2017-09-18"],
  ["1.8.9", "release", "2015-12-03"],
  ["b1.7.3", "old_beta", "2011-07-07"],
  ["a1.2.6", "old_alpha", "2010-12-02"],
];

const manifest: VersionManifest = {
  latest: { release: "1.21.3", snapshot: "24w44a" },
  versions: ids.map(([id, type, d]) => ({ id, type, url: "", time: d, releaseTime: `${d}T10:00:00+00:00` })),
};

let settings: Settings = { gameDir: "C:\Users\Steve\AppData\Roaming\GodBBSLauncher\.minecraft", downloadSource: "auto", concurrency: 64 };
const installed: InstalledVersion[] = [
  { id: "1.20.1", kind: "release", releaseTime: "2023-06-12T10:00:00+00:00", hasJar: true },
  { id: "1.12.2", kind: "release", releaseTime: "2017-09-18T10:00:00+00:00", hasJar: true },
];

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 200));
  const table: Record<string, () => unknown> = {
    get_settings: () => settings,
    save_settings: () => void (settings = args!.settings as Settings),
    probe_mirrors: () => [{ source: "official", millis: 812 }, { source: "bmclapi", millis: 96 }],
    get_version_manifest: () => manifest,
    list_installed_versions: () => installed,
    install_version: () => {
      throw "浏览器预览模式不能真的下载，请用 npm run tauri dev 启动";
    },
  };
  return table[cmd]() as T;
}
