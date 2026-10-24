import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DownloadReport,
  InstalledVersion,
  InstallProgress,
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

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),
  probeMirrors: () => call<ProbeResult[]>("probe_mirrors"),
  getVersionManifest: (force = false) => call<VersionManifest>("get_version_manifest", { force }),
  listInstalledVersions: () => call<InstalledVersion[]>("list_installed_versions"),
  installVersion: (id: string) => call<DownloadReport>("install_version", { id }),
};

export function onInstallProgress(cb: (p: InstallProgress) => void): Promise<UnlistenFn> {
  if (!inTauri) return Promise.resolve(() => {});
  return listen<InstallProgress>("install-progress", (e) => cb(e.payload));
}
