<script setup lang="ts">
import { onMounted, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { api, inTauri } from "../api";
import type { ExportEntry, PackFormat } from "../api/types";
import { useVersionsStore } from "../stores/versions";

const props = defineProps<{ versionId: string }>();
const emit = defineEmits<{ close: [] }>();
const versions = useVersionsStore();

const entries = ref<ExportEntry[]>([]);
const picked = ref<Set<string>>(new Set());
const format = ref<PackFormat>("modrinth");
const name = ref(props.versionId);
const version = ref("1.0.0");
const author = ref("");
const summary = ref("");
const error = ref<string | null>(null);

const formats: { key: PackFormat; label: string; desc: string; ext: string }[] = [
  { key: "modrinth", label: "Modrinth", desc: "Modrinth 上能找到的模组只写下载链接，包最小，通用性最好", ext: "mrpack" },
  { key: "curseforge", label: "CurseForge", desc: "CurseForge 上能找到的模组只写引用，可以在 CurseForge App 里导入", ext: "zip" },
  { key: "mcbbs", label: "MCBBS", desc: "所有文件都打包进去，离线可用，还会带上内存和启动参数设置", ext: "zip" },
];

onMounted(async () => {
  try {
    entries.value = await api.exportCandidates(props.versionId);
    picked.value = new Set(entries.value.filter((e) => e.defaultOn).map((e) => e.name));
  } catch (e) {
    error.value = String(e);
  }
});

function toggle(n: string) {
  const s = new Set(picked.value);
  s.has(n) ? s.delete(n) : s.add(n);
  picked.value = s;
}

async function start() {
  error.value = null;
  const f = formats.find((x) => x.key === format.value)!;
  const fileName = `${name.value.trim() || props.versionId}-${version.value.trim() || "1.0"}.${f.ext}`.replace(/[\\/:*?"<>|]/g, "_");
  if (!inTauri) {
    error.value = "浏览器预览模式不能导出，请用 npm run tauri dev 启动";
    return;
  }
  const out = await save({ defaultPath: fileName, filters: [{ name: `${f.label} 整合包`, extensions: [f.ext] }] });
  if (!out) return;
  const job = versions.startJob(`导出 ${fileName}`);
  emit("close");
  try {
    const r = await api.modpackExport(job.versionId, props.versionId, {
      format: format.value,
      name: name.value.trim(),
      version: version.value.trim(),
      author: author.value.trim(),
      summary: summary.value.trim(),
      include: [...picked.value],
    }, out);
    job.status = "done";
    job.doneText = `已导出到 ${r.path}（下载链接 ${r.linked} 个，打包 ${r.bundled} 个）`;
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
        导出整合包 · {{ versionId }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body body">
        <div class="formats">
          <label v-for="f in formats" :key="f.key" class="fmt" :class="{ active: format === f.key }">
            <input v-model="format" type="radio" :value="f.key" />
            <b>{{ f.label }}</b>
            <span class="muted">{{ f.desc }}</span>
          </label>
        </div>
        <div class="grid">
          <label>名称</label><input v-model="name" class="input" />
          <label>版本</label><input v-model="version" class="input" />
          <label>作者</label><input v-model="author" class="input" placeholder="可不填" />
          <label>简介</label><input v-model="summary" class="input" placeholder="可不填" />
        </div>
        <h4>包含哪些内容</h4>
        <div class="entries">
          <label v-for="e in entries" :key="e.name" class="entry">
            <input type="checkbox" :checked="picked.has(e.name)" @change="toggle(e.name)" />
            {{ e.isDir ? "📁" : "📄" }} {{ e.name }}
          </label>
        </div>
        <p class="muted small">日志、崩溃报告、启动器缓存不会被导出。存档默认不勾选。</p>
        <p v-if="error" class="error-text">{{ error }}</p>
      </div>
      <div class="actions">
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn primary" :disabled="!picked.size || !name.trim()" @click="start">选择保存位置并导出</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 620px; max-width: calc(100vw - 32px); margin: 0; display: flex; flex-direction: column; max-height: calc(100vh - 80px); }
.body { overflow-y: auto; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.formats { display: grid; gap: 6px; }
.fmt { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border: 1px solid var(--border); border-radius: var(--radius); cursor: pointer; background: var(--bg-box-alt); }
.fmt.active { border-color: var(--grass); background: rgba(94, 156, 53, 0.1); }
.fmt b { width: 90px; flex-shrink: 0; }
.fmt .muted { font-size: 12px; }
.grid { display: grid; grid-template-columns: 50px 1fr; gap: 6px 10px; align-items: center; margin-top: 12px; }
.grid label { color: var(--text-muted); text-align: right; }
h4 { margin: 14px 0 6px; color: var(--brown-dark); }
.entries { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 4px 10px; }
.entry { display: flex; align-items: center; gap: 4px; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.small { font-size: 12px; }
.actions { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
</style>
