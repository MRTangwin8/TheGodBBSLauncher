<script setup lang="ts">
import { ref } from "vue";
import { useVersionsStore, type InstallJob } from "../stores/versions";
import { formatBytes } from "../utils";

const versions = useVersionsStore();
const collapsed = ref(false);

const stageText: Record<InstallJob["stage"], string> = {
  versionJson: "获取版本信息",
  assetIndex: "获取资源索引",
  files: "下载游戏文件",
};

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
      下载任务 · 共 {{ versions.jobList.length }} 楼
      <a class="extra" @click="collapsed = !collapsed">{{ collapsed ? "展开" : "收起" }}</a>
    </div>
    <div v-show="!collapsed" class="floors">
      <div v-for="job in versions.jobList" :key="job.versionId" class="floor">
        <div class="floor-head">
          <span class="floor-no">{{ job.floor }}#</span>
          <b>{{ job.versionId }}</b>
          <span v-if="job.status === 'running'" class="muted">{{ stageText[job.stage] }}</span>
          <span v-else-if="job.status === 'done'" class="ok">安装完成</span>
          <span v-else class="error-text">失败</span>
          <a v-if="job.status !== 'running'" class="close" title="删帖" @click="versions.dismiss(job.versionId)">×</a>
        </div>
        <div class="xp-bar"><div :style="{ width: percent(job) + '%' }" /></div>
        <div v-if="job.progress && job.status === 'running'" class="floor-meta muted">
          {{ job.progress.finishedFiles }}/{{ job.progress.totalFiles }} 个文件 ·
          {{ formatBytes(job.progress.downloadedBytes) }}/{{ formatBytes(job.progress.totalBytes) }} ·
          {{ formatBytes(job.progress.speed) }}/s
        </div>
        <div v-else-if="job.status === 'done' && job.report" class="floor-meta muted">
          新下载 {{ job.report.downloaded }} 个，已存在跳过 {{ job.report.skipped }} 个
        </div>
        <div v-else-if="job.error" class="floor-meta error-text">{{ job.error }}</div>
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
