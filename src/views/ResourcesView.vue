<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { CompatReport, LocalResource, Project, ResourceKind, ResourceSource, SearchPage, SortBy, TargetInfo } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { formatBytes, formatDate, loaderLabel } from "../utils";
import ResourceDialog from "../components/ResourceDialog.vue";

const versions = useVersionsStore();

const versionId = ref<string>("");
const kind = ref<ResourceKind>("mod");
const source = ref<ResourceSource>("modrinth");
const tab = ref<"browse" | "installed">("browse");
const query = ref("");
const sort = ref<SortBy>("relevance");
const PAGE = 20;
const offset = ref(0);

const target = ref<TargetInfo | null>(null);
const page = ref<SearchPage | null>(null);
const searching = ref(false);
const error = ref<string | null>(null);
const dialog = ref<Project | null>(null);

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
    error.value = String(e);
  } finally {
    checking.value = false;
  }
}

const kinds: { key: ResourceKind; label: string }[] = [
  { key: "mod", label: "模组" },
  { key: "resourcepack", label: "资源包" },
  { key: "shader", label: "光影" },
];

onMounted(async () => {
  if (!versions.installed.length) await versions.loadInstalled();
  // 默认选第一个装了加载器的版本，没有就选第一个
  versionId.value = (versions.installed.find((v) => v.loader) ?? versions.installed[0])?.id ?? "";
});

const isVanillaMod = computed(() => kind.value === "mod" && target.value && !target.value.loader);

watch([versionId, kind], async () => {
  if (!versionId.value) return;
  error.value = null;
  try {
    target.value = await api.resourceTarget(versionId.value, kind.value);
  } catch (e) {
    error.value = String(e);
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
  else await loadLocal();
}

async function search() {
  if (!target.value || tab.value !== "browse") return;
  searching.value = true;
  error.value = null;
  try {
    page.value = await api.resourceSearch(source.value, {
      query: query.value,
      kind: kind.value,
      gameVersion: target.value.mcVersion,
      loader: kind.value === "mod" ? target.value.loader : null,
      sort: sort.value,
      offset: offset.value,
      limit: PAGE,
    });
  } catch (e) {
    error.value = String(e);
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
    error.value = String(e);
  } finally {
    localLoading.value = false;
  }
}

async function toggle(r: LocalResource) {
  try {
    r.filename = await api.localResourceToggle(versionId.value, kind.value, r.filename, !r.enabled);
    r.enabled = !r.enabled;
  } catch (e) {
    error.value = String(e);
  }
}

async function remove(r: LocalResource) {
  if (!confirm(`确定删除 ${r.title ?? r.filename} 吗？文件会被直接删除。`)) return;
  try {
    await api.localResourceDelete(versionId.value, kind.value, r.filename);
    local.value = local.value.filter((x) => x !== r);
  } catch (e) {
    error.value = String(e);
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
  return n >= 10000 ? `${(n / 10000).toFixed(n >= 1e6 ? 0 : 1)} 万` : String(n);
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      资源中心
      <span class="extra target">
        安装到
        <select v-model="versionId" class="input small-select">
          <option v-for="v in versions.installed" :key="v.id" :value="v.id">
            {{ v.id }}（{{ v.loader ? loaderLabel[v.loader] : "原版" }}{{ v.mcVersion && v.mcVersion !== v.id ? ` · ${v.mcVersion}` : "" }}）
          </option>
        </select>
        <template v-if="target">
          <span class="muted">MC {{ target.mcVersion }} · {{ target.loader ? loaderLabel[target.loader] : "原版" }}</span>
        </template>
      </span>
    </div>

    <div v-if="!versions.installed.length" class="box-body muted">
      还没有安装任何版本，先去 <RouterLink to="/download">下载中心</RouterLink> 装一个吧。
    </div>

    <template v-else>
      <div class="toolbar">
        <div class="tabs">
          <a v-for="k in kinds" :key="k.key" :class="{ active: kind === k.key }" @click="kind = k.key">{{ k.label }}</a>
        </div>
        <div class="tabs">
          <a :class="{ active: tab === 'browse' }" @click="tab = 'browse'">找资源</a>
          <a :class="{ active: tab === 'installed' }" @click="tab = 'installed'">已安装</a>
        </div>
      </div>

      <div v-if="isVanillaMod" class="box-body">
        <div class="mod-tip">【版主提示】{{ versionId }} 是原版，装模组需要先在「下载中心」给它装上 Fabric / Forge 等加载器。</div>
      </div>

      <!-- 找资源 -->
      <template v-if="tab === 'browse'">
        <form class="search" @submit.prevent="offset = 0; search()">
          <div class="tabs src">
            <a :class="{ active: source === 'modrinth' }" @click="source = 'modrinth'">Modrinth</a>
            <a :class="{ active: source === 'curseforge' }" @click="source = 'curseforge'">
              CurseForge<span v-if="target?.curseforgeViaMirror" class="muted mirror">（镜像）</span>
            </a>
          </div>
          <input v-model="query" class="input grow" placeholder="搜索模组、资源包、光影…" />
          <select v-model="sort" class="input">
            <option value="relevance">相关度</option>
            <option value="downloads">下载量</option>
            <option value="updated">最近更新</option>
          </select>
          <button class="btn primary">搜索</button>
        </form>

        <p v-if="error" class="box-body error-text">{{ error }}</p>
        <div v-else-if="searching && !page" class="box-body muted">搜索中…</div>

        <table v-if="page" class="thread-table results" :class="{ dim: searching }">
          <tbody>
            <tr v-for="p in page.hits" :key="p.id">
              <td class="icon-cell">
                <img v-if="p.iconUrl" :src="p.iconUrl" alt="" loading="lazy" />
                <div v-else class="icon-placeholder">{{ p.title.slice(0, 1) }}</div>
              </td>
              <td>
                <a class="title" @click="open(p.pageUrl)">{{ p.title }}</a>
                <span class="muted by">by {{ p.author }}</span>
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
                <button class="btn small primary" :disabled="!!isVanillaMod" @click="dialog = p">安装</button>
              </td>
            </tr>
            <tr v-if="!page.hits.length"><td colspan="4" class="muted">没有找到相关资源</td></tr>
          </tbody>
        </table>
        <div v-if="page && page.total > PAGE" class="pager">
          <button class="btn small" :disabled="offset === 0 || searching" @click="go(-1)">上一页</button>
          <span class="muted">第 {{ offset / PAGE + 1 }} / {{ Math.ceil(page.total / PAGE) }} 页 · 共 {{ page.total }} 个</span>
          <button class="btn small" :disabled="offset + PAGE >= page.total || searching" @click="go(1)">下一页</button>
        </div>
      </template>

      <!-- 已安装 -->
      <template v-else>
        <div class="search">
          <span class="muted grow">{{ target?.dir }}</span>
          <button v-if="kind === 'mod' && target?.loader" class="btn small" :disabled="checking" @click="checkCompat">
            {{ checking ? "检查中…" : "检查兼容性" }}
          </button>
          <button class="btn small" @click="loadLocal">刷新</button>
          <button class="btn small" @click="openFolder">打开文件夹</button>
        </div>
        <p v-if="error" class="box-body error-text">{{ error }}</p>
        <div v-if="localLoading" class="box-body muted">正在扫描并识别…</div>
        <table v-else class="thread-table">
          <tbody>
            <tr v-for="r in local" :key="r.filename" :class="{ disabled: !r.enabled }">
              <td>
                <b>{{ r.title ?? r.filename }}</b>
                <span v-if="r.version" class="muted ver">{{ r.version }}</span>
                <span v-if="!r.enabled" class="tag off">已禁用</span>
                <span v-if="compat[r.filename]" class="compat" :class="'st-' + compat[r.filename].status" :title="compat[r.filename].notes.join('；')">
                  {{ compat[r.filename].status === "ok" ? "✔ 兼容" : compat[r.filename].status === "problem" ? "✘ 不兼容" : "? 未知" }}
                </span>
                <div v-for="p in compat[r.filename]?.problems ?? []" :key="p" class="problem">✘ {{ p }}</div>
                <div v-if="r.title" class="muted small">{{ r.filename }}</div>
              </td>
              <td class="num muted">{{ r.size ? formatBytes(r.size) : "文件夹" }}</td>
              <td class="num actions">
                <button class="btn small" @click="toggle(r)">{{ r.enabled ? "禁用" : "启用" }}</button>
                <button class="btn small" @click="remove(r)">删除</button>
              </td>
            </tr>
            <tr v-if="!local.length"><td colspan="3" class="muted">这里还是空的</td></tr>
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
</template>

<style scoped>
.target { display: flex; align-items: center; gap: 6px; }
.small-select { padding: 2px 6px; font-size: 12px; max-width: 260px; }
.toolbar { display: flex; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid var(--border); }
.tabs { display: flex; }
.tabs a { padding: 4px 12px; border: 1px solid var(--border); margin-right: -1px; background: var(--bg-box-alt); color: var(--text); white-space: nowrap; }
.tabs a:hover { text-decoration: none; background: var(--bg-title); }
.tabs a.active { background: var(--bg-box); font-weight: bold; color: var(--brown-dark); }
.search { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px dashed var(--border); }
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
</style>
