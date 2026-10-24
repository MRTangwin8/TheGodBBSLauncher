<script setup lang="ts">
import { ref } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { inTauri } from "../api";
import type { CrashAnalysis } from "../api/types";

const props = defineProps<{ analysis: CrashAnalysis; title?: string }>();
const openEvidence = ref<Set<string>>(new Set());

const severityLabel = { error: "错误", warning: "可能", info: "提示" };

function toggle(id: string) {
  const s = new Set(openEvidence.value);
  s.has(id) ? s.delete(id) : s.add(id);
  openEvidence.value = s;
}

function reveal(path: string) {
  if (inTauri) revealItemInDir(path);
}

function copyReport() {
  const lines = props.analysis.findings.flatMap((f) => [
    `【${f.title}】${f.description}`,
    ...f.solutions.map((s, i) => `  ${i + 1}. ${s}`),
    f.evidence ? `  日志：${f.evidence}` : "",
  ]);
  navigator.clipboard.writeText(lines.filter(Boolean).join("\n"));
}
</script>

<template>
  <div class="crash">
    <div class="crash-head">
      <span class="crash-title">🔍 {{ title ?? "崩溃分析" }}</span>
      <span v-if="analysis.exitCode !== null" class="muted">退出码 {{ analysis.exitCode }}</span>
      <a v-if="analysis.findings.length" class="copy" @click="copyReport">复制诊断结果</a>
    </div>

    <div v-if="!analysis.findings.length" class="unknown">
      <b>没能识别出具体原因。</b>
      <span class="muted">可以把完整日志复制下来发给坛友求助；如果是新问题，欢迎补充到规则库里。</span>
    </div>

    <div v-for="(f, i) in analysis.findings" :key="f.id" class="floor" :class="f.severity">
      <div class="floor-head">
        <span class="floor-no">{{ i + 1 }}#</span>
        <span class="sev">{{ severityLabel[f.severity] }}</span>
        <b>{{ f.title }}</b>
      </div>
      <p class="desc">{{ f.description }}</p>
      <ol v-if="f.solutions.length" class="solutions">
        <li v-for="s in f.solutions" :key="s">{{ s }}</li>
      </ol>
      <div v-if="f.evidence" class="evidence">
        <a @click="toggle(f.id)">{{ openEvidence.has(f.id) ? "收起日志依据" : "查看日志依据" }}</a>
        <code v-if="openEvidence.has(f.id)">{{ f.evidence }}</code>
      </div>
    </div>

    <div v-if="analysis.files.length" class="files muted">
      已分析：
      <a v-for="p in analysis.files" :key="p" :title="p" @click="reveal(p)">{{ p.split(/[\\/]/).pop() }}</a>
    </div>
  </div>
</template>

<style scoped>
.crash { border: 1px solid var(--border-strong); border-radius: var(--radius); background: var(--bg-box); margin: 10px 12px; }
.crash-head { display: flex; align-items: center; gap: 10px; padding: 7px 12px; background: var(--bg-title); border-bottom: 1px solid var(--border); }
.crash-title { font-weight: bold; color: var(--brown-dark); }
.copy { margin-left: auto; font-size: 12px; }
.unknown { padding: 10px 12px; display: flex; flex-direction: column; gap: 4px; }
.floor { padding: 10px 12px; border-bottom: 1px dashed var(--border); border-left: 4px solid transparent; }
.floor:last-of-type { border-bottom: none; }
.floor.error { border-left-color: var(--red); }
.floor.warning { border-left-color: var(--gold); }
.floor.info { border-left-color: var(--link); }
.floor-head { display: flex; align-items: center; gap: 6px; }
.floor-no { color: var(--text-muted); font-family: var(--font-pixel); }
.sev { font-size: 11px; padding: 0 5px; border-radius: 2px; color: #fff; background: var(--red); }
.warning .sev { background: var(--gold); }
.info .sev { background: var(--link); }
.desc { margin: 6px 0; line-height: 1.6; }
.solutions { margin: 0; padding-left: 22px; line-height: 1.7; }
.evidence { margin-top: 6px; font-size: 12px; }
.evidence code { display: block; margin-top: 4px; padding: 6px 8px; background: #1e1a14; color: #f0c05a; border-radius: 2px; white-space: pre-wrap; word-break: break-all; font-family: var(--font-pixel); }
.files { padding: 6px 12px; font-size: 12px; border-top: 1px solid var(--border); display: flex; gap: 8px; flex-wrap: wrap; }
</style>
