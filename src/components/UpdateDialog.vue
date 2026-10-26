<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "../api";
import type { UpdateInfo } from "../api/types";
import { formatDate } from "../utils";

// 发现新版本时问一声：去下载 / 以后再说 / 跳过这个版本。更新说明是纯文本，不当 HTML 渲染
const props = defineProps<{ info: UpdateInfo }>();
const emit = defineEmits<{ close: []; skip: [version: string] }>();
const error = ref("");

async function download() {
  if (!props.info.url) return;
  try {
    if (inTauri) await openUrl(props.info.url);
    else window.open(props.info.url, "_blank", "noopener,noreferrer");
    emit("close");
  } catch (e) {
    error.value = `打不开浏览器：${e}。发布页地址：${props.info.url}`;
  }
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box" role="dialog" aria-modal="true" aria-label="发现新版本">
      <div class="box-title">
        发现新版本
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body">
        <p class="versions">
          <span class="muted">当前 v{{ info.current }}</span>
          <span class="arrow">→</span>
          <b class="new">v{{ info.latest }}</b>
          <span v-if="info.prerelease" class="tag pre">预发布</span>
          <span v-if="info.published" class="muted date">{{ formatDate(info.published) }}</span>
        </p>
        <pre v-if="info.notes" class="notes">{{ info.notes }}</pre>
        <p v-else class="muted">这个版本没有填写更新说明。</p>
        <p class="muted small">点「前往下载」会用系统浏览器打开发布页，从那里选择适合你系统的安装包；核对文件的 SHA256 / MD5 请看发布页或构建日志。</p>
        <p v-if="error" class="error-text small">{{ error }}</p>
      </div>
      <div class="actions">
        <button class="btn small" @click="emit('skip', info.latest ?? '')">跳过此版本</button>
        <span class="grow" />
        <button class="btn" @click="emit('close')">以后再说</button>
        <button class="btn primary" @click="download">前往下载</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 30; }
.dialog { width: 520px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.versions { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 0 0 10px; font-size: 15px; }
.arrow { color: var(--text-muted); }
.new { font-size: 18px; color: var(--grass); }
.date { margin-left: auto; font-size: 12px; }
.tag.pre { background: #c47d1a; margin: 0; }
.notes {
  max-height: 220px;
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
.small { font-size: 12px; margin: 4px 0; }
.actions { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
.grow { flex: 1; }
</style>
