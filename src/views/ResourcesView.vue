<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { CompatReport, LocalResource, Project, ResourceKind, ResourceSource, SearchPage, SortBy, TargetInfo } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { formatBytes, formatDate, loaderLabel } from "../utils";
import ResourceDialog from "../components/ResourceDialog.vue";
import CommunitySources from "../components/CommunitySources.vue";
import ModpackVersionDialog from "../components/ModpackVersionDialog.vue";
import { browseCommunity } from "../community";
import { errText, locale, t } from "../i18n";

const versions = useVersionsStore();
const route = useRoute();

const versionId = ref<string>("");
const kind = ref<ResourceKind>("mod");
const source = ref<ResourceSource | "klpbbs">("modrinth");
const tab = ref<"browse" | "installed" | "community">("browse");
const query = ref("");
const sort = ref<SortBy>("relevance");
const PAGE = 20;
const offset = ref(0);

const target = ref<TargetInfo | null>(null);
const page = ref<SearchPage | null>(null);
const searching = ref(false);
const error = ref<string | null>(null);
const dialog = ref<Project | null>(null);
const modpackDialog = ref<Project | null>(null);
// CurseForge 是否走镜像；整合包不选安装目标，沿用最近一次读到的值
const cfMirror = ref(false);

const local = ref<LocalResource[]>([]);
const localLoading = ref(false);
const compat = ref<Record<string, CompatReport>>({});
const checking = ref(false);

async function checkCompat() {
  checking.value = true;
  error.value = null;
  try {
    const list = await api.localCompat(versionId.value);
    compat.value = Object.fromEntries(list.map((c) => [c.filename, c.report]));
  } catch (e) {
    error.value = errText(e);
  } finally {
    checking.value = false;
  }
}

const kinds: ResourceKind[] = ["mod", "resourcepack", "shader", "modpack"];

// 整合包自带游戏版本：不选安装目标、不按已装版本筛选，下载后走整合包导入
const isModpack = computed(() => kind.value === "modpack");
const placeholder = computed(() => t(`resources.search.${kind.value}`));

async function openMemorial() {
  error.value = null;
  try {
    await browseCommunity("memorialResources");
  } catch (e) {
    error.value = errText(e);
  }
}

onMounted(async () => {
  if (!versions.installed.length) await versions.loadInstalled();
  // 从首页「管理模组」进来时带着版本；否则选第一个装了加载器的版本
  const wanted = route.query.version as string | undefined;
  versionId.value = (versions.installed.find((v) => v.id === wanted) ?? versions.installed.find((v) => v.loader) ?? versions.installed[0])?.id ?? "";
  if (!versionId.value) await search();
});

const isVanillaMod = computed(() => kind.value === "mod" && target.value && !target.value.loader);

watch([versionId, kind], async () => {
  if (isModpack.value) {
    if (tab.value === "installed") tab.value = "browse";
    target.value = null;
    offset.value = 0;
    await refresh();
    return;
  }
  if (!versionId.value) {
    target.value = null;
    offset.value = 0;
    await refresh();
    return;
  }
  error.value = null;
  try {
    target.value = await api.resourceTarget(versionId.value, kind.value);
    cfMirror.value = target.value.curseforgeViaMirror;
  } catch (e) {
    error.value = errText(e);
    return;
  }
  offset.value = 0;
  await refresh();
});

watch([source, sort], () => {
  offset.value = 0;
  search();
});

watch(tab, refresh);

async function refresh() {
  if (tab.value === "browse") await search();
  else if (tab.value === "installed") await loadLocal();
}

async function search() {
  if (tab.value !== "browse") return;
  searching.value = true;
  error.value = null;
  try {
    const searchQuery = {
      query: query.value,
      kind: kind.value,
      gameVersion: isModpack.value ? null : target.value?.mcVersion ?? null,
      loader: kind.value === "mod" ? target.value?.loader ?? null : null,
      sort: sort.value,
      offset: offset.value,
      limit: PAGE,
    };
    page.value = source.value === "klpbbs"
      ? await api.klpbbsSearch(searchQuery)
      : await api.resourceSearch(source.value, searchQuery);
  } catch (e) {
    error.value = errText(e);
    page.value = null;
  } finally {
    searching.value = false;
  }
}

function go(delta: number) {
  offset.value = Math.max(0, offset.value + delta * PAGE);
  search();
}

async function loadLocal() {
  if (!versionId.value) return;
  compat.value = {};
  localLoading.value = true;
  try {
    local.value = await api.localResources(versionId.value, kind.value);
  } catch (e) {
    error.value = errText(e);
  } finally {
    localLoading.value = false;
  }
}

async function toggle(r: LocalResource) {
  try {
    r.filename = await api.localResourceToggle(versionId.value, kind.value, r.filename, !r.enabled);
    r.enabled = !r.enabled;
  } catch (e) {
    error.value = errText(e);
  }
}

async function remove(r: LocalResource) {
  if (!confirm(t("resources.confirmDelete", { name: r.title ?? r.filename }))) return;
  try {
    await api.localResourceDelete(versionId.value, kind.value, r.filename);
    local.value = local.value.filter((x) => x !== r);
  } catch (e) {
    error.value = errText(e);
  }
}

function open(url: string) {
  if (inTauri) openUrl(url);
  else window.open(url, "_blank");
}

function openFolder() {
  if (target.value && inTauri) revealItemInDir(target.value.dir);
}

function formatCount(n: number) {
  if (locale.value !== "zh-CN") return new Intl.NumberFormat(locale.value, { notation: "compact", maximumFractionDigits: 1 }).format(n);
  return n >= 10000 ? `${(n / 10000).toFixed(n >= 1e6 ? 0 : 1)} 万` : String(n);
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      {{ $t("nav.resources") }}
      <span v-if="tab !== 'community' && !isModpack && versions.installed.length" class="extra target">
        {{ $t("resources.installTo") }}
        <select v-model="versionId" class="input small-select">
          <option v-for="v in versions.installed" :key="v.id" :value="v.id">
            {{ v.id }} ({{ v.loader ? loaderLabel[v.loader] : $t("common.vanilla") }}{{ v.mcVersion && v.mcVersion !== v.id ? ` · ${v.mcVersion}` : "" }})
          </option>
        </select>
        <template v-if="target">
          <span class="muted">MC {{ target.mcVersion }} · {{ target.loader ? loaderLabel[target.loader] : $t("common.vanilla") }}</span>
        </template>
      </span>
    </div>

    <div class="toolbar">
      <div v-if="tab !== 'community'" class="tabs">
        <a v-for="k in kinds" :key="k" :class="{ active: kind === k }" @click="kind = k">{{ $t(`resources.kind.${k}`) }}</a>
      </div>
      <div class="tabs">
        <a :class="{ active: tab === 'browse' }" @click="tab = 'browse'">{{ $t("resources.browse") }}</a>
        <a v-if="!isModpack" :class="{ active: tab === 'installed' }" @click="tab = 'installed'">{{ $t("download.installed") }}</a>
        <a :class="{ active: tab === 'community' }" @click="tab = 'community'">{{ $t("resources.community") }}</a>
      </div>
    </div>
    <CommunitySources v-if="tab === 'community'" />
    <div v-else-if="tab === 'installed' && !versions.installed.length" class="box-body muted">
      <i18n-t keypath="resources.noVersion"><RouterLink to="/download">{{ $t("nav.download") }}</RouterLink></i18n-t>
    </div>

    <template v-else>
      <div v-if="isVanillaMod" class="box-body">
        <div class="mod-tip">{{ $t("common.modTip") }}{{ $t("resources.vanillaMod", { id: versionId }) }}</div>
      </div>

      <!-- 找资源 -->
      <template v-if="tab === 'browse'">
        <form class="search" @submit.prevent="offset = 0; search()">
          <div class="tabs src">
            <a :class="{ active: source === 'modrinth' }" @click="source = 'modrinth'">Modrinth</a>
            <a :class="{ active: source === 'curseforge' }" @click="source = 'curseforge'">
              CurseForge<span v-if="cfMirror" class="muted mirror">{{ $t("resources.mirror") }}</span>
            </a>
            <a :class="{ active: source === 'klpbbs' }" @click="source = 'klpbbs'">{{ $t("resources.klpbbs") }}</a>
          </div>
          <input v-model="query" class="input grow" :placeholder="placeholder" />
          <select v-model="sort" class="input">
            <option value="relevance">{{ $t("resources.sort.relevance") }}</option>
            <option value="downloads">{{ $t("resources.sort.downloads") }}</option>
            <option value="updated">{{ $t("resources.sort.updated") }}</option>
          </select>
          <button class="btn primary">{{ $t("resources.searchBtn") }}</button>
        </form>

        <div v-if="isModpack" class="memorial">
          <span>{{ $t("resources.memorialAsk") }}</span>
          <button class="btn small primary" @click="openMemorial">{{ $t("resources.memorialGo") }} →</button>
          <span class="muted small">{{ $t("resources.memorialHint") }}</span>
        </div>

        <p v-if="error" class="box-body error-text">{{ error }}</p>
        <div v-else-if="searching && !page" class="box-body muted">{{ $t("resources.searching") }}</div>

        <table v-if="page" class="thread-table results" :class="{ dim: searching }">
          <tbody>
            <tr v-for="p in page.hits" :key="`${p.source}:${p.id}`">
              <td class="icon-cell">
                <img v-if="p.iconUrl" :src="p.iconUrl" alt="" loading="lazy" />
                <div v-else class="icon-placeholder">{{ p.title.slice(0, 1) }}</div>
              </td>
              <td>
                <a class="title" @click="open(p.pageUrl)">{{ p.title }}</a>
                <span class="muted by">{{ $t("themeImport.by", { author: p.author }) }}</span>
                <span v-if="source === 'klpbbs'" class="muted by">{{ p.source === "curseforge" ? "CurseForge" : "Modrinth" }}</span>
                <div class="desc">{{ p.description }}</div>
                <div class="cats">
                  <span v-for="c in p.categories.slice(0, 4)" :key="c" class="cat">{{ c }}</span>
                </div>
              </td>
              <td class="num stats">
                <div>⬇ {{ formatCount(p.downloads) }}</div>
                <div class="muted">{{ formatDate(p.updated) }}</div>
              </td>
              <td class="num">
                <button v-if="isModpack" class="btn small primary" @click="modpackDialog = p">{{ $t("resources.downloadImport") }}</button>
                <button v-else class="btn small primary" :disabled="!versionId || !!isVanillaMod" :title="!versionId ? $t('resources.needVersion') : undefined" @click="dialog = p">{{ $t("download.install") }}</button>
              </td>
            </tr>
            <tr v-if="!page.hits.length"><td colspan="4" class="muted">{{ $t("resources.noResult") }}</td></tr>
          </tbody>
        </table>
        <div v-if="page && page.total > PAGE" class="pager">
          <button class="btn small" :disabled="offset === 0 || searching" @click="go(-1)">{{ $t("resources.prev") }}</button>
          <span class="muted">{{ $t("resources.page", { i: offset / PAGE + 1, n: Math.ceil(page.total / PAGE), total: page.total }) }}</span>
          <button class="btn small" :disabled="offset + PAGE >= page.total || searching" @click="go(1)">{{ $t("resources.next") }}</button>
        </div>
      </template>

      <!-- 已安装 -->
      <template v-else>
        <div class="search">
          <span class="muted grow">{{ target?.dir }}</span>
          <button v-if="kind === 'mod' && target?.loader" class="btn small" :disabled="checking" @click="checkCompat">
            {{ checking ? $t("settings.checking") : $t("resources.checkCompat") }}
          </button>
          <button class="btn small" @click="loadLocal">{{ $t("common.refresh") }}</button>
          <button class="btn small" @click="openFolder">{{ $t("launchHome.openFolder") }}</button>
        </div>
        <p v-if="error" class="box-body error-text">{{ error }}</p>
        <div v-if="localLoading" class="box-body muted">{{ $t("resources.scanning") }}</div>
        <table v-else class="thread-table">
          <tbody>
            <tr v-for="r in local" :key="r.filename" :class="{ disabled: !r.enabled }">
              <td>
                <b>{{ r.title ?? r.filename }}</b>
                <span v-if="r.version" class="muted ver">{{ r.version }}</span>
                <span v-if="!r.enabled" class="tag off">{{ $t("resources.disabledTag") }}</span>
                <span v-if="compat[r.filename]" class="compat" :class="'st-' + compat[r.filename].status" :title="compat[r.filename].notes.join($t('common.listSep'))">
                  {{ compat[r.filename].status === "ok" ? `✔ ${$t("resource.compatOk")}` : compat[r.filename].status === "problem" ? `✘ ${$t("resource.compatBad")}` : `? ${$t("log.unknown")}` }}
                </span>
                <div v-for="p in compat[r.filename]?.problems ?? []" :key="p" class="problem">✘ {{ p }}</div>
                <div v-if="r.title" class="muted small">{{ r.filename }}</div>
              </td>
              <td class="num muted">{{ r.size ? formatBytes(r.size) : $t("buttons.folder") }}</td>
              <td class="num actions">
                <button class="btn small" @click="toggle(r)">{{ r.enabled ? $t("resources.disable") : $t("resources.enable") }}</button>
                <button class="btn small" @click="remove(r)">{{ $t("common.delete") }}</button>
              </td>
            </tr>
            <tr v-if="!local.length"><td colspan="3" class="muted">{{ $t("resources.empty") }}</td></tr>
          </tbody>
        </table>
      </template>
    </template>
  </div>

  <ResourceDialog
    v-if="dialog"
    :project="dialog"
    :kind="kind"
    :version-id="versionId"
    @close="dialog = null"
    @installed="tab === 'installed' && loadLocal()"
  />
  <ModpackVersionDialog v-if="modpackDialog" :project="modpackDialog" @close="modpackDialog = null" />
</template>

<style scoped>
.target { display: flex; align-items: center; gap: 6px; }
.small-select { padding: 2px 6px; font-size: 12px; max-width: 260px; }
.toolbar { display: flex; flex-wrap: wrap; gap: 8px; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid var(--border); }
.tabs { display: flex; }
.tabs a { padding: 4px 12px; border: 1px solid var(--border); margin-right: -1px; background: var(--bg-box-alt); color: var(--text); white-space: nowrap; }
.tabs a:hover { text-decoration: none; background: var(--bg-title); }
.tabs a.active { background: var(--bg-box); font-weight: bold; color: var(--brown-dark); }
.search { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px dashed var(--border); }
.grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mirror { font-size: 11px; font-weight: normal; }
.results.dim { opacity: 0.6; }
.icon-cell { width: 56px; }
.icon-cell img, .icon-placeholder { width: 44px; height: 44px; border-radius: 4px; border: 1px solid var(--border); display: block; object-fit: cover; }
.icon-placeholder { display: flex; align-items: center; justify-content: center; background: var(--bg-title); color: var(--brown); font-weight: bold; font-size: 18px; }
.title { font-weight: bold; font-size: 14px; }
.by { font-size: 12px; margin-left: 6px; }
.desc { margin: 3px 0; color: var(--text); }
.cats { display: flex; gap: 4px; flex-wrap: wrap; }
.cat { font-size: 11px; padding: 0 5px; border: 1px solid var(--border); border-radius: 2px; color: var(--text-muted); }
.stats { font-size: 12px; }
.pager { display: flex; justify-content: center; align-items: center; gap: 12px; padding: 10px; }
.actions { display: flex; gap: 6px; justify-content: flex-end; }
.small { font-size: 12px; }
tr.disabled b { color: var(--text-muted); text-decoration: line-through; }
.tag.off { background: #8b8b8b; margin-left: 6px; }
.compat { font-size: 11px; padding: 0 5px; border-radius: 2px; color: #fff; margin-left: 6px; cursor: help; }
.compat.st-ok { background: var(--grass); }
.compat.st-problem { background: var(--red); }
.compat.st-unknown { background: #8b8b8b; }
.problem { color: var(--red); font-size: 12px; }
.ver { margin-left: 6px; }
.memorial { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 10px 12px; border-bottom: 1px dashed var(--border); background: var(--bg-box-alt); }
</style>
