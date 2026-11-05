<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useGameStore } from "../stores/game";
import { api } from "../api";
import type { CrashAnalysis } from "../api/types";
import CrashPanel from "../components/CrashPanel.vue";
import { errText, t } from "../i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "../api";
import { ask } from "../utils";

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
    fileError.value = errText(err);
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

// 分享到 mclo.gs：任何拿到链接的人都能看，先确认
const sharing = ref(false);
const shared = ref<{ runId: string; url: string } | null>(null);
const shareError = ref<string | null>(null);
async function share() {
  const r = run.value;
  if (!r || !(await ask(t("log.shareConfirm")))) return;
  sharing.value = true;
  shareError.value = null;
  try {
    shared.value = { runId: r.runId, url: await api.shareLog(r.lines.join("\n")) };
  } catch (e) {
    shareError.value = errText(e);
  } finally {
    sharing.value = false;
  }
}
function copyText(text: string) {
  navigator.clipboard.writeText(text);
}
async function openShared(url: string) {
  if (inTauri) await openUrl(url);
  else window.open(url, "_blank", "noopener,noreferrer");
}

function copyAll() {
  navigator.clipboard.writeText((run.value?.lines ?? []).join("\n"));
}

const exitText = computed(() => {
  const r = run.value;
  if (!r || r.status === "running") return "";
  if (r.killed) return t("log.killed");
  return r.code === 0 ? t("log.exitOk") : t("log.exitBad", { code: r.code ?? t("log.unknown") });
});
</script>

<template>
  <div class="box">
    <div class="box-title">
      {{ $t("nav.log") }}
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
      <button class="btn small" @click="fileInput?.click()">📄 {{ $t("log.analyzeFile") }}</button>
      <span class="muted">{{ $t("log.analyzeHint") }}</span>
      <input ref="fileInput" type="file" accept=".log,.txt" hidden @change="analyzeFile" />
      <span v-if="fileError" class="error-text">{{ fileError }}</span>
    </div>
    <div v-if="fileAnalysis" class="file-analysis">
      <CrashPanel :analysis="fileAnalysis.result" :title="$t('log.fileAnalysis', { name: fileAnalysis.name })" />
      <a class="close-file" @click="fileAnalysis = null">{{ $t("common.close") }}</a>
    </div>

    <div v-if="!run" class="box-body muted">
      <i18n-t keypath="log.empty"><RouterLink to="/versions">{{ $t("nav.versions") }}</RouterLink></i18n-t>
    </div>

    <template v-else>
      <div class="toolbar">
        <span>
          <b>{{ run.versionId }}</b> · {{ $t("log.player", { name: run.player }) }} ·
          <span v-if="run.status === 'running'" class="running">{{ $t("log.running") }}</span>
          <span v-else :class="run.code === 0 ? 'muted' : 'error-text'">{{ exitText }}</span>
        </span>
        <input v-model="filter" class="input" :placeholder="$t('log.filter')" />
        <button class="btn" @click="copyAll">{{ $t("log.copyAll") }}</button>
        <button class="btn" :disabled="sharing || !run.lines.length" @click="share">{{ sharing ? $t("log.sharing") : $t("log.share") }}</button>
        <button v-if="run.status === 'running'" class="btn" @click="game.kill(run.runId)">{{ $t("launchHome.kill") }}</button>
        <button v-else class="btn" @click="game.clear(run.runId)">{{ $t("progress.dismiss") }}</button>
      </div>
      <div v-if="shared?.runId === run.runId" class="share-row">
        {{ $t("log.shared") }} <a @click="openShared(shared.url)">{{ shared.url }}</a>
        <a @click="copyText(shared.url)">{{ $t("mp.copy") }}</a>
      </div>
      <div v-else-if="shareError" class="share-row error-text">{{ shareError }}</div>
      <CrashPanel v-if="run.analysis" :analysis="run.analysis" :title="$t('log.crashTitle')" />
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
.share-row { display: flex; gap: 10px; align-items: center; padding: 6px 12px; border-bottom: 1px dashed var(--border); font-size: 12px; }
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
