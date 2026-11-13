<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api } from "../../api";
import type { LocalResource } from "../../api/types";
import { ask, formatBytes } from "../../utils";
import { errText, t } from "../../i18n";

// 版本管理 → 资源包 / 光影：列出这个版本的 resourcepacks / shaderpacks 文件夹，可以禁用、删除。
// 禁用就是给文件名加 .disabled，游戏里就看不到它了；启用哪个资源包、光影还是在游戏里选
const props = defineProps<{ versionId: string; gameDir: string; kind: "resourcepack" | "shader"; running: boolean }>();
const emit = defineEmits<{ message: [ok: boolean, text: string]; openFolder: [path: string] }>();

const items = ref<LocalResource[] | null>(null);
const busy = ref(false);
const filter = ref("");
const folder = computed(() => `${props.gameDir}${props.gameDir.includes("\\") ? "\\" : "/"}${props.kind === "shader" ? "shaderpacks" : "resourcepacks"}`);
const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return (items.value ?? []).filter((m) => !q || m.filename.toLowerCase().includes(q) || m.title?.toLowerCase().includes(q));
});

async function load() {
  busy.value = true;
  try {
    items.value = await api.localResources(props.versionId, props.kind);
  } catch (e) {
    items.value = [];
    emit("message", false, errText(e));
  } finally {
    busy.value = false;
  }
}
// 资源包和光影共用这个组件，切换标签时要重新读
watch(() => [props.versionId, props.kind], () => { items.value = null; filter.value = ""; load(); }, { immediate: true });

async function toggle(m: LocalResource) {
  try {
    m.filename = await api.localResourceToggle(props.versionId, props.kind, m.filename, !m.enabled);
    m.enabled = !m.enabled;
  } catch (e) {
    emit("message", false, errText(e));
  }
}

async function remove(m: LocalResource) {
  if (!(await ask(t("instance.confirmDeleteMod", { name: m.title ?? m.filename })))) return;
  try {
    await api.localResourceDelete(props.versionId, props.kind, m.filename);
    items.value = (items.value ?? []).filter((x) => x !== m);
  } catch (e) {
    emit("message", false, errText(e));
  }
}
</script>

<template>
  <div class="box-body bar">
    <input v-model="filter" class="input grow" :placeholder="$t(`instance.packs.search.${kind}`)" />
    <button class="btn small" :disabled="busy" @click="load">{{ $t("common.refresh") }}</button>
    <button class="btn small" @click="emit('openFolder', folder)">{{ $t("instance.packs.openFolder") }}</button>
    <RouterLink :to="{ path: '/resources', query: { version: versionId, kind } }" class="btn small primary">{{ $t(`instance.packs.download.${kind}`) }}</RouterLink>
  </div>
  <div class="box-body hint muted small">{{ $t(`instance.packs.hint.${kind}`) }}</div>
  <table class="thread-table">
    <thead>
      <tr>
        <th>{{ $t(`instance.packs.header.${kind}`, { n: items?.length ?? 0 }) }}</th>
        <th class="num">{{ $t("instance.size") }}</th>
        <th class="num">{{ $t("common.actions") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="m in shown" :key="m.filename" :class="{ off: !m.enabled }">
        <td>
          <b>{{ m.title ?? m.filename.replace(/\.disabled$/, "") }}</b>
          <span v-if="m.version" class="muted small ver">{{ m.version }}</span>
          <span v-if="!m.enabled" class="tag off-tag">{{ $t("resources.disabledTag") }}</span>
          <div v-if="m.title" class="muted small break">{{ m.filename }}</div>
        </td>
        <td class="num muted">{{ m.size ? formatBytes(m.size) : $t("instance.packs.folder") }}</td>
        <td class="num"><span class="row-actions">
          <button class="btn small" :disabled="running" :title="running ? $t('instance.quitFirst') : ''" @click="toggle(m)">{{ m.enabled ? $t("resources.disable") : $t("resources.enable") }}</button>
          <button class="btn small" :disabled="running" @click="remove(m)">{{ $t("common.delete") }}</button>
        </span></td>
      </tr>
      <tr v-if="busy && !items"><td colspan="3" class="muted">{{ $t("instance.packs.reading") }}</td></tr>
      <tr v-else-if="!shown.length"><td colspan="3" class="muted">{{ filter ? $t("instance.packs.noMatch") : $t(`instance.packs.empty.${kind}`) }}</td></tr>
    </tbody>
  </table>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.grow { flex: 1; min-width: 0; }
.hint { padding-top: 0; }
.small { font-size: 12px; }
.break { word-break: break-all; }
.ver { margin-left: 6px; }
tr.off b { color: var(--text-muted); text-decoration: line-through; }
.off-tag { background: var(--text-muted); margin-left: 6px; }
</style>
