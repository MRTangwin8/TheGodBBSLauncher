<script setup lang="ts">
import { ref } from "vue";
import { useVersionsStore, type InstallJob } from "../stores/versions";
import { formatBytes } from "../utils";
import { errText } from "../i18n";

const versions = useVersionsStore();
const collapsed = ref(false);


function percent(job: InstallJob): number {
  const p = job.progress;
  if (job.status === "done") return 100;
  if (!p) return 0;
  if (p.totalBytes > 0) return Math.min(100, (p.downloadedBytes / p.totalBytes) * 100);
  return p.totalFiles ? (p.finishedFiles / p.totalFiles) * 100 : 0;
}
</script>

<template>
  <!-- 下载进度做成"楼层"：每个任务是一层回复 -->
  <aside v-if="versions.jobList.length" class="float box">
    <div class="box-title">
      {{ $t("progress.title", { n: versions.jobList.length }) }}
      <a class="extra" @click="collapsed = !collapsed">{{ collapsed ? $t("common.expand") : $t("common.collapse") }}</a>
    </div>
    <div v-show="!collapsed" class="floors">
      <div v-for="job in versions.jobList" :key="job.versionId" class="floor">
        <div class="floor-head">
          <span class="floor-no">{{ job.floor }}#</span>
          <b>{{ job.versionId }}</b>
          <span v-if="job.status === 'running'" class="muted">
            {{ job.kind === "launch" ? (job.stage === "java" ? $t("progress.launchJava") : $t("progress.launchFiles")) : $t(`progress.stage.${job.stage}`) }}
          </span>
          <span v-else-if="job.status === 'done'" class="ok">{{ $t("progress.done") }}</span>
          <span v-else class="error-text">{{ job.kind === "launch" ? $t("progress.launchFailed") : $t("progress.failed") }}</span>
          <a v-if="job.status !== 'running'" class="close" :title="$t('progress.dismiss')" @click="versions.dismiss(job.versionId)">×</a>
        </div>
        <div class="xp-bar"><div :style="{ width: percent(job) + '%' }" /></div>
        <div v-if="job.progress && job.status === 'running' && job.stage === 'processor'" class="floor-meta muted">
          {{ $t("progress.processorStep", { i: job.progress.finishedFiles + 1 > job.progress.totalFiles ? job.progress.totalFiles : job.progress.finishedFiles + 1, n: job.progress.totalFiles }) }}
        </div>
        <div v-else-if="job.progress && job.status === 'running'" class="floor-meta muted">
          {{ $t("progress.files", { done: job.progress.finishedFiles, total: job.progress.totalFiles }) }} ·
          {{ formatBytes(job.progress.downloadedBytes) }}/{{ formatBytes(job.progress.totalBytes) }} ·
          {{ formatBytes(job.progress.speed) }}/s
        </div>
        <div v-else-if="job.status === 'done' && job.doneText" class="floor-meta muted">{{ job.doneText }}</div>
        <div v-else-if="job.status === 'done' && job.resultId" class="floor-meta muted">
          <i18n-t keypath="progress.generated"><b>{{ job.resultId }}</b><RouterLink to="/versions">{{ $t("nav.versions") }}</RouterLink></i18n-t>
        </div>
        <div v-else-if="job.status === 'done' && job.report" class="floor-meta muted">
          {{ $t("progress.report", { downloaded: job.report.downloaded, skipped: job.report.skipped }) }}
        </div>
        <div v-else-if="job.error" class="floor-meta error-text">{{ errText(job.error) }}</div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.float { position: fixed; right: 16px; bottom: 16px; width: 330px; z-index: 10; margin: 0; }
.floors { max-height: 300px; overflow-y: auto; }
.floor { padding: 8px 12px; border-bottom: 1px dashed var(--border); }
.floor:last-child { border-bottom: none; }
.floor-head { display: flex; align-items: center; gap: 6px; margin-bottom: 5px; }
.floor-no { color: var(--text-muted); font-family: var(--font-pixel); }
.floor-meta { margin-top: 4px; font-size: 12px; }
.ok { color: var(--grass); }
.close { margin-left: auto; font-size: 16px; line-height: 1; color: var(--text-muted); }
</style>
