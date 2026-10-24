<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useGameStore } from "../stores/game";

const game = useGameStore();
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
      <div v-if="run.status === 'exited' && run.code !== 0" class="box-body crash">
        <div class="mod-tip">【版主提示】游戏崩溃了。崩溃分析功能还在施工，可以先把日志复制给坛友求助。</div>
      </div>
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
.crash { padding-bottom: 0; }
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
