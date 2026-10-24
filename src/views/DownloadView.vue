<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useVersionsStore } from "../stores/versions";
import type { VersionKind } from "../api/types";
import { formatDate, kindLabel } from "../utils";
import InstallDialog from "../components/InstallDialog.vue";
import { open } from "@tauri-apps/plugin-dialog";
import { inTauri } from "../api";
import { useModpackStore } from "../stores/modpack";

const versions = useVersionsStore();
const dialogFor = ref<string | null>(null);
const modpack = useModpackStore();

async function pickModpack() {
  if (!inTauri) {
    // 浏览器预览：直接用假路径看对话框
    modpack.importPath = "C:/Downloads/Fabulously.Optimized-v6.5.0.mrpack";
    return;
  }
  const file = await open({ multiple: false, filters: [{ name: "整合包", extensions: ["mrpack", "zip"] }] });
  if (typeof file === "string") modpack.importPath = file;
}
const filter = ref<VersionKind | "old">("release");
const keyword = ref("");
const PAGE = 50;
const limit = ref(PAGE);

onMounted(() => versions.manifest || versions.loadManifest());

const tabs: { key: VersionKind | "old"; label: string }[] = [
  { key: "release", label: "正式版" },
  { key: "snapshot", label: "快照" },
  { key: "old", label: "远古版本" },
];

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
      下载中心
      <span class="extra">
        <template v-if="versions.manifest">
          最新正式版 <b>{{ versions.manifest.latest.release }}</b> · 最新快照 <b>{{ versions.manifest.latest.snapshot }}</b>
        </template>
      </span>
    </div>
    <div class="toolbar">
      <div class="tabs">
        <a v-for="t in tabs" :key="t.key" :class="{ active: filter === t.key }" @click="switchTab(t.key)">{{ t.label }}</a>
      </div>
      <input v-model="keyword" class="input" placeholder="搜索版本号…" @input="limit = PAGE" />
      <button class="btn" @click="versions.loadManifest(true)">刷新</button>
      <button class="btn primary" @click="pickModpack">导入整合包</button>
    </div>

    <div v-if="versions.manifestError" class="box-body">
      <div class="mod-tip">【版主提示】版本列表获取失败：{{ versions.manifestError }}</div>
    </div>
    <div v-else-if="!versions.manifest" class="box-body muted">正在加载版本列表…</div>

    <table v-else class="thread-table">
      <thead>
        <tr><th>主题</th><th class="num">发布时间</th><th class="num">操作</th></tr>
      </thead>
      <tbody>
        <tr v-for="v in filtered.slice(0, limit)" :key="v.id">
          <td>
            <span class="tag" :class="v.type">{{ kindLabel[v.type] }}</span>
            <b>{{ v.id }}</b>
            <span v-if="versions.installedIds.has(v.id)" class="tag installed">已安装</span>
            <span v-if="v.id === versions.manifest.latest.release" class="hot">新</span>
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
                补全文件
              </button>
              <button
                class="btn small primary"
                :disabled="versions.jobs[v.id]?.status === 'running'"
                @click="dialogFor = v.id"
              >
                {{ versions.jobs[v.id]?.status === "running" ? "安装中…" : versions.installedIds.has(v.id) ? "装加载器" : "安装" }}
              </button>
            </span>
          </td>
        </tr>
        <tr v-if="!filtered.length"><td colspan="3" class="muted">没有找到相关主题</td></tr>
      </tbody>
    </table>
    <div v-if="filtered.length > limit" class="more">
      <button class="btn" @click="limit += PAGE">加载更多（还有 {{ filtered.length - limit }} 帖）</button>
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
