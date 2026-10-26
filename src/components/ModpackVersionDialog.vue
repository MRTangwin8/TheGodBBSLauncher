<script setup lang="ts">
import { onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { Project, ResourceVersion } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { useModpackStore } from "../stores/modpack";
import { formatBytes, formatDate } from "../utils";

// 资源中心选整合包版本：下载到缓存后，交给和拖进窗口一样的导入对话框（预览、起名、安装）
const props = defineProps<{ project: Project }>();
const emit = defineEmits<{ close: [] }>();
const versions = useVersionsStore();
const modpack = useModpackStore();

const list = ref<ResourceVersion[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

const typeLabel: Record<string, string> = { release: "正式", beta: "测试", alpha: "开发" };

onMounted(async () => {
  try {
    list.value = await api.modpackVersions(props.project.source, props.project.id);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
});

function openProject() {
  if (inTauri) openUrl(props.project.pageUrl);
  else window.open(props.project.pageUrl, "_blank", "noopener,noreferrer");
}

async function download(v: ResourceVersion) {
  const jobId = `整合包 ${props.project.title} ${v.versionNumber}`;
  const job = versions.startJob(jobId);
  emit("close");
  try {
    const path = await api.modpackDownload(jobId, v);
    versions.dismiss(jobId);
    modpack.importPath = path;
  } catch (e) {
    job.status = "failed";
    job.error = String(e);
  }
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        选择整合包版本 · {{ project.title }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body body">
        <p class="muted small hint">整合包自带游戏版本和加载器，导入时会单独创建一个版本，不影响已有的游戏。</p>
        <p v-if="error" class="error-text">{{ error }}</p>
        <div v-else-if="loading" class="muted">正在获取版本列表…</div>
        <div v-else-if="!list.length" class="mod-tip">【版主提示】这个整合包还没有可下载的版本</div>
        <table v-else class="thread-table">
          <tbody>
            <tr v-for="v in list" :key="v.id">
              <td>
                <span class="tag" :class="'rt-' + v.releaseType">{{ typeLabel[v.releaseType] ?? v.releaseType }}</span>
                <b>{{ v.versionNumber }}</b>
                <div class="muted small">
                  MC {{ v.gameVersions.slice(0, 3).join(", ") || "未知" }}<template v-if="v.loaders.length"> · {{ v.loaders.join(", ") }}</template>
                </div>
                <div v-if="!v.file" class="muted small">作者限制了第三方下载，需要到项目页面手动下载</div>
              </td>
              <td class="num">{{ formatDate(v.published) }}</td>
              <td class="num">{{ v.file ? formatBytes(v.file.size) : "-" }}</td>
              <td class="num">
                <button v-if="v.file" class="btn small primary" @click="download(v)">下载并导入</button>
                <button v-else class="btn small" @click="openProject">项目页面</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 640px; max-width: calc(100vw - 32px); margin: 0; display: flex; flex-direction: column; max-height: calc(100vh - 80px); }
.body { overflow-y: auto; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.small { font-size: 12px; }
.hint { margin: 0 0 8px; }
.tag.rt-release { background: var(--grass); }
.tag.rt-beta { background: #c47d1a; }
.tag.rt-alpha { background: var(--red); }
</style>
