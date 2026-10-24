import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onInstallProgress } from "../api";
import type { DownloadReport, InstalledVersion, LoaderKind, LoaderVersion, Progress, Stage, VersionManifest } from "../api/types";
import { loaderLabel } from "../utils";

export interface InstallJob {
  versionId: string;
  /** 安装，或启动前的补全文件 / 下载 Java */
  kind: "install" | "launch";
  /** 楼层号，按开始顺序递增 */
  floor: number;
  stage: Stage;
  progress: Progress | null;
  status: "running" | "done" | "failed";
  error?: string;
  report?: DownloadReport;
  /** 加载器安装成功后的版本 id */
  resultId?: string;
}

export const useVersionsStore = defineStore("versions", () => {
  const manifest = ref<VersionManifest | null>(null);
  const manifestError = ref<string | null>(null);
  const installed = ref<InstalledVersion[]>([]);
  const jobs = reactive<Record<string, InstallJob>>({});
  let floorCounter = 0;

  const installedIds = computed(() => new Set(installed.value.map((v) => v.id)));
  const jobList = computed(() => Object.values(jobs).sort((a, b) => b.floor - a.floor));
  const running = computed(() => jobList.value.filter((j) => j.status === "running"));

  onInstallProgress((p) => {
    const job = jobs[p.versionId];
    if (job) {
      job.stage = p.stage;
      job.progress = p.progress;
    }
  });

  async function loadManifest(force = false) {
    manifestError.value = null;
    try {
      manifest.value = await api.getVersionManifest(force);
    } catch (e) {
      manifestError.value = String(e);
    }
  }

  async function loadInstalled() {
    installed.value = await api.listInstalledVersions();
  }

  async function install(versionId: string) {
    if (jobs[versionId]?.status === "running") return;
    jobs[versionId] = { versionId, kind: "install", floor: ++floorCounter, stage: "versionJson", progress: null, status: "running" };
    const job = jobs[versionId];
    try {
      job.report = await api.installVersion(versionId);
      job.status = job.report.failed.length ? "failed" : "done";
      if (job.report.failed.length) job.error = `${job.report.failed.length} 个文件下载失败：${job.report.failed[0].reason}`;
    } catch (e) {
      job.status = "failed";
      job.error = String(e);
    }
    await loadInstalled();
  }

  /** 原版 + 加载器；任务名就是显示在楼层里的标题 */
  async function installLoader(mc: string, kind: LoaderKind, loader: LoaderVersion, name: string | null) {
    const jobId = name || `${mc} + ${loaderLabel[kind]} ${loader.version}`;
    if (jobs[jobId]?.status === "running") return;
    jobs[jobId] = { versionId: jobId, kind: "install", floor: ++floorCounter, stage: "versionJson", progress: null, status: "running" };
    const job = jobs[jobId];
    try {
      const id = await api.installLoader(jobId, mc, kind, loader.id, name);
      job.status = "done";
      job.resultId = id;
    } catch (e) {
      job.status = "failed";
      job.error = String(e);
    }
    await loadInstalled();
  }

  function dismiss(versionId: string) {
    delete jobs[versionId];
  }

  /** 其它下载任务（如资源中心）借用同一套楼层显示 */
  function startJob(jobId: string): InstallJob {
    jobs[jobId] = { versionId: jobId, kind: "install", floor: ++floorCounter, stage: "files", progress: null, status: "running" };
    return jobs[jobId];
  }

  /** 启动准备阶段也走 install-progress 事件，借用同一套"楼层"显示 */
  function trackLaunch(versionId: string): InstallJob {
    jobs[versionId] = { versionId, kind: "launch", floor: ++floorCounter, stage: "files", progress: null, status: "running" };
    return jobs[versionId];
  }

  return { manifest, manifestError, installed, installedIds, jobs, jobList, running, loadManifest, loadInstalled, install, installLoader, dismiss, trackLaunch, startJob };
});
