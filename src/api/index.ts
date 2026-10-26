import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  AccountList,
  CrashAnalysis,
  DeviceCode,
  DownloadReport,
  ExportEntry,
  ExportOptions,
  ExportSummary,
  ModpackPreview,
  GameExit,
  GameLog,
  InstalledVersion,
  InstallPlan,
  LocalCompat,
  LocalResource,
  ResourceKind,
  ResourceSource,
  ResourceVersion,
  SearchPage,
  SearchQuery,
  Skin,
  TargetInfo,
  InstallProgress,
  JavaInstall,
  LoaderKind,
  LoaderVersion,
  LaunchResult,
  MemoryInfo,
  ProbeResult,
  RulesInfo,
  Settings,
  ThemePreview,
  VersionManifest,
} from "./types";
import { mock } from "./mock";

/** 在普通浏览器里预览界面时没有 Tauri，用假数据代替 */
export const inTauri = "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return Promise.resolve(() => {});
  return listen<T>(event, (e) => cb(e.payload));
}

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),
  backgroundImage: () => call<string | null>("background_image"),
  themeExport: (path: string, name: string, author: string) => call<void>("theme_export", { path, name, author }),
  themePreview: (path: string) => call<ThemePreview>("theme_preview", { path }),
  themeImport: (path: string) => call<Settings>("theme_import", { path }),
  probeMirrors: () => call<ProbeResult[]>("probe_mirrors"),
  getVersionManifest: (force = false) => call<VersionManifest>("get_version_manifest", { force }),
  listInstalledVersions: () => call<InstalledVersion[]>("list_installed_versions"),
  installVersion: (id: string) => call<DownloadReport>("install_version", { id }),

  listAccounts: () => call<AccountList>("list_accounts"),
  addOfflineAccount: (name: string) => call<Account>("add_offline_account", { name }),
  microsoftLoginStart: () => call<DeviceCode>("microsoft_login_start"),
  microsoftLoginFinish: () => call<Account>("microsoft_login_finish"),
  microsoftLoginCancel: () => call<void>("microsoft_login_cancel"),
  addYggdrasilAccount: (server: string, username: string, password: string) =>
    call<Account>("add_yggdrasil_account", { server, username, password }),
  accountSkin: (id: string) => call<Skin | null>("account_skin", { id }),
  removeAccount: (id: string) => call<void>("remove_account", { id }),
  selectAccount: (id: string) => call<void>("select_account", { id }),

  listLoaderVersions: (kind: LoaderKind, mcVersion: string) => call<LoaderVersion[]>("list_loader_versions", { kind, mcVersion }),
  installLoader: (jobId: string, mcVersion: string, kind: LoaderKind, loaderId: string, name: string | null) =>
    call<string>("install_loader", { jobId, mcVersion, kind, loaderId, name }),
  renameVersion: (id: string, newName: string) => call<void>("rename_version", { id, newName }),

  listJava: (force = false) => call<JavaInstall[]>("list_java", { force }),
  memoryInfo: () => call<MemoryInfo>("memory_info"),

  resourceTarget: (versionId: string, kind: ResourceKind) => call<TargetInfo>("resource_target", { versionId, kind }),
  resourceSearch: (source: ResourceSource, query: SearchQuery) => call<SearchPage>("resource_search", { source, query }),
  klpbbsSearch: (query: SearchQuery) => call<SearchPage>("klpbbs_search", { query }),
  resourceVersions: (source: ResourceSource, projectId: string, kind: ResourceKind, versionId: string) =>
    call<ResourceVersion[]>("resource_versions", { source, projectId, kind, versionId }),
  resourcePlan: (kind: ResourceKind, versionId: string, resource: ResourceVersion) =>
    call<InstallPlan>("resource_plan", { kind, versionId, resource }),
  resourceInstall: (jobId: string, kind: ResourceKind, versionId: string, plan: InstallPlan) =>
    call<DownloadReport>("resource_install", { jobId, kind, versionId, plan }),
  localResources: (versionId: string, kind: ResourceKind) => call<LocalResource[]>("local_resources", { versionId, kind }),
  localResourceToggle: (versionId: string, kind: ResourceKind, filename: string, enabled: boolean) =>
    call<string>("local_resource_toggle", { versionId, kind, filename, enabled }),
  localResourceDelete: (versionId: string, kind: ResourceKind, filename: string) =>
    call<void>("local_resource_delete", { versionId, kind, filename }),
  localCompat: (versionId: string) => call<LocalCompat[]>("local_compat", { versionId }),

  modpackPreview: (path: string) => call<ModpackPreview>("modpack_preview", { path }),
  modpackImport: (jobId: string, path: string, name: string) => call<string>("modpack_import", { jobId, path, name }),
  modpackVersions: (source: ResourceSource, projectId: string) => call<ResourceVersion[]>("modpack_versions", { source, projectId }),
  modpackDownload: (jobId: string, version: ResourceVersion) => call<string>("modpack_download", { jobId, version }),
  exportCandidates: (versionId: string) => call<ExportEntry[]>("export_candidates", { versionId }),
  modpackExport: (jobId: string, versionId: string, options: ExportOptions, outPath: string) =>
    call<ExportSummary>("modpack_export", { jobId, versionId, options, outPath }),

  analyzeLogText: (text: string) => call<CrashAnalysis>("analyze_log_text", { text }),
  crashRulesInfo: () => call<RulesInfo>("crash_rules_info"),
  updateCrashRules: (url: string) => call<RulesInfo>("update_crash_rules", { url }),
  resetCrashRules: () => call<RulesInfo>("reset_crash_rules"),

  launchGame: (versionId: string) => call<LaunchResult>("launch_game", { versionId }),
  killGame: (runId: string) => call<void>("kill_game", { runId }),
};

export const onInstallProgress = (cb: (p: InstallProgress) => void) => on("install-progress", cb);
export const onGameLog = (cb: (p: GameLog) => void) => on("game-log", cb);
export const onGameExit = (cb: (p: GameExit) => void) => on("game-exit", cb);
