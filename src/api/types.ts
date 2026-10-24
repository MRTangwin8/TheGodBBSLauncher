// 和 Rust 侧 serde 结构保持一致（camelCase）

export type VersionKind = "release" | "snapshot" | "old_beta" | "old_alpha";

export interface ManifestEntry {
  id: string;
  type: VersionKind;
  url: string;
  time: string;
  releaseTime: string;
  sha1?: string;
}

export interface VersionManifest {
  latest: { release: string; snapshot: string };
  versions: ManifestEntry[];
}

export interface InstalledVersion {
  id: string;
  kind?: string;
  inheritsFrom?: string;
  releaseTime?: string;
  hasJar: boolean;
}

export type SourcePref = "auto" | "official" | "bmclapi";
export type Source = "official" | "bmclapi";

export interface Settings {
  gameDir: string;
  downloadSource: SourcePref;
  concurrency: number;
  javaPath: string | null;
  memoryMb: number | null;
  jvmArgs: string;
  gameArgs: string;
  isolateVersions: boolean;
  windowWidth: number | null;
  windowHeight: number | null;
}

export interface ProbeResult {
  source: Source;
  millis: number | null;
}

export interface Progress {
  totalFiles: number;
  finishedFiles: number;
  skippedFiles: number;
  failedFiles: number;
  totalBytes: number;
  downloadedBytes: number;
  speed: number;
}

export type Stage = "versionJson" | "assetIndex" | "files" | "java";

export interface InstallProgress {
  versionId: string;
  stage: Stage;
  progress: Progress;
}

export interface DownloadReport {
  total: number;
  downloaded: number;
  skipped: number;
  failed: { path: string; reason: string }[];
}

export type AccountKind = "offline" | "microsoft" | "yggdrasil";

export interface Account {
  id: string;
  kind: AccountKind;
  name: string;
  uuid: string;
  server?: string;
  serverName?: string;
}

export interface AccountList {
  accounts: Account[];
  selected: string | null;
}

export interface DeviceCode {
  userCode: string;
  deviceCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export interface JavaInstall {
  path: string;
  major: number;
  version: string;
  arch: string;
  vendor: string | null;
}

export interface MemoryInfo {
  totalMb: number;
  autoMb: number;
}

export interface LaunchResult {
  runId: string;
  versionId: string;
  player: string;
}

export interface GameLog {
  runId: string;
  line: string;
}

export interface GameExit {
  runId: string;
  versionId: string;
  code: number | null;
}
