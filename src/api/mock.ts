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
  curseforgeApiKey: "",
  layout: "classic",
  palette: "mcbbs",
  theme: "light",
  backgroundImage: null,
  lastVersion: "fabric-loader-0.16.9-1.21.1",
};
const installed: InstalledVersion[] = [
  { id: "fabric-loader-0.16.9-1.21.1", kind: "release", releaseTime: "2024-08-08T10:00:00+00:00", hasJar: true, loader: "fabric", inheritsFrom: "1.21.1", mcVersion: "1.21.1", playTimeSecs: 45000, lastPlayed: 1792850000, modpack: null },
  { id: "1.20.1", kind: "release", releaseTime: "2023-06-12T10:00:00+00:00", hasJar: true, loader: null, mcVersion: null, playTimeSecs: 3600, lastPlayed: 1792700000, modpack: null },
  { id: "1.12.2", kind: "release", releaseTime: "2017-09-18T10:00:00+00:00", hasJar: true, loader: null, mcVersion: null, playTimeSecs: 3600, lastPlayed: 1792700000, modpack: null },
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
    background_image: () => null,
    account_skin: () => null,
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
    resource_target: () => ({ mcVersion: "1.21.1", loader: "fabric", dir: "C:/mc/versions/fabric-loader-0.16.9-1.21.1/mods", curseforgeViaMirror: true }),
    resource_search: () => ({
      total: 2,
      hits: [
        { source: "modrinth", id: "PtjYWJkn", slug: "sodium-extra", title: "Sodium Extra", description: "A Sodium addon that adds features that shouldn't be in Sodium.", author: "FlashyReese", iconUrl: null, downloads: 29000000, categories: ["optimization", "utility"], updated: "2026-09-01T00:00:00Z", pageUrl: "https://modrinth.com/mod/sodium-extra" },
        { source: "modrinth", id: "AANobbMI", slug: "sodium", title: "Sodium", description: "The fastest rendering optimization mod for Minecraft.", author: "jellysquid3", iconUrl: null, downloads: 80000000, categories: ["optimization"], updated: "2026-10-01T00:00:00Z", pageUrl: "https://modrinth.com/mod/sodium" },
      ],
    }),
    resource_versions: () => [
      { source: "modrinth", id: "v1", projectId: "PtjYWJkn", name: "0.9.4", versionNumber: "mc1.21.1-0.9.4+fabric", gameVersions: ["1.21.1"], loaders: ["fabric"], published: "2026-09-01T00:00:00Z", releaseType: "release", file: { url: "", filename: "sodium-extra-0.9.4.jar", sha1: null, size: 1024000 }, dependencies: [] },
      { source: "modrinth", id: "v0", projectId: "PtjYWJkn", name: "0.9.3", versionNumber: "mc1.21.1-0.9.3+fabric", gameVersions: ["1.21.1"], loaders: ["fabric"], published: "2026-08-01T00:00:00Z", releaseType: "beta", file: { url: "", filename: "sodium-extra-0.9.3.jar", sha1: null, size: 1000000 }, dependencies: [] },
    ],
    resource_plan: () => ({
      items: [
        { source: "modrinth", projectId: "PtjYWJkn", title: "Sodium Extra", versionId: "v1", versionNumber: "mc1.21.1-0.9.4+fabric", file: { url: "", filename: "sodium-extra-0.9.4.jar", sha1: null, size: 1024000 }, requiredBy: null, replaces: [], compat: { status: "ok", problems: [], notes: ["已核对 76 个必需的 Mixin 注入点"] }, note: null, alternative: null },
        { source: "modrinth", projectId: "AANobbMI", title: "Sodium", versionId: "s1", versionNumber: "mc1.21.1-0.8.13-fabric", file: { url: "", filename: "sodium-fabric-0.8.13.jar", sha1: null, size: 2048000 }, requiredBy: "Sodium Extra", replaces: [], compat: { status: "ok", problems: [], notes: ["已核对 75 个必需的 Mixin 注入点"] }, note: "原定的 mc1.21.1-0.8.12 不兼容（CameraMixin 要修改的 Camera.setup 在当前游戏里不存在），已自动换成 mc1.21.1-0.8.13", alternative: null },
      ],
      alreadyInstalled: [],
      unresolved: [],
      optional: ["Iris Shaders", "Reese's Sodium Options"],
      incompatible: [],
    }),
    local_resources: () => [
      { filename: "sodium-fabric-0.8.13.jar", enabled: true, size: 2048000, projectId: "AANobbMI", title: "Sodium", version: "0.8.13" },
      { filename: "old-mod.jar.disabled", enabled: false, size: 51200, projectId: null, title: null, version: null },
    ],
    local_compat: () => [
      { filename: "sodium-fabric-0.8.13.jar", report: { status: "ok", problems: [], notes: ["已核对 75 个必需的 Mixin 注入点"] } },
      { filename: "old-mod.jar.disabled", report: { status: "problem", problems: ["需要 Minecraft [1.20,1.21)，当前是 1.21.1"], notes: [] } },
    ],
    modpack_preview: () => ({
      suggestedName: "Fabulously Optimized-6.5.0",
      info: { format: "modrinth", name: "Fabulously Optimized", version: "6.5.0", author: "", summary: "Improve your graphics and performance with this simple modpack.", mc: "1.21.1", loader: "fabric", loaderVersion: "0.19.3", fileCount: 50, overrideCount: 63, minMemoryMb: null, jvmArgs: [], gameArgs: [], warnings: [] },
    }),
    export_candidates: () => [
      { name: "config", isDir: true, defaultOn: true },
      { name: "mods", isDir: true, defaultOn: true },
      { name: "options.txt", isDir: false, defaultOn: true },
      { name: "resourcepacks", isDir: true, defaultOn: true },
      { name: "saves", isDir: true, defaultOn: false },
      { name: "screenshots", isDir: true, defaultOn: false },
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
