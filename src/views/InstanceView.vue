<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import { api, inTauri } from "../api";
import type { InstallPlan, InstanceInfo, InstanceSettings, JavaInstall, LocalResource, MemoryInfo, ModUpdate, World, WorldBackup } from "../api/types";
import { useGameStore } from "../stores/game";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";
import { formatBytes, formatDuration, loaderLabel, timeAgo } from "../utils";
import ScreenshotGallery from "../components/instance/ScreenshotGallery.vue";
import ServerList from "../components/instance/ServerList.vue";
import { errText, locale, t } from "../i18n";

// 版本管理 → 管理：单个版本的 Java / 内存 / 参数、模组、存档
const route = useRoute();
const router = useRouter();
const versions = useVersionsStore();
const game = useGameStore();
const settings = useSettingsStore();

const id = computed(() => String(route.params.id ?? ""));
const version = computed(() => versions.installed.find((v) => v.id === id.value));
const running = computed(() => game.running.some((r) => r.versionId === id.value));

type Tab = "settings" | "mods" | "saves" | "servers" | "screenshots";
const tabs: Tab[] = ["settings", "mods", "saves", "servers", "screenshots"];
const tab = ref<Tab>(tabs.find((k) => k === route.query.tab) ?? "settings");
watch(tab, (k) => router.replace({ query: { tab: k } }));

const info = ref<InstanceInfo | null>(null);
const loadError = ref("");
const message = ref<{ ok: boolean; text: string } | null>(null);

function fail(e: unknown) {
  message.value = { ok: false, text: errText(e) };
}

async function openFolder(path: string) {
  await api.quickOpenFolder(path).catch(fail);
}
const sub = (name: string) => (info.value ? `${info.value.gameDir}${info.value.gameDir.includes("\\") ? "\\" : "/"}${name}` : "");

onMounted(async () => {
  if (!versions.installed.length) await versions.loadInstalled();
  try {
    info.value = await api.instanceInfo(id.value);
    form.value = { ...info.value.settings };
  } catch (e) {
    loadError.value = errText(e);
    return;
  }
  javas.value = await api.listJava().catch(() => []);
  memory.value = await api.memoryInfo().catch(() => null);
  if (tab.value === "mods") await loadMods();
  if (tab.value === "saves") await loadSaves();
});
watch(tab, (t) => {
  message.value = null;
  if (t === "mods" && !mods.value) loadMods();
  if (t === "saves" && !worlds.value) loadSaves();
});

// ---- 启动设置 ----
const form = ref<InstanceSettings | null>(null);
const javas = ref<JavaInstall[]>([]);
const memory = ref<MemoryInfo | null>(null);
const saving = ref(false);
const dirty = computed(() => !!info.value && !!form.value && JSON.stringify(form.value) !== JSON.stringify(info.value.settings));

const globalJava = computed(() => settings.settings?.javaPath ?? t("instance.javaAuto"));
const globalMemory = computed(() => settings.settings?.memoryMb ?? memory.value?.autoMb ?? null);
/** 选中的 Java 不在扫描列表里（手动选的）也要能显示出来 */
const javaOptions = computed(() => {
  const list = javas.value.map((j) => ({ path: j.path, label: `Java ${j.major} (${j.version}, ${j.vendor ?? j.arch})` }));
  const cur = form.value?.javaPath;
  if (cur && !list.some((j) => j.path === cur)) list.push({ path: cur, label: cur });
  return list;
});
const customMemory = computed({
  get: () => form.value?.memoryMb != null,
  set: (on: boolean) => {
    if (form.value) form.value.memoryMb = on ? (form.value.memoryMb ?? globalMemory.value ?? 4096) : null;
  },
});

async function pickJava() {
  if (!inTauri) return fail(t("theme.previewNoPick"));
  const chosen = await open({ multiple: false, directory: false, title: t("instance.pickJava") });
  if (typeof chosen === "string" && form.value) form.value.javaPath = chosen;
}

async function saveSettings() {
  if (!form.value) return;
  saving.value = true;
  message.value = null;
  try {
    info.value = await api.instanceSave(id.value, { ...form.value, javaPath: form.value.javaPath || null });
    form.value = { ...info.value.settings };
    message.value = { ok: true, text: running.value ? t("instance.savedNext") : t("instance.saved") };
  } catch (e) {
    fail(e);
  } finally {
    saving.value = false;
  }
}

// ---- 模组 ----
const mods = ref<LocalResource[] | null>(null);
const modsBusy = ref(false);
const modFilter = ref("");
const shownMods = computed(() => {
  const q = modFilter.value.trim().toLowerCase();
  return (mods.value ?? []).filter((m) => !q || m.filename.toLowerCase().includes(q) || m.title?.toLowerCase().includes(q));
});
const enabledCount = computed(() => (mods.value ?? []).filter((m) => m.enabled).length);

async function loadMods() {
  modsBusy.value = true;
  try {
    mods.value = await api.localResources(id.value, "mod");
  } catch (e) {
    mods.value = [];
    fail(e);
  } finally {
    modsBusy.value = false;
  }
}

async function toggleMod(m: LocalResource) {
  try {
    m.filename = await api.localResourceToggle(id.value, "mod", m.filename, !m.enabled);
    m.enabled = !m.enabled;
  } catch (e) {
    fail(e);
  }
}

// 模组更新：先问 Modrinth，认不出的再问 CurseForge，按这个版本的游戏版本和加载器找最新版
const updates = ref<ModUpdate[] | null>(null);
const picked = ref(new Set<string>());
const checking = ref(false);
const updating = ref(false);
const unknownCount = ref(0);
const updateFor = computed(() => new Map((updates.value ?? []).map((u) => [u.filename, u])));

async function checkUpdates() {
  checking.value = true;
  try {
    const r = await api.modUpdates(id.value);
    unknownCount.value = r.unknown;
    updates.value = r.updates;
    // 默认勾选正式版，测试版让人自己决定
    picked.value = new Set(updates.value.filter((u) => u.latest.releaseType === "release").map((u) => u.filename));
  } catch (e) {
    fail(e);
  } finally {
    checking.value = false;
  }
}

function togglePick(u: ModUpdate) {
  const s = new Set(picked.value);
  if (s.has(u.filename)) s.delete(u.filename);
  else s.add(u.filename);
  picked.value = s;
}

async function applyUpdates() {
  const chosen = (updates.value ?? []).filter((u) => picked.value.has(u.filename) && u.latest.file);
  if (!chosen.length) return;
  const plan: InstallPlan = {
    items: chosen.map((u) => ({
      source: u.latest.source,
      projectId: u.projectId,
      title: u.title,
      versionId: u.latest.id,
      versionNumber: u.latest.versionNumber,
      // 原来禁用着的，新版也保持禁用
      file: { ...u.latest.file!, filename: u.enabled ? u.latest.file!.filename : `${u.latest.file!.filename}.disabled` },
      requiredBy: null,
      replaces: [u.filename],
      compat: null,
      note: null,
      alternative: null,
    })),
    alreadyInstalled: [],
    unresolved: [],
    optional: [],
    incompatible: [],
  };
  const jobId = t("instance.updateJob", { n: chosen.length, id: id.value });
  const job = versions.startJob(jobId);
  updating.value = true;
  try {
    const r = await api.resourceInstall(jobId, "mod", id.value, plan);
    job.report = r;
    job.status = r.failed.length ? "failed" : "done";
    if (r.failed.length) {
      job.error = t("progress.filesFailed", { n: r.failed.length, reason: r.failed[0].reason });
      message.value = { ok: false, text: t("instance.updateFailed", { n: r.failed.length }) };
    } else {
      message.value = { ok: true, text: t("instance.updated", { n: chosen.length }) };
    }
  } catch (e) {
    job.status = "failed";
    job.error = errText(e);
    fail(e);
  } finally {
    updating.value = false;
    updates.value = null;
    await loadMods();
  }
}

async function deleteMod(m: LocalResource) {
  if (!confirm(t("instance.confirmDeleteMod", { name: m.title ?? m.filename }))) return;
  try {
    await api.localResourceDelete(id.value, "mod", m.filename);
    mods.value = (mods.value ?? []).filter((x) => x !== m);
  } catch (e) {
    fail(e);
  }
}

// ---- 存档 ----
const worlds = ref<World[] | null>(null);
const backups = ref<WorldBackup[]>([]);
const savesBusy = ref<string | null>(null); // 正在处理的世界 / 备份文件

async function loadSaves() {
  try {
    [worlds.value, backups.value] = await Promise.all([api.worldList(id.value), api.worldBackups(id.value)]);
  } catch (e) {
    worlds.value = [];
    fail(e);
  }
}

async function backupWorld(w: World) {
  savesBusy.value = w.folder;
  message.value = null;
  try {
    const b = await api.worldBackup(id.value, w.folder);
    backups.value = [b, ...backups.value];
    message.value = { ok: true, text: t("instance.backedUp", { name: w.folder, size: formatBytes(b.size) }) };
  } catch (e) {
    fail(e);
  } finally {
    savesBusy.value = null;
  }
}

async function restore(b: WorldBackup) {
  const exists = worlds.value?.some((w) => w.folder === b.world);
  const tip = exists ? `\n${t("instance.restoreTip", { name: b.world })}` : "";
  if (!confirm(t("instance.confirmRestore", { name: b.world, time: fullTime(b.created) }) + tip)) return;
  savesBusy.value = b.file;
  message.value = null;
  try {
    const auto = await api.worldRestore(id.value, b.file);
    message.value = { ok: true, text: t("instance.restored", { name: b.world }) + (auto ? t("instance.restoredAuto") : "") };
    await loadSaves();
  } catch (e) {
    fail(e);
  } finally {
    savesBusy.value = null;
  }
}

async function deleteBackup(b: WorldBackup) {
  if (!confirm(t("instance.confirmDeleteBackup", { time: fullTime(b.created), name: b.world }))) return;
  try {
    await api.worldBackupDelete(id.value, b.file);
    backups.value = backups.value.filter((x) => x !== b);
  } catch (e) {
    fail(e);
  }
}

function fullTime(unix: number) {
  return new Date(unix * 1000).toLocaleString(locale.value, { hour12: false });
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      <RouterLink to="/versions">{{ $t("nav.versions") }}</RouterLink> › {{ id }}
      <span v-if="version?.loader" class="tag" :class="'ld-' + version.loader">{{ loaderLabel[version.loader] }}</span>
      <span class="extra">
        <template v-if="info">{{ $t("quickLaunch.played", { time: formatDuration(info.playTimeSecs) }) }} · {{ $t("instance.last", { time: timeAgo(info.lastPlayed) }) }}</template>
      </span>
    </div>

    <div v-if="loadError" class="box-body">
      <div class="mod-tip">{{ $t("common.modTip") }}{{ loadError }}</div>
      <p><RouterLink to="/versions">{{ $t("instance.backToVersions") }}</RouterLink></p>
    </div>

    <template v-else-if="info && form">
      <div class="box-body bar">
        <div class="tabs">
          <a v-for="k in tabs" :key="k" :class="{ active: tab === k }" @click="tab = k">{{ $t(`instance.tab.${k}`) }}</a>
        </div>
        <span v-if="running" class="playing">● {{ $t("versions.playing") }}</span>
        <button class="btn small" @click="openFolder(info.gameDir)">{{ $t("instance.openGameDir") }}</button>
      </div>
      <div v-if="message" class="box-body msg" :class="message.ok ? 'ok-text' : 'error-text'">{{ message.text }}</div>

      <!-- 启动设置 -->
      <div v-if="tab === 'settings'" class="box-body form">
        <p class="muted small"><i18n-t keypath="instance.scopeHint"><span>{{ id }}</span><RouterLink to="/settings">{{ $t("nav.settings") }}</RouterLink></i18n-t></p>

        <label class="row">
          <span class="label">{{ $t("settings.isolate") }}</span>
          <span class="field">
            <label class="check"><input v-model="form.isolated" type="checkbox" :disabled="!!info.modpack || info.globalIsolate" />
              {{ $t("instance.isolateHint") }}</label>
            <span v-if="info.modpack" class="muted small">{{ $t("instance.modpackIsolated", { name: info.modpack.name }) }}</span>
            <span v-else-if="info.globalIsolate" class="muted small">{{ $t("instance.globalIsolate") }}</span>
            <span v-else-if="form.isolated !== info.settings.isolated" class="warn small">{{ $t("instance.isolateWarn") }}</span>
            <code class="small break">{{ info.gameDir }}</code>
          </span>
        </label>

        <div class="row">
          <span class="label">Java</span>
          <span class="field">
            <span class="line">
              <select v-model="form.javaPath" class="input grow">
                <option :value="null">{{ $t("instance.followGlobal", { v: globalJava }) }}</option>
                <option v-for="j in javaOptions" :key="j.path" :value="j.path">{{ j.label }}</option>
              </select>
              <button class="btn small" @click="pickJava">{{ $t("common.browse") }}</button>
            </span>
            <span v-if="form.javaPath" class="muted small break">{{ form.javaPath }}</span>
          </span>
        </div>

        <div class="row">
          <span class="label">{{ $t("settings.memory") }}</span>
          <span class="field">
            <label class="check"><input v-model="customMemory" type="checkbox" /> {{ $t("instance.customMemory") }}</label>
            <span v-if="form.memoryMb != null" class="line">
              <input v-model.number="form.memoryMb" type="range" min="1024" :max="memory?.totalMb ?? 16384" step="256" class="grow" />
              <input v-model.number="form.memoryMb" type="number" min="512" step="256" class="input num" /> MB
            </span>
            <span v-else class="muted small">{{ $t("instance.followGlobalMemory", { v: globalMemory ? `${globalMemory} MB` : $t("settings.auto") }) }}</span>
            <span v-if="memory && form.memoryMb != null && form.memoryMb > memory.totalMb * 0.8" class="warn small">{{ $t("instance.memoryWarn", { mb: memory.totalMb }) }}</span>
          </span>
        </div>

        <label class="row">
          <span class="label">{{ $t("settings.jvmArgs") }}</span>
          <span class="field">
            <input v-model="form.jvmArgs" class="input" :placeholder="$t('instance.jvmPlaceholder')" />
          </span>
        </label>
        <label class="row">
          <span class="label">{{ $t("settings.gameArgs") }}</span>
          <span class="field">
            <input v-model="form.gameArgs" class="input" :placeholder="$t('instance.gameArgsPlaceholder')" />
          </span>
        </label>

        <div class="row">
          <span class="label" />
          <span class="field"><span class="line">
            <button class="btn primary" :disabled="saving || !dirty" @click="saveSettings">{{ saving ? $t("instance.saving") : $t("common.save") }}</button>
            <button class="btn" :disabled="saving || !dirty" @click="form = { ...info.settings }">{{ $t("instance.revert") }}</button>
          </span></span>
        </div>
      </div>

      <!-- 模组 -->
      <template v-else-if="tab === 'mods'">
        <div class="box-body bar">
          <input v-model="modFilter" class="input grow" :placeholder="$t('instance.searchMods')" />
          <button class="btn small" :disabled="modsBusy" @click="loadMods">{{ $t("common.refresh") }}</button>
          <button class="btn small" :disabled="checking || updating || !mods?.length" @click="checkUpdates">{{ checking ? $t("settings.checking") : $t("settings.checkUpdate") }}</button>
          <button class="btn small" @click="openFolder(sub('mods'))">{{ $t("instance.openMods") }}</button>
          <RouterLink :to="{ path: '/resources', query: { version: id } }" class="btn small primary">{{ $t("instance.downloadMods") }}</RouterLink>
        </div>
        <div v-if="version && !version.loader" class="box-body">
          <div class="mod-tip">{{ $t("common.modTip") }}{{ $t("instance.noLoader") }}</div>
        </div>
        <div v-if="updates" class="box-body updates">
          <template v-if="updates.length">
            <div class="line">
              <b>{{ $t("instance.updatesFound", { n: updates.length }) }}</b>
              <span class="muted small grow">{{ $t("instance.updatesHint") }}</span>
              <button class="btn small" @click="updates = null">{{ $t("common.collapse") }}</button>
              <button class="btn small primary" :disabled="!picked.size || updating || running" :title="running ? $t('instance.quitFirst') : ''" @click="applyUpdates">
                {{ updating ? $t("update.updating") : $t("instance.updateSelected", { n: picked.size }) }}
              </button>
            </div>
            <label v-for="u in updates" :key="u.filename" class="check up-row">
              <input type="checkbox" :checked="picked.has(u.filename)" @change="togglePick(u)" />
              <b>{{ u.title }}</b>
              <span class="muted small">{{ u.current }} → </span><span class="ok-text small">{{ u.latest.versionNumber }}</span>
              <span class="muted small">{{ u.latest.source === "curseforge" ? "CurseForge" : "Modrinth" }}</span>
              <span v-if="u.latest.releaseType !== 'release'" class="tag pre-tag">{{ u.latest.releaseType === "beta" ? $t("instance.beta") : $t("instance.alpha") }}</span>
            </label>
          </template>
          <div v-else class="line">
            <span class="ok-text">{{ $t("instance.allLatest") }}</span>
            <button class="btn small" @click="updates = null">{{ $t("common.collapse") }}</button>
          </div>
          <div v-if="unknownCount" class="muted small">{{ $t("instance.unknownMods", { n: unknownCount }) }}</div>
        </div>
        <table class="thread-table">
          <thead>
            <tr><th>{{ $t("instance.modsHeader", { on: enabledCount, total: mods?.length ?? 0 }) }}</th><th class="num">{{ $t("instance.size") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="m in shownMods" :key="m.filename" :class="{ off: !m.enabled }">
              <td>
                <b>{{ m.title ?? m.filename.replace(/\.disabled$/, "") }}</b>
                <span v-if="m.version" class="muted small ver">{{ m.version }}</span>
                <span v-if="!m.enabled" class="tag off-tag">{{ $t("resources.disabledTag") }}</span>
                <span v-if="updateFor.get(m.filename)" class="tag new-tag">{{ $t("instance.updatable") }}</span>
                <div v-if="m.title" class="muted small break">{{ m.filename }}</div>
              </td>
              <td class="num muted">{{ formatBytes(m.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="running" :title="running ? $t('instance.quitFirst') : ''" @click="toggleMod(m)">{{ m.enabled ? $t("resources.disable") : $t("resources.enable") }}</button>
                <button class="btn small" :disabled="running" @click="deleteMod(m)">{{ $t("common.delete") }}</button>
              </span></td>
            </tr>
            <tr v-if="modsBusy && !mods"><td colspan="3" class="muted">{{ $t("instance.readingMods") }}</td></tr>
            <tr v-else-if="!shownMods.length"><td colspan="3" class="muted">{{ modFilter ? $t("instance.noMatch") : $t("instance.noMods") }}</td></tr>
          </tbody>
        </table>
      </template>

      <!-- 服务器 -->
      <ServerList v-else-if="tab === 'servers'" :version-id="id" :game-dir="info.gameDir" :running="running" @message="(ok, text) => (message = { ok, text })" />

      <!-- 截图 -->
      <ScreenshotGallery v-else-if="tab === 'screenshots'" :version-id="id" :game-dir="info.gameDir" @message="(ok, text) => (message = { ok, text })" @open-folder="openFolder" />

      <!-- 存档 -->
      <template v-else>
        <div class="box-body bar">
          <span class="muted small grow"><i18n-t keypath="instance.backupHint"><code>godbbs-backups</code></i18n-t></span>
          <button class="btn small" @click="loadSaves">{{ $t("common.refresh") }}</button>
          <button class="btn small" @click="openFolder(sub('saves'))">{{ $t("instance.openSaves") }}</button>
        </div>
        <table class="thread-table">
          <thead>
            <tr><th>{{ $t("instance.world") }}</th><th class="num">{{ $t("instance.lastPlayed") }}</th><th class="num">{{ $t("instance.size") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="w in worlds ?? []" :key="w.folder">
              <td><b>{{ w.folder }}</b></td>
              <td class="num muted">{{ timeAgo(w.modified) }}</td>
              <td class="num muted">{{ formatBytes(w.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="!!savesBusy" @click="backupWorld(w)">{{ savesBusy === w.folder ? $t("instance.backingUp") : $t("instance.backup") }}</button>
              </span></td>
            </tr>
            <tr v-if="worlds && !worlds.length"><td colspan="4" class="muted">{{ $t("instance.noWorlds") }}</td></tr>
            <tr v-if="!worlds"><td colspan="4" class="muted">{{ $t("instance.readingWorlds") }}</td></tr>
          </tbody>
        </table>

        <div class="box-title sub-title">{{ $t("instance.backups") }}</div>
        <table class="thread-table">
          <thead>
            <tr><th>{{ $t("instance.world") }}</th><th class="num">{{ $t("instance.backupTime") }}</th><th class="num">{{ $t("instance.size") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="b in backups" :key="b.file">
              <td><b>{{ b.world }}</b></td>
              <td class="num muted">{{ fullTime(b.created) }}</td>
              <td class="num muted">{{ formatBytes(b.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="!!savesBusy || running" :title="running ? $t('instance.quitFirst') : ''" @click="restore(b)">{{ savesBusy === b.file ? $t("instance.restoring") : $t("instance.restore") }}</button>
                <button class="btn small" :disabled="!!savesBusy" @click="deleteBackup(b)">{{ $t("common.delete") }}</button>
              </span></td>
            </tr>
            <tr v-if="!backups.length"><td colspan="4" class="muted">{{ $t("instance.noBackups") }}</td></tr>
          </tbody>
        </table>
      </template>
    </template>

    <div v-else class="box-body muted">{{ $t("instance.reading") }}</div>
  </div>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.tabs { display: flex; margin-right: auto; }
.tabs a { padding: 4px 12px; border: 1px solid var(--border); margin-right: -1px; background: var(--bg-box-alt); color: var(--text); cursor: pointer; }
.tabs a:hover { text-decoration: none; background: var(--bg-title); }
.tabs a.active { background: var(--bg-box); font-weight: bold; color: var(--brown-dark); }
.msg { padding-top: 0; }
.form { display: flex; flex-direction: column; gap: 12px; }
.row { display: flex; gap: 12px; align-items: flex-start; }
.label { flex: none; width: 72px; padding-top: 4px; color: var(--text-muted); }
.field { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.line { display: flex; align-items: center; gap: 8px; }
.check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.grow { flex: 1; min-width: 0; }
.num.input { width: 90px; }
.small { font-size: 12px; }
.break { word-break: break-all; }
.warn { color: var(--gold); }
.ok-text { color: var(--grass); }
.playing { color: var(--grass); font-size: 12px; }
.row-actions { display: inline-flex; gap: 6px; }
.ver { margin-left: 6px; }
tr.off b { color: var(--text-muted); text-decoration: line-through; }
.tag { margin-left: 6px; }
.off-tag { background: var(--text-muted); }
.new-tag { background: var(--grass); }
.pre-tag { background: var(--gold); }
.updates { display: flex; flex-direction: column; gap: 6px; border-bottom: 1px solid var(--border); }
.up-row { gap: 8px; }
.sub-title { border-top: 1px solid var(--border); }
.tag.ld-fabric { background: #b39b72; }
.tag.ld-quilt { background: #8e5fb4; }
.tag.ld-forge { background: #3e4b61; }
.tag.ld-neoforge { background: #d2691e; }
</style>
