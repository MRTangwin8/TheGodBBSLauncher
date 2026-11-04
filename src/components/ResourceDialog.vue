<script setup lang="ts">
import { ask } from "../utils";
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { InstallPlan, Project, ResourceKind, ResourceVersion } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { formatBytes, formatDate, releaseType } from "../utils";
import { errText, t } from "../i18n";

const props = defineProps<{ project: Project; kind: ResourceKind; versionId: string }>();
const emit = defineEmits<{ close: []; installed: [] }>();
const versions = useVersionsStore();

const list = ref<ResourceVersion[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const chosen = ref<ResourceVersion | null>(null);
const plan = ref<InstallPlan | null>(null);
const planning = ref(false);


function openProject() {
  if (inTauri) openUrl(props.project.pageUrl);
  else window.open(props.project.pageUrl, "_blank", "noopener,noreferrer");
}

onMounted(async () => {
  try {
    list.value = await api.resourceVersions(props.project.source, props.project.id, props.kind, props.versionId);
  } catch (e) {
    error.value = errText(e);
  } finally {
    loading.value = false;
  }
});

async function pick(v: ResourceVersion) {
  chosen.value = v;
  planning.value = true;
  error.value = null;
  try {
    plan.value = await api.resourcePlan(props.kind, props.versionId, v);
  } catch (e) {
    error.value = errText(e);
    chosen.value = null;
  } finally {
    planning.value = false;
  }
}

const totalSize = computed(() => plan.value?.items.reduce((s, i) => s + i.file.size, 0) ?? 0);
const hasProblem = computed(() => plan.value?.items.some((i) => i.compat?.status === "problem") ?? false);

async function confirm() {
  if (!plan.value) return;
  if (hasProblem.value && !(await ask(t("resource.confirmProblem")))) return;
  const p = plan.value;
  const jobId = `${props.project.title} → ${props.versionId}`;
  const job = versions.startJob(jobId);
  emit("close");
  try {
    const r = await api.resourceInstall(jobId, props.kind, props.versionId, p);
    job.report = r;
    job.status = r.failed.length ? "failed" : "done";
    if (r.failed.length) job.error = t("progress.filesFailed", { n: r.failed.length, reason: r.failed[0].reason });
    emit("installed");
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
        {{ plan ? $t("resource.confirm") : $t("resource.pickVersion") }} · {{ project.title }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>

      <div class="box-body body">
        <p v-if="error" class="error-text">{{ error }}</p>

        <!-- 第一步：选版本 -->
        <template v-if="!plan">
          <div v-if="loading" class="muted">{{ $t("resource.loading", { id: versionId }) }}</div>
          <div v-else-if="!list.length && !error" class="mod-tip">{{ $t("common.modTip") }}{{ $t("resource.none") }}</div>
          <table v-else class="thread-table">
            <tbody>
              <tr v-for="v in list" :key="v.id">
                <td>
                  <span class="tag" :class="'rt-' + v.releaseType">{{ releaseType(v.releaseType) }}</span>
                  <b>{{ v.versionNumber }}</b>
                  <div class="muted small">{{ v.gameVersions.slice(0, 6).join(", ") }} · {{ v.loaders.join(", ") || $t("resource.universal") }}</div>
                  <div v-if="!v.file" class="muted small">{{ $t("resource.noFile") }}</div>
                </td>
                <td class="num">{{ formatDate(v.published) }}</td>
                <td class="num">{{ v.file ? formatBytes(v.file.size) : "-" }}</td>
                <td class="num">
                  <button v-if="v.file" class="btn small primary" :disabled="planning" @click="pick(v)">
                    {{ planning && chosen?.id === v.id ? $t("resource.checking") : $t("resource.pick") }}
                  </button>
                  <button v-else class="btn small" @click="openProject">{{ $t("resource.projectPage") }}</button>
                </td>
              </tr>
            </tbody>
          </table>
        </template>

        <!-- 第二步：安装计划 -->
        <template v-else>
          <p class="muted small checked">{{ $t("resource.checked") }}</p>
          <div v-for="(item, i) in plan.items" :key="item.projectId" class="floor">
            <span class="floor-no">{{ i + 1 }}#</span>
            <span v-if="item.compat" class="compat" :class="'st-' + item.compat.status" :title="item.compat.notes.join($t('common.listSep'))">
              {{ item.compat.status === "ok" ? `✔ ${$t("resource.compatOk")}` : item.compat.status === "problem" ? `✘ ${$t("resource.compatBad")}` : `? ${$t("log.unknown")}` }}
            </span>
            <b>{{ item.title }}</b>
            <span class="muted">{{ item.versionNumber }}</span>
            <span v-if="item.requiredBy" class="tag dep">{{ $t("resource.requiredBy", { name: item.requiredBy }) }}</span>
            <span class="size muted">{{ formatBytes(item.file.size) }}</span>
            <div v-if="item.note" class="note">🔄 {{ item.note }}</div>
            <div v-if="item.compat?.status === 'problem'" class="problems">
              <div v-for="p in item.compat.problems" :key="p">✘ {{ p }}</div>
              <div v-if="item.alternative" class="alt">
                <i18n-t keypath="resource.suggest"><b>{{ item.alternative.versionNumber }}</b></i18n-t>
                <span v-if="item.alternative.releaseType !== 'release'" class="muted">{{ $t("resource.preTag", { type: releaseType(item.alternative.releaseType) }) }}</span>
                <button class="btn small primary" :disabled="planning" @click="pick(item.alternative)">{{ $t("resource.useThis") }}</button>
              </div>
            </div>
            <div v-else-if="item.compat?.notes.length" class="muted small">{{ item.compat.notes.join($t("common.listSep")) }}</div>
            <div v-if="item.replaces.length" class="muted small">{{ $t("resource.replaces", { list: item.replaces.join($t("common.listComma")) }) }}</div>
          </div>
          <div class="notes">
            <p v-if="plan.alreadyInstalled.length" class="ok">✔ {{ $t("resource.alreadyInstalled", { list: plan.alreadyInstalled.join($t("common.listComma")) }) }}</p>
            <p v-if="plan.unresolved.length" class="error-text">✘ {{ $t("resource.unresolved", { list: plan.unresolved.join($t("common.listComma")) }) }}</p>
            <p v-if="plan.incompatible.length" class="error-text">⚠ {{ $t("resource.incompatible", { list: plan.incompatible.join($t("common.listComma")) }) }}</p>
            <p v-if="plan.optional.length" class="muted">{{ $t("resource.optional", { list: plan.optional.join($t("common.listComma")) }) }}</p>
          </div>
        </template>
      </div>

      <div v-if="plan" class="actions">
        <span class="muted">{{ $t("resource.total", { n: plan.items.length, size: formatBytes(totalSize) }) }}</span>
        <button class="btn" @click="plan = null; chosen = null">{{ $t("common.back") }}</button>
        <button class="btn primary" :class="{ danger: hasProblem }" :disabled="!plan.items.length" @click="confirm">
          {{ hasProblem ? $t("resource.installAnyway") : $t("resource.confirm") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 680px; max-width: calc(100vw - 32px); margin: 0; display: flex; flex-direction: column; max-height: calc(100vh - 80px); }
.body { overflow-y: auto; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.small { font-size: 12px; }
.tag.rt-release { background: var(--grass); }
.tag.rt-beta { background: #c47d1a; }
.tag.rt-alpha { background: var(--red); }
.tag.dep { background: var(--link); margin-left: 6px; }
.floor { padding: 8px 4px; border-bottom: 1px dashed var(--border); display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.floor-no { color: var(--text-muted); font-family: var(--font-pixel); }
.floor .size { margin-left: auto; }
.floor > div { width: 100%; padding-left: 26px; }
.notes p { margin: 6px 0; }
.compat { font-size: 11px; padding: 0 5px; border-radius: 2px; color: #fff; cursor: help; }
.compat.st-ok { background: var(--grass); }
.compat.st-problem { background: var(--red); }
.compat.st-unknown { background: #8b8b8b; }
.note { color: var(--link); font-size: 12px; }
.problems { color: var(--red); font-size: 12px; }
.alt { margin-top: 4px; display: flex; align-items: center; gap: 6px; color: var(--text); }
.checked { margin: 0 0 6px; }
.btn.danger { background: linear-gradient(#e0705f, var(--red)); border-color: #8f2a1f; }
.ok { color: var(--grass); }
.actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
.actions .muted { margin-right: auto; }
</style>
