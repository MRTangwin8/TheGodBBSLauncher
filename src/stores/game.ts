import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onGameExit, onGameLog } from "../api";
import { useVersionsStore } from "./versions";
import type { CrashAnalysis, GameExit } from "../api/types";

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
  const earlyExits: Record<string, GameExit> = {};

  function appendLine(lines: string[], line: string) {
    lines.push(line);
    if (lines.length > MAX_LINES) lines.splice(0, lines.length - MAX_LINES);
  }

  function applyExit(run: GameRun, event: GameExit) {
    run.status = "exited";
    run.code = event.code;
    run.killed = event.killed;
    run.analysis = event.analysis;
  }

  onGameLog(({ runId, line }) => {
    const run = runs[runId];
    if (!run) {
      if (launching.value) appendLine(early[runId] ??= [], line);
      return;
    }
    appendLine(run.lines, line);
  });

  onGameExit((event) => {
    const run = runs[event.runId];
    if (run) {
      applyExit(run, event);
    } else if (launching.value) {
      earlyExits[event.runId] = event;
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
      if (earlyExits[r.runId]) applyExit(runs[r.runId], earlyExits[r.runId]);
      delete early[r.runId];
      delete earlyExits[r.runId];
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
      for (const id of Object.keys(early)) delete early[id];
      for (const id of Object.keys(earlyExits)) delete earlyExits[id];
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
