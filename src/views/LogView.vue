<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useGameStore } from "../stores/game";
import { api } from "../api";
import type { CrashAnalysis } from "../api/types";
import CrashPanel from "../components/CrashPanel.vue";

const game = useGameStore();

// 分析外部日志文件（崩溃报告、别人发来的 latest.log 等）
const fileInput = ref<HTMLInputElement | null>(null);
const fileAnalysis = ref<{ name: string; result: CrashAnalysis } | null>(null);
const fileError = ref<string | null>(null);

async function analyzeFile(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  fileError.value = null;
  try {
    const result = await api.analyzeLogText(await file.text());
    fileAnalysis.value = { name: file.name, result };
  } catch (err) {
    fileError.value = String(err);
  } finally {
    (e.target as HTMLInputElement).value = "";
  }
}
const run = computed(() => (game.current ? game.runs[game.current] : game.runList[0]));
const box = ref<HTMLElement | null>(null);
const follow = ref(true);
const filter = ref("");

const lines = computed(() => {
  const all = run.value?.lines ?? [];
  const kw = filter.value.trim().toLowerCase();
  return kw ? all.filter((l) => l.toLowerCase().includes(kw)) : all;
});

function levelClass(line: string) {
  if (/\/(ERROR|FATAL)\]|Exception|^\s+at /.test(line)) return "lv-error";
  if (/\/WARN\]/.test(line)) return "lv-warn";
  if (line.startsWith("[神权]")) return "lv-god";
  return "";
}

watch(
  () => lines.value.length,
  async () => {
    if (!follow.value) return;
    await nextTick();
    box.value?.scrollTo({ top: box.value.scrollHeight });
  },
);

function onScroll() {
  const el = box.value;
  if (el) follow.value = el.scrollTop + el.clientHeight >= el.scrollHeight - 30;
}

function copyAll() {
  navigator.clipboard.writeText((run.value?.lines ?? []).join("\n"));
}

const exitText = computed(() => {
  const r = run.value;
  if (!r || r.status === "running") return "";
  if (r.killed) return "已手动结束";
  return r.code === 0 ? "游戏已正常退出" : `游戏异常退出（退出码 ${r.code ?? "未知"}）`;
});
</script>

<template>
  <div class="box">
    <div class="box-title">
      游戏日志
      <span class="extra runs">
        <a
          v-for="r in game.runList"
          :key="r.runId"
          :class="{ active: run?.runId === r.runId }"
          @click="game.current = r.runId"
        >
          {{ r.versionId }}<span v-if="r.status === 'running'" class="dot">●</span>
        </a>
      </span>
    </div>

    <div class="tool-row">
      <button class="btn small" @click="fileInput?.click()">📄 分析日志文件…</button>
      <span class="muted">支持崩溃报告 crash-*.txt、latest.log、hs_err_pid*.log</span>
      <input ref="fileInput" type="file" accept=".log,.txt" hidden @change="analyzeFile" />
      <span v-if="fileError" class="error-text">{{ fileError }}</span>
    </div>
    <div v-if="fileAnalysis" class="file-analysis">
      <CrashPanel :analysis="fileAnalysis.result" :title="`日志分析：${fileAnalysis.name}`" />
      <a class="close-file" @click="fileAnalysis = null">关闭</a>
    </div>

    <div v-if="!run" class="box-body muted">
      还没有启动过游戏。去 <RouterLink to="/versions">版本管理</RouterLink> 点「启动」吧。
    </div>

    <template v-else>
      <div class="toolbar">
        <span>
          <b>{{ run.versionId }}</b> · 玩家 {{ run.player }} ·
          <span v-if="run.status === 'running'" class="running">运行中</span>
          <span v-else :class="run.code === 0 ? 'muted' : 'error-text'">{{ exitText }}</span>
        </span>
        <input v-model="filter" class="input" placeholder="筛选日志…" />
        <button class="btn" @click="copyAll">复制全部</button>
        <button v-if="run.status === 'running'" class="btn" @click="game.kill(run.runId)">结束进程</button>
        <button v-else class="btn" @click="game.clear(run.runId)">删帖</button>
      </div>
      <CrashPanel v-if="run.analysis" :analysis="run.analysis" title="游戏崩溃了，版主帮你看了看" />
      <div ref="box" class="log" @scroll="onScroll">
        <div v-for="(l, i) in lines" :key="i" :class="levelClass(l)">{{ l }}</div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.runs { display: flex; gap: 4px; }
.runs a { padding: 0 8px; border: 1px solid var(--border); border-radius: 2px; color: var(--text); }
.runs a.active { background: var(--bg-box); font-weight: bold; }
.dot { color: var(--grass); margin-left: 4px; }
.toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--border); }
.toolbar > span { margin-right: auto; }
.running { color: var(--grass); font-weight: bold; }
.tool-row { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-bottom: 1px dashed var(--border); font-size: 12px; }
.file-analysis { position: relative; border-bottom: 1px solid var(--border); }
.close-file { position: absolute; right: 24px; bottom: 16px; font-size: 12px; }
.log {
  height: calc(100vh - 330px);
  min-height: 260px;
  overflow: auto;
  padding: 8px 12px;
  font-family: var(--font-pixel);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  background: #1e1a14;
  color: #e6dcc6;
}
.lv-warn { color: #f0c05a; }
.lv-error { color: #ff7a6b; }
.lv-god { color: #8fd36a; }
</style>
