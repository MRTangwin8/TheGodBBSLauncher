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
  loader: LoaderKind | null;
  mcVersion: string | null;
  playTimeSecs: number;
  lastPlayed: number | null;
  modpack: string | null;
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
  crashRulesUrl: string;
  curseforgeApiKey: string;
  layout: "classic" | "sidebar" | "modern";
  palette: string;
  theme: "light" | "dark" | "system";
  backgroundImage: string | null;
  lastVersion: string | null;
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

export type Stage = "versionJson" | "assetIndex" | "files" | "java" | "loader" | "processor";

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

export interface Skin {
  dataUrl: string;
  slim: boolean;
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
  killed: boolean;
  analysis: CrashAnalysis | null;
}

export type Severity = "error" | "warning" | "info";

export interface Finding {
  id: string;
  title: string;
  severity: Severity;
  description: string;
  solutions: string[];
  evidence: string | null;
}

export interface CrashAnalysis {
  exitCode: number | null;
  findings: Finding[];
  files: string[];
}

export interface RulesInfo {
  version: number;
  count: number;
  custom: boolean;
}

export type LoaderKind = "fabric" | "quilt" | "forge" | "neoforge";

export interface LoaderVersion {
  id: string;
  version: string;
  stable: boolean;
}

export type ResourceSource = "modrinth" | "curseforge";
export type ResourceKind = "mod" | "resourcepack" | "shader";
export type SortBy = "relevance" | "downloads" | "updated";

export interface SearchQuery {
  query: string;
  kind: ResourceKind;
  gameVersion: string | null;
  loader: LoaderKind | null;
  sort: SortBy;
  offset: number;
  limit: number;
}

export interface Project {
  source: ResourceSource;
  id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  iconUrl: string | null;
  downloads: number;
  categories: string[];
  updated: string;
  pageUrl: string;
}

export interface SearchPage {
  hits: Project[];
  total: number;
}

export interface ResFile {
  url: string;
  filename: string;
  sha1: string | null;
  size: number;
}

export interface ResourceVersion {
  source: ResourceSource;
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  published: string;
  releaseType: string;
  file: ResFile | null;
  dependencies: { projectId: string; versionId: string | null; kind: string }[];
}

export interface PlanItem {
  source: ResourceSource;
  projectId: string;
  title: string;
  versionId: string;
  versionNumber: string;
  file: ResFile;
  requiredBy: string | null;
  replaces: string[];
  compat: CompatReport | null;
  note: string | null;
  alternative: ResourceVersion | null;
}

export interface CompatReport {
  status: "ok" | "problem" | "unknown";
  problems: string[];
  notes: string[];
}

export interface LocalCompat {
  filename: string;
  report: CompatReport;
}

export interface InstallPlan {
  items: PlanItem[];
  alreadyInstalled: string[];
  unresolved: string[];
  optional: string[];
  incompatible: string[];
}

export interface TargetInfo {
  mcVersion: string | null;
  loader: LoaderKind | null;
  dir: string;
  curseforgeViaMirror: boolean;
}

export interface LocalResource {
  filename: string;
  enabled: boolean;
  size: number;
  projectId: string | null;
  title: string | null;
  version: string | null;
}

export type PackFormat = "modrinth" | "curseforge" | "mcbbs";

export interface ModpackInfo {
  format: PackFormat;
  name: string;
  version: string;
  author: string;
  summary: string;
  mc: string;
  loader: LoaderKind | null;
  loaderVersion: string | null;
  fileCount: number;
  overrideCount: number;
  minMemoryMb: number | null;
  jvmArgs: string[];
  gameArgs: string[];
  warnings: string[];
}

export interface ModpackPreview {
  info: ModpackInfo;
  suggestedName: string;
}

export interface ExportEntry {
  name: string;
  isDir: boolean;
  defaultOn: boolean;
}

export interface ExportOptions {
  format: PackFormat;
  name: string;
  version: string;
  author: string;
  summary: string;
  include: string[];
}

export interface ExportSummary {
  path: string;
  linked: number;
  bundled: number;
}
