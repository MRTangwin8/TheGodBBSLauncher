<script setup lang="ts">
import { onMounted, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { api, inTauri } from "../api";
import type { ExportEntry, PackFormat } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { errText, t } from "../i18n";

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

const formats: { key: PackFormat; label: string; ext: string }[] = [
  { key: "modrinth", label: "Modrinth", ext: "mrpack" },
  { key: "curseforge", label: "CurseForge", ext: "zip" },
  { key: "mcbbs", label: "MCBBS", ext: "zip" },
];

onMounted(async () => {
  try {
    entries.value = await api.exportCandidates(props.versionId);
    picked.value = new Set(entries.value.filter((e) => e.defaultOn).map((e) => e.name));
  } catch (e) {
    error.value = errText(e);
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
    error.value = t("export.previewOnly");
    return;
  }
  const out = await save({ defaultPath: fileName, filters: [{ name: t("export.filter", { format: f.label }), extensions: [f.ext] }] });
  if (!out) return;
  const job = versions.startJob(t("export.job", { file: fileName }), false);
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
    job.doneText = t("export.done", { path: r.path, linked: r.linked, bundled: r.bundled });
  } catch (e) {
    job.status = "failed";
    job.error = errText(e);
  }
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        {{ $t("export.title", { id: versionId }) }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body body">
        <div class="formats">
          <label v-for="f in formats" :key="f.key" class="fmt" :class="{ active: format === f.key }">
            <input v-model="format" type="radio" :value="f.key" />
            <b>{{ f.label }}</b>
            <span class="muted">{{ $t(`export.desc.${f.key}`) }}</span>
          </label>
        </div>
        <div class="grid">
          <label>{{ $t("export.name") }}</label><input v-model="name" class="input" />
          <label>{{ $t("export.version") }}</label><input v-model="version" class="input" />
          <label>{{ $t("export.author") }}</label><input v-model="author" class="input" :placeholder="$t('common.optional')" />
          <label>{{ $t("export.summary") }}</label><input v-model="summary" class="input" :placeholder="$t('common.optional')" />
        </div>
        <h4>{{ $t("export.include") }}</h4>
        <div class="entries">
          <label v-for="e in entries" :key="e.name" class="entry">
            <input type="checkbox" :checked="picked.has(e.name)" @change="toggle(e.name)" />
            {{ e.isDir ? "📁" : "📄" }} {{ e.name }}
          </label>
        </div>
        <p class="muted small">{{ $t("export.excluded") }}</p>
        <p v-if="error" class="error-text">{{ error }}</p>
      </div>
      <div class="actions">
        <button class="btn" @click="emit('close')">{{ $t("common.cancel") }}</button>
        <button class="btn primary" :disabled="!picked.size || !name.trim()" @click="start">{{ $t("export.start") }}</button>
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
