<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { api } from "../api";
import type { ModpackPreview } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { loaderLabel } from "../utils";

const props = defineProps<{ path: string }>();
const emit = defineEmits<{ close: [] }>();
const versions = useVersionsStore();
const router = useRouter();

const preview = ref<ModpackPreview | null>(null);
const error = ref<string | null>(null);
const name = ref("");

const formatLabel = { modrinth: "Modrinth（.mrpack）", curseforge: "CurseForge", mcbbs: "MCBBS" };
const fileName = computed(() => props.path.split(/[\\/]/).pop());

onMounted(async () => {
  try {
    preview.value = await api.modpackPreview(props.path);
    name.value = preview.value.suggestedName;
  } catch (e) {
    error.value = String(e);
  }
});

const nameError = computed(() => {
  const n = name.value.trim();
  if (!n) return "版本名不能为空";
  if (/[\\/:*?"<>|]/.test(n)) return '不能包含 \\ / : * ? " < > |';
  if (versions.installed.some((v) => v.id === n)) return "已经有同名版本了";
  return null;
});

async function start() {
  const n = name.value.trim();
  const job = versions.startJob(`整合包 ${n}`);
  job.stage = "versionJson";
  emit("close");
  router.push("/download");
  try {
    job.resultId = await api.modpackImport(job.versionId, props.path, n);
    job.status = "done";
  } catch (e) {
    job.status = "failed";
    job.error = String(e);
  }
  await versions.loadInstalled();
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        导入整合包
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body">
        <p class="muted small">{{ fileName }}</p>
        <p v-if="error" class="error-text">{{ error }}</p>
        <div v-else-if="!preview" class="muted">正在读取整合包…</div>
        <template v-else>
          <h3 class="pack-name">
            {{ preview.info.name || "未命名整合包" }} <span class="muted">{{ preview.info.version }}</span>
          </h3>
          <p v-if="preview.info.summary" class="summary">{{ preview.info.summary }}</p>
          <dl class="facts">
            <dt>格式</dt><dd>{{ formatLabel[preview.info.format] }}</dd>
            <dt>游戏</dt>
            <dd>
              Minecraft {{ preview.info.mc }}
              <template v-if="preview.info.loader"> + {{ loaderLabel[preview.info.loader] }} {{ preview.info.loaderVersion }}</template>
            </dd>
            <dt v-if="preview.info.author">作者</dt><dd v-if="preview.info.author">{{ preview.info.author }}</dd>
            <dt>文件</dt><dd>需下载 {{ preview.info.fileCount }} 个，包内自带 {{ preview.info.overrideCount }} 个</dd>
            <dt v-if="preview.info.minMemoryMb">内存</dt><dd v-if="preview.info.minMemoryMb">至少 {{ preview.info.minMemoryMb }} MB</dd>
          </dl>
          <div v-for="w in preview.info.warnings" :key="w" class="mod-tip warn">【版主提示】{{ w }}</div>
          <div class="name-row">
            <label>版本名称</label>
            <input v-model="name" class="input" />
          </div>
          <p v-if="nameError" class="error-text small">{{ nameError }}</p>
          <p v-else class="muted small">会单独放在 versions/{{ name.trim() }}，存档和模组与其它版本互不影响</p>
        </template>
      </div>
      <div class="actions">
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn primary" :disabled="!preview || !!nameError" @click="start">开始导入</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 560px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.small { font-size: 12px; margin: 4px 0; }
.pack-name { font-size: 18px; color: var(--brown-dark); margin: 4px 0; }
.summary { margin: 4px 0 10px; }
.facts { display: grid; grid-template-columns: 60px 1fr; gap: 4px 10px; margin: 8px 0; padding: 8px 10px; background: var(--bg-box-alt); border-radius: var(--radius); }
.facts dt { color: var(--text-muted); }
.facts dd { margin: 0; }
.warn { margin: 8px 0; }
.name-row { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
.name-row label { color: var(--text-muted); white-space: nowrap; }
.name-row .input { flex: 1; font-family: var(--font-pixel); }
.actions { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
</style>
