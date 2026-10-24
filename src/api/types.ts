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

export type Stage = "versionJson" | "assetIndex" | "files";

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
