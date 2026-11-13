import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onGameExit, onGameLog } from "../api";
import { useVersionsStore } from "./versions";
import { useSettingsStore } from "./settings";
import type { CrashAnalysis, GameExit } from "../api/types";

export interface GameRun {
  runId: string;
  versionId: string;
  player: string;
  startedAt: number;
  lines: string[];
  /** 因为超过上限从开头丢掉的行数：界面用「丢掉的行数 + 下标」当每一行的固定编号 */
  dropped: number;
  status: "running" | "exited";
  code: number | null;
  killed: boolean;
  /** 正常退出（见 GameExit.normal） */
  normal: boolean;
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

  /** 追加一批日志，只留最后 MAX_LINES 行；返回丢掉了多少行 */
  function appendLines(lines: string[], batch: string[]): number {
    // 一批就超过上限时只要它的结尾，不用先全塞进去再删
    const tail = batch.length > MAX_LINES ? batch.slice(batch.length - MAX_LINES) : batch;
    const over = lines.length + tail.length - MAX_LINES;
    if (over > 0) lines.splice(0, Math.min(over, lines.length));
    lines.push(...tail);
    return Math.max(over, 0) + (batch.length - tail.length);
  }

  function applyExit(run: GameRun, event: GameExit) {
    run.status = "exited";
    run.code = event.code;
    run.killed = event.killed;
    run.normal = event.normal ?? event.code === 0;
    run.analysis = event.analysis;
  }

  onGameLog(({ runId, lines }) => {
    const run = runs[runId];
    if (!run) {
      if (launching.value) appendLines(early[runId] ??= [], lines);
      return;
    }
    run.dropped += appendLines(run.lines, lines);
  });

  onGameExit((event) => {
    const run = runs[event.runId];
    if (run) {
      applyExit(run, event);
    } else if (launching.value) {
      earlyExits[event.runId] = event;
    }
  });

  async function launch(versionId: string, server?: string | null): Promise<boolean> {
    if (launching.value) return false;
    const versions = useVersionsStore();
    launching.value = versionId;
    error.value = null;
    // 后端一开始启动就把它记成「上次启动的版本」，界面这份设置也要跟上：
    // 不然回到首页选中的还是旧版本，之后保存别的设置时还会把旧值写回去
    const settings = useSettingsStore().settings;
    if (settings) settings.lastVersion = versionId;
    const job = versions.trackLaunch(versionId);
    try {
      const r = await api.launchGame(versionId, server);
      runs[r.runId] = {
        ...r,
        startedAt: Date.now(),
        lines: early[r.runId] ?? [],
        dropped: 0,
        status: "running",
        code: null,
        killed: false,
        normal: false,
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
