<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useVersionsStore } from "../stores/versions";
import type { VersionKind } from "../api/types";
import { formatDate, kindLabel } from "../utils";
import InstallDialog from "../components/InstallDialog.vue";
import { open } from "@tauri-apps/plugin-dialog";
import { inTauri } from "../api";
import { useModpackStore } from "../stores/modpack";
import { errText, t } from "../i18n";

const versions = useVersionsStore();
const dialogFor = ref<string | null>(null);
const modpack = useModpackStore();

async function pickModpack() {
  if (!inTauri) {
    // 浏览器预览：直接用假路径看对话框
    modpack.importPath = "C:/Downloads/Fabulously.Optimized-v6.5.0.mrpack";
    return;
  }
  const file = await open({ multiple: false, filters: [{ name: t("common.modpack"), extensions: ["mrpack", "zip"] }] });
  if (typeof file === "string") modpack.importPath = file;
}
const filter = ref<VersionKind | "old">("release");
const keyword = ref("");
const PAGE = 50;
const limit = ref(PAGE);

onMounted(() => versions.manifest || versions.loadManifest());

const tabs = computed<{ key: VersionKind | "old"; label: string }[]>(() => [
  { key: "release", label: t("versionKind.release") },
  { key: "snapshot", label: t("versionKind.snapshot") },
  { key: "old", label: t("download.old") },
]);

const filtered = computed(() => {
  const list = versions.manifest?.versions ?? [];
  const kw = keyword.value.trim().toLowerCase();
  return list.filter((v) => {
    const kindOk = filter.value === "old" ? v.type.startsWith("old_") : v.type === filter.value;
    return kindOk && (!kw || v.id.toLowerCase().includes(kw));
  });
});

function switchTab(key: VersionKind | "old") {
  filter.value = key;
  limit.value = PAGE;
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      {{ $t("nav.download") }}
      <span class="extra">
        <template v-if="versions.manifest">
          <i18n-t keypath="download.latest"><b>{{ versions.manifest.latest.release }}</b><b>{{ versions.manifest.latest.snapshot }}</b></i18n-t>
        </template>
      </span>
    </div>
    <div class="toolbar">
      <div class="tabs">
        <a v-for="tab in tabs" :key="tab.key" :class="{ active: filter === tab.key }" @click="switchTab(tab.key)">{{ tab.label }}</a>
      </div>
      <input v-model="keyword" class="input" :placeholder="$t('download.search')" @input="limit = PAGE" />
      <button class="btn" @click="versions.loadManifest(true)">{{ $t("common.refresh") }}</button>
      <button class="btn primary" @click="pickModpack">{{ $t("download.importModpack") }}</button>
    </div>

    <div v-if="versions.manifestError" class="box-body">
      <div class="mod-tip">{{ $t("common.modTip") }}{{ $t("download.manifestFailed", { err: errText(versions.manifestError) }) }}</div>
    </div>
    <div v-else-if="!versions.manifest" class="box-body muted">{{ $t("download.loading") }}</div>

    <table v-else class="thread-table">
      <thead>
        <tr><th>{{ $t("common.thread") }}</th><th class="num">{{ $t("common.released") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
      </thead>
      <tbody>
        <tr v-for="v in filtered.slice(0, limit)" :key="v.id">
          <td>
            <span class="tag" :class="v.type">{{ kindLabel(v.type) }}</span>
            <b>{{ v.id }}</b>
            <span v-if="versions.installedIds.has(v.id)" class="tag installed">{{ $t("download.installed") }}</span>
            <span v-if="v.id === versions.manifest.latest.release" class="hot">{{ $t("download.new") }}</span>
          </td>
          <td class="num">{{ formatDate(v.releaseTime) }}</td>
          <td class="num">
            <span class="row-actions">
              <button
                v-if="versions.installedIds.has(v.id)"
                class="btn small"
                :disabled="versions.jobs[v.id]?.status === 'running'"
                @click="versions.install(v.id)"
              >
                {{ $t("common.completeFiles") }}
              </button>
              <button
                class="btn small primary"
                :disabled="versions.jobs[v.id]?.status === 'running'"
                @click="dialogFor = v.id"
              >
                {{ versions.jobs[v.id]?.status === "running" ? $t("download.installing") : versions.installedIds.has(v.id) ? $t("download.addLoader") : $t("download.install") }}
              </button>
            </span>
          </td>
        </tr>
        <tr v-if="!filtered.length"><td colspan="3" class="muted">{{ $t("download.noResult") }}</td></tr>
      </tbody>
    </table>
    <div v-if="filtered.length > limit" class="more">
      <button class="btn" @click="limit += PAGE">{{ $t("download.more", { n: filtered.length - limit }) }}</button>
    </div>
  </div>
  <InstallDialog v-if="dialogFor" :mc="dialogFor" @close="dialogFor = null" />
</template>

<style scoped>
.toolbar { display: flex; gap: 8px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--border); }
.tabs { display: flex; margin-right: auto; }
.tabs a { padding: 4px 12px; border: 1px solid var(--border); margin-right: -1px; background: var(--bg-box-alt); color: var(--text); }
.tabs a:hover { text-decoration: none; background: var(--bg-title); }
.tabs a.active { background: var(--bg-box); font-weight: bold; color: var(--brown-dark); border-bottom-color: var(--bg-box); }
.hot { color: var(--red); font-weight: bold; font-size: 12px; margin-left: 4px; }
.more { padding: 10px; text-align: center; }
.row-actions { display: inline-flex; gap: 6px; }
</style>
