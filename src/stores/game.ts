import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onGameExit, onGameLog } from "../api";
import { useVersionsStore } from "./versions";
import type { CrashAnalysis } from "../api/types";

export interface GameRun {
  runId: string;
  versionId: string;
  player: string;
  startedAt: number;
  lines: string[];
  status: "running" | "exited";
  code: number | null;
  killed: boolean;
  analysis: CrashAnalysis | null;
}

/** 每个进程最多保留的日志行数 */
const MAX_LINES = 5000;

export const useGameStore = defineStore("game", () => {
  const runs = reactive<Record<string, GameRun>>({});
  const current = ref<string | null>(null);
  const launching = ref<string | null>(null);
  const error = ref<string | null>(null);

  const runList = computed(() => Object.values(runs).sort((a, b) => b.startedAt - a.startedAt));
  const running = computed(() => runList.value.filter((r) => r.status === "running"));

  // 进程还没登记前就可能来日志，先暂存
  const early: Record<string, string[]> = {};

  onGameLog(({ runId, line }) => {
    const run = runs[runId];
    if (!run) {
      (early[runId] ??= []).push(line);
      return;
    }
    run.lines.push(line);
    if (run.lines.length > MAX_LINES) run.lines.splice(0, run.lines.length - MAX_LINES);
  });

  onGameExit(({ runId, code, killed, analysis }) => {
    const run = runs[runId];
    if (run) {
      run.status = "exited";
      run.code = code;
      run.killed = killed;
      run.analysis = analysis;
    }
  });

  async function launch(versionId: string): Promise<boolean> {
    if (launching.value) return false;
    const versions = useVersionsStore();
    launching.value = versionId;
    error.value = null;
    const job = versions.trackLaunch(versionId);
    try {
      const r = await api.launchGame(versionId);
      runs[r.runId] = {
        ...r,
        startedAt: Date.now(),
        lines: early[r.runId] ?? [],
        status: "running",
        code: null,
        killed: false,
        analysis: null,
      };
      delete early[r.runId];
      current.value = r.runId;
      versions.dismiss(job.versionId);
      return true;
    } catch (e) {
      error.value = String(e);
      job.status = "failed";
      job.error = String(e);
      return false;
    } finally {
      launching.value = null;
    }
  }

  async function kill(runId: string) {
    await api.killGame(runId);
  }

  function clear(runId: string) {
    delete runs[runId];
    if (current.value === runId) current.value = runList.value[0]?.runId ?? null;
  }

  return { runs, runList, running, current, launching, error, launch, kill, clear };
});
