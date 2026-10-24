import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onInstallProgress } from "../api";
import type { DownloadReport, InstalledVersion, Progress, Stage, VersionManifest } from "../api/types";

export interface InstallJob {
  versionId: string;
  /** 楼层号，按开始顺序递增 */
  floor: number;
  stage: Stage;
  progress: Progress | null;
  status: "running" | "done" | "failed";
  error?: string;
  report?: DownloadReport;
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
    jobs[versionId] = { versionId, floor: ++floorCounter, stage: "versionJson", progress: null, status: "running" };
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

  function dismiss(versionId: string) {
    delete jobs[versionId];
  }

  return { manifest, manifestError, installed, installedIds, jobs, jobList, running, loadManifest, loadInstalled, install, dismiss };
});
