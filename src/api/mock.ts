// 浏览器预览用的假数据，只为看界面，不做任何真实下载或登录
import type { AccountList, InstalledVersion, Settings, VersionManifest } from "./types";

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

let settings: Settings = {
  gameDir: "C:\\Users\\Steve\\AppData\\Roaming\\GodBBSLauncher\\.minecraft",
  downloadSource: "auto",
  concurrency: 64,
  javaPath: null,
  memoryMb: null,
  jvmArgs: "",
  gameArgs: "",
  isolateVersions: true,
  windowWidth: null,
  windowHeight: null,
  crashRulesUrl: "",
};
const installed: InstalledVersion[] = [
  { id: "1.20.1", kind: "release", releaseTime: "2023-06-12T10:00:00+00:00", hasJar: true, loader: null },
  { id: "1.12.2", kind: "release", releaseTime: "2017-09-18T10:00:00+00:00", hasJar: true, loader: null },
];
const accounts: AccountList = {
  accounts: [
    { id: "a1", kind: "microsoft", name: "Notch", uuid: "069a79f444e94726a5befca90e38aaf5" },
    { id: "a2", kind: "offline", name: "Steve", uuid: "5627dd98e6be3c21b8a8e92344183641" },
    { id: "a3", kind: "yggdrasil", name: "Alex", uuid: "x", server: "https://littleskin.cn/api/yggdrasil", serverName: "LittleSkin" },
  ],
  selected: "a1",
};

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 200));
  const unsupported = () => {
    throw "浏览器预览模式不支持这个操作，请用 npm run tauri dev 启动";
  };
  const table: Record<string, () => unknown> = {
    get_settings: () => settings,
    save_settings: () => void (settings = args!.settings as Settings),
    probe_mirrors: () => [{ source: "official", millis: 812 }, { source: "bmclapi", millis: 96 }],
    get_version_manifest: () => manifest,
    list_installed_versions: () => installed,
    list_accounts: () => accounts,
    select_account: () => void (accounts.selected = args!.id as string),
    list_java: () => [
      { path: "C:\\Program Files\\Eclipse Adoptium\\jdk-21\\bin\\java.exe", major: 21, version: "21.0.4", arch: "x86_64", vendor: "Eclipse Adoptium" },
      { path: "C:\\Program Files\\Java\\jre1.8.0_421\\bin\\java.exe", major: 8, version: "1.8.0_421", arch: "x86_64", vendor: "Oracle Corporation" },
    ],
    list_loader_versions: () =>
      args!.kind === "quilt"
        ? []
        : [
            { id: "0.16.9", version: "0.16.9", stable: true },
            { id: "0.16.8", version: "0.16.8", stable: true },
            { id: "0.17.0-beta.1", version: "0.17.0-beta.1", stable: false },
          ],
    crash_rules_info: () => ({ version: 1, count: 25, custom: false }),
    analyze_log_text: () => ({
      exitCode: null,
      files: [],
      findings: [
        {
          id: "fabric-missing-dependency",
          title: "模组缺少前置：fabric-api",
          severity: "error",
          description: "模组「Sodium Extra」需要前置模组「fabric-api」，但没有安装。",
          solutions: ["去 Modrinth 或 CurseForge 下载「fabric-api」，放进这个版本的 mods 文件夹", "注意下载和游戏版本、加载器都对应的版本"],
          evidence: "- Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of fabric-api, which is missing!",
        },
        {
          id: "out-of-memory",
          title: "内存不足",
          severity: "error",
          description: "分配给游戏的内存不够用了（Java heap space）。",
          solutions: ["在「设置 → 最大内存」里调高分配"],
          evidence: "java.lang.OutOfMemoryError: Java heap space",
        },
      ],
    }),
    memory_info: () => ({ totalMb: 16384, autoMb: 4096 }),
    microsoft_login_start: () => ({ userCode: "GODBBS42", deviceCode: "", verificationUri: "https://www.microsoft.com/link", expiresIn: 900, interval: 5 }),
    microsoft_login_finish: () => new Promise(() => {}),
  };
  return (table[cmd] ?? unsupported)() as T;
}
