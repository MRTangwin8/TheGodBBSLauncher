<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri, onUpdateProgress } from "../api";
import type { UpdateInfo, UpdateProgress } from "../api/types";
import { useGameStore } from "../stores/game";
import { formatBytes, formatDate } from "../utils";
import { errText, t } from "../i18n";

// 发现新版本时问一声：立即更新（应用内安装）/ 前往下载 / 以后再说 / 跳过此版本。
// 更新说明是纯文本，不当 HTML 渲染。安装包的签名由后端校验，校验不过不会装。
const props = defineProps<{ info: UpdateInfo }>();
const emit = defineEmits<{ close: []; skip: [version: string] }>();
const game = useGameStore();

const installing = ref(false);
const progress = ref<UpdateProgress | null>(null);
const error = ref("");

const percent = computed(() => {
  const p = progress.value;
  return p?.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : 0;
});
const downloadDone = computed(() => !!progress.value?.total && progress.value.downloaded >= progress.value.total);

let unlisten: (() => void) | undefined;
onUnmounted(() => unlisten?.());

async function install() {
  if (!props.info.latest) return;
  if (game.running.length) {
    error.value = t("update.gameRunning");
    return;
  }
  error.value = "";
  installing.value = true;
  progress.value = null;
  unlisten = await onUpdateProgress((p) => (progress.value = p));
  try {
    // 成功的话启动器会在这里自己重启，下面不会执行到
    await api.installUpdate(props.info.latest);
  } catch (e) {
    error.value = errText(e);
    installing.value = false;
  } finally {
    unlisten?.();
    unlisten = undefined;
  }
}

async function download() {
  if (!props.info.url) return;
  try {
    if (inTauri) await openUrl(props.info.url);
    else window.open(props.info.url, "_blank", "noopener,noreferrer");
    emit("close");
  } catch (e) {
    error.value = t("update.cantOpen", { err: errText(e), url: props.info.url });
  }
}

function dismiss() {
  if (!installing.value) emit("close");
}
</script>

<template>
  <div class="mask" @click.self="dismiss">
    <div class="dialog box" role="dialog" aria-modal="true" :aria-label="$t('update.title')">
      <div class="box-title">
        {{ $t("update.title") }}
        <a v-if="!installing" class="extra close" @click="dismiss">×</a>
      </div>
      <div class="box-body">
        <p class="versions">
          <span class="muted">{{ $t("update.current", { v: info.current }) }}</span>
          <span class="arrow">→</span>
          <b class="new">v{{ info.latest }}</b>
          <span v-if="info.prerelease" class="tag pre">{{ $t("update.prerelease") }}</span>
          <span v-if="info.published" class="muted date">{{ formatDate(info.published) }}</span>
        </p>
        <pre v-if="info.notes" class="notes">{{ info.notes }}</pre>
        <p v-else class="muted">{{ $t("update.noNotes") }}</p>

        <div v-if="installing" class="progress">
          <div class="xp-bar"><div :style="{ width: (downloadDone ? 100 : percent) + '%' }" /></div>
          <p class="muted small">
            <template v-if="downloadDone">{{ $t("update.downloaded") }}</template>
            <template v-else-if="progress">{{ $t("update.downloading", { size: formatBytes(progress.downloaded) }) }}<template v-if="progress.total"> / {{ formatBytes(progress.total) }} ({{ percent }}%)</template></template>
            <template v-else>{{ $t("update.connecting") }}</template>
          </p>
        </div>
        <p v-else-if="info.canInstall" class="muted small">{{ $t("update.installHint") }}</p>
        <p v-else class="muted small">{{ $t("update.downloadHint") }}</p>
        <p v-if="error" class="error-text small">{{ error }}<template v-if="info.canInstall"> {{ $t("update.fallback") }}</template></p>
      </div>
      <div class="actions">
        <button class="btn small" :disabled="installing" @click="emit('skip', info.latest ?? '')">{{ $t("update.skip") }}</button>
        <span class="grow" />
        <button class="btn" :disabled="installing" @click="dismiss">{{ $t("update.later") }}</button>
        <button class="btn" :class="{ primary: !info.canInstall }" :disabled="installing" @click="download">{{ $t("update.goDownload") }}</button>
        <button v-if="info.canInstall" class="btn primary" :disabled="installing" @click="install">{{ installing ? $t("update.updating") : $t("update.now") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 30; }
.dialog { width: 540px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.versions { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 0 0 10px; font-size: 15px; }
.arrow { color: var(--text-muted); }
.new { font-size: 18px; color: var(--grass); }
.date { margin-left: auto; font-size: 12px; }
.tag.pre { background: #c47d1a; margin: 0; }
.notes {
  max-height: 200px;
  overflow-y: auto;
  margin: 0 0 10px;
  padding: 10px 12px;
  white-space: pre-wrap;
  word-break: break-word;
  font: 12px/1.6 var(--font);
  color: var(--text);
  background: var(--bg-box-alt);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.progress { margin: 6px 0; }
.small { font-size: 12px; margin: 4px 0; }
/* 日语等文案较长的语言一行放不下，按钮换行而不是溢出对话框 */
.actions { display: flex; align-items: center; flex-wrap: wrap; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
.actions .btn { white-space: nowrap; }
.grow { flex: 1; }
</style>
