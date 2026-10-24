import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  AccountList,
  DeviceCode,
  DownloadReport,
  GameExit,
  GameLog,
  InstalledVersion,
  InstallProgress,
  JavaInstall,
  LaunchResult,
  MemoryInfo,
  ProbeResult,
  Settings,
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
  removeAccount: (id: string) => call<void>("remove_account", { id }),
  selectAccount: (id: string) => call<void>("select_account", { id }),

  listJava: (force = false) => call<JavaInstall[]>("list_java", { force }),
  memoryInfo: () => call<MemoryInfo>("memory_info"),

  launchGame: (versionId: string) => call<LaunchResult>("launch_game", { versionId }),
  killGame: (runId: string) => call<void>("kill_game", { runId }),
};

export const onInstallProgress = (cb: (p: InstallProgress) => void) => on("install-progress", cb);
export const onGameLog = (cb: (p: GameLog) => void) => on("game-log", cb);
export const onGameExit = (cb: (p: GameExit) => void) => on("game-exit", cb);
