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
  autoCheckUpdate: true,
  skippedUpdate: null,
  announcementsEnabled: true,
  dismissedAnnouncements: [],
  buttons: { launchLabel: "", launchStyle: "stone", launchColor: "", launchSound: false, launchShortcut: true, navOrder: [], navHidden: [], quick: [] },
  customPalette: { accent: "#5e9c35", surface: "#fbf2db", text: "#3d2f1f", wood: "#6b4a2b" },
  tweaks: { radius: null, cardOpacity: null, cardBlur: null, bgBlur: 0, bgDim: 0, fontSize: 13, font: "", animations: true },
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

const modpackPage = {
  total: 2,
  hits: [
    { source: "modrinth", id: "1KVo5zza", slug: "fabulously-optimized", title: "Fabulously Optimized", description: "Beautiful graphics, speedy performance and familiar features in one simple package.", author: "robotkoer", iconUrl: null, downloads: 9800000, categories: ["optimization", "lightweight"], updated: "2026-10-20T00:00:00Z", pageUrl: "https://modrinth.com/modpack/fabulously-optimized" },
    { source: "curseforge", id: "285109", slug: "rlcraft", title: "RLCraft", description: "一个专门设计的整合包，旨在带来极度硬核且半现实的生存挑战。", author: "Shivaxi", iconUrl: null, downloads: 22000000, categories: ["Adventure and RPG", "Hardcore"], updated: "2026-09-12T00:00:00Z", pageUrl: "https://www.curseforge.com/minecraft/modpacks/rlcraft" },
  ],
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
    // 预览默认没有公告；地址加 ?mockNotice 可以看到有公告时的样子
    get_announcements: () => ({
      fetchedAt: Math.floor(Date.now() / 1000),
      stale: false,
      items: location.search.includes("mockNotice")
        ? [
            { id: "welcome", title: "欢迎使用神权论坛启动器", body: ["这里会显示 GitHub 上发布的公告。", "不用更新启动器就能收到。"].join(String.fromCharCode(10)), date: "2026-10-27", level: "info", pinned: true, link: "https://github.com/MRTangwin8/TheGodBBSLauncher", linkText: "项目主页", expires: null },
            { id: "warn1", title: "<b>这段不会被当成 HTML</b>", body: "旧版本的整合包导出有问题，请更新到 0.3.3。", date: "2026-10-26", level: "warn", pinned: false, link: null, linkText: "", expires: null },
          ]
        : [],
    }),
    quick_open_folder: () => undefined,
    quick_run_program: () => undefined,
    // 预览默认"已是最新"，免得弹窗盖住截图；地址加 ?mockUpdate 可以看到有更新时的样子
    check_update: () => {
      const has = location.search.includes("mockUpdate");
      return {
        current: "0.2.0",
        latest: has ? "0.3.0" : "0.2.0",
        hasUpdate: has,
        url: "https://github.com/MRTangwin8/TheGodBBSLauncher/releases/tag/" + (has ? "v0.3.0" : "v0.2.0"),
        notes: has ? ["## 更新内容", "- 资源中心新增整合包分类", "- 启动时检查更新，有新版本会提示", "- CI 输出安装包的 SHA256 / MD5"].join(String.fromCharCode(10)) : "",
        published: "2026-10-26T00:00:00Z",
        prerelease: true,
        canInstall: has,
      };
    },
    install_update: () => {
      throw "浏览器预览模式不能在应用内更新";
    },
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
    klpbbs_search: () => ({
      total: 2,
      hits: [
        { source: "curseforge", id: "238222", slug: "jei", title: "JEI 物品管理器", description: "查询物品、配方与用途。", author: "mezz", iconUrl: null, downloads: 460000000, categories: ["物品管理"], updated: "2026-10-01T00:00:00Z", pageUrl: "https://www.curseforge.com/minecraft/mc-mods/jei" },
        { source: "modrinth", id: "AANobbMI", slug: "sodium", title: "钠 Sodium", description: "Minecraft 客户端渲染优化模组。", author: "jellysquid3", iconUrl: null, downloads: 80000000, categories: ["优化"], updated: "2026-10-01T00:00:00Z", pageUrl: "https://modrinth.com/mod/sodium" },
      ],
    }),
    resource_search: () => ((args?.query as { kind?: string } | undefined)?.kind === "modpack" ? modpackPage : {
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
    modpack_versions: () => [
      { source: "modrinth", id: "fo1", projectId: "1KVo5zza", name: "6.5.0", versionNumber: "6.5.0", gameVersions: ["1.21.1"], loaders: ["fabric"], published: "2026-10-20T00:00:00Z", releaseType: "release", file: { url: "", filename: "Fabulously.Optimized-6.5.0.mrpack", sha1: null, size: 31000 }, dependencies: [] },
      { source: "modrinth", id: "fo0", projectId: "1KVo5zza", name: "6.4.1", versionNumber: "6.4.1", gameVersions: ["1.21"], loaders: ["fabric"], published: "2026-08-02T00:00:00Z", releaseType: "beta", file: { url: "", filename: "Fabulously.Optimized-6.4.1.mrpack", sha1: null, size: 30500 }, dependencies: [] },
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
    microsoft_login_start: () => ({ userCode: "GODBBS42", verificationUri: "https://www.microsoft.com/link", expiresIn: 900, interval: 5 }),
    microsoft_login_finish: () => new Promise(() => {}),
  };
  return (table[cmd] ?? unsupported)() as T;
}
