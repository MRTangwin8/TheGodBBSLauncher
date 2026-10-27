<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import { api, inTauri } from "../api";
import type { InstanceInfo, InstanceSettings, JavaInstall, LocalResource, MemoryInfo, World, WorldBackup } from "../api/types";
import { useGameStore } from "../stores/game";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";
import { formatBytes, formatDuration, loaderLabel, timeAgo } from "../utils";

// 版本管理 → 管理：单个版本的 Java / 内存 / 参数、模组、存档
const route = useRoute();
const router = useRouter();
const versions = useVersionsStore();
const game = useGameStore();
const settings = useSettingsStore();

const id = computed(() => String(route.params.id ?? ""));
const version = computed(() => versions.installed.find((v) => v.id === id.value));
const running = computed(() => game.running.some((r) => r.versionId === id.value));

type Tab = "settings" | "mods" | "saves";
const tabs: { key: Tab; label: string }[] = [
  { key: "settings", label: "启动设置" },
  { key: "mods", label: "模组" },
  { key: "saves", label: "存档" },
];
const tab = ref<Tab>((["settings", "mods", "saves"] as const).find((t) => t === route.query.tab) ?? "settings");
watch(tab, (t) => router.replace({ query: { tab: t } }));

const info = ref<InstanceInfo | null>(null);
const loadError = ref("");
const message = ref<{ ok: boolean; text: string } | null>(null);

function fail(e: unknown) {
  message.value = { ok: false, text: `${e}` };
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
    loadError.value = `${e}`;
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

const globalJava = computed(() => settings.settings?.javaPath ?? "自动选择合适的版本");
const globalMemory = computed(() => settings.settings?.memoryMb ?? memory.value?.autoMb ?? null);
/** 选中的 Java 不在扫描列表里（手动选的）也要能显示出来 */
const javaOptions = computed(() => {
  const list = javas.value.map((j) => ({ path: j.path, label: `Java ${j.major}（${j.version}，${j.vendor ?? j.arch}）` }));
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
  if (!inTauri) return fail("浏览器预览模式不能选择文件");
  const chosen = await open({ multiple: false, directory: false, title: "选择 java 可执行文件（javaw.exe / java）" });
  if (typeof chosen === "string" && form.value) form.value.javaPath = chosen;
}

async function saveSettings() {
  if (!form.value) return;
  saving.value = true;
  message.value = null;
  try {
    info.value = await api.instanceSave(id.value, { ...form.value, javaPath: form.value.javaPath || null });
    form.value = { ...info.value.settings };
    message.value = { ok: true, text: running.value ? "已保存，下次启动时生效。" : "已保存。" };
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

async function deleteMod(m: LocalResource) {
  if (!confirm(`删除「${m.title ?? m.filename}」？\n文件会直接删掉，不进回收站。只是暂时不想用的话，点「禁用」就行。`)) return;
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
    message.value = { ok: true, text: `已备份「${w.folder}」（${formatBytes(b.size)}）。` };
  } catch (e) {
    fail(e);
  } finally {
    savesBusy.value = null;
  }
}

async function restore(b: WorldBackup) {
  const exists = worlds.value?.some((w) => w.folder === b.world);
  const tip = exists ? "\n现在的「" + b.world + "」会先自动备份一份，然后被替换。" : "";
  if (!confirm(`把「${b.world}」恢复到 ${fullTime(b.created)} 的样子？${tip}`)) return;
  savesBusy.value = b.file;
  message.value = null;
  try {
    const auto = await api.worldRestore(id.value, b.file);
    message.value = { ok: true, text: `已恢复「${b.world}」。${auto ? "恢复前的样子也存了一份备份，后悔了可以再恢复回去。" : ""}` };
    await loadSaves();
  } catch (e) {
    fail(e);
  } finally {
    savesBusy.value = null;
  }
}

async function deleteBackup(b: WorldBackup) {
  if (!confirm(`删除 ${fullTime(b.created)} 的「${b.world}」备份？删了就找不回来了。`)) return;
  try {
    await api.worldBackupDelete(id.value, b.file);
    backups.value = backups.value.filter((x) => x !== b);
  } catch (e) {
    fail(e);
  }
}

function fullTime(unix: number) {
  return new Date(unix * 1000).toLocaleString("zh-CN", { hour12: false });
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      <RouterLink to="/versions">版本管理</RouterLink> › {{ id }}
      <span v-if="version?.loader" class="tag" :class="'ld-' + version.loader">{{ loaderLabel[version.loader] }}</span>
      <span class="extra">
        <template v-if="info">已玩 {{ formatDuration(info.playTimeSecs) }} · 上次 {{ timeAgo(info.lastPlayed) }}</template>
      </span>
    </div>

    <div v-if="loadError" class="box-body">
      <div class="mod-tip">【版主提示】{{ loadError }}</div>
      <p><RouterLink to="/versions">返回版本管理</RouterLink></p>
    </div>

    <template v-else-if="info && form">
      <div class="box-body bar">
        <div class="tabs">
          <a v-for="t in tabs" :key="t.key" :class="{ active: tab === t.key }" @click="tab = t.key">{{ t.label }}</a>
        </div>
        <span v-if="running" class="playing">● 游戏中</span>
        <button class="btn small" @click="openFolder(info.gameDir)">打开游戏目录</button>
      </div>
      <div v-if="message" class="box-body msg" :class="message.ok ? 'ok-text' : 'error-text'">{{ message.text }}</div>

      <!-- 启动设置 -->
      <div v-if="tab === 'settings'" class="box-body form">
        <p class="muted small">这里的设置只对「{{ id }}」生效；留空 / 选「跟随全局」的项使用 <RouterLink to="/settings">设置</RouterLink> 里的值。</p>

        <label class="row">
          <span class="label">版本隔离</span>
          <span class="field">
            <label class="check"><input v-model="form.isolated" type="checkbox" :disabled="!!info.modpack || info.globalIsolate" />
              存档、模组、配置单独放在这个版本自己的文件夹里</label>
            <span v-if="info.modpack" class="muted small">整合包「{{ info.modpack.name }}」必须隔离，不能关闭。</span>
            <span v-else-if="info.globalIsolate" class="muted small">全局设置已经开启了版本隔离，所有版本都是隔离的。</span>
            <span v-else-if="form.isolated !== info.settings.isolated" class="warn small">切换后游戏目录会变，原来的存档和模组不会跟着搬过去。</span>
            <code class="small break">{{ info.gameDir }}</code>
          </span>
        </label>

        <div class="row">
          <span class="label">Java</span>
          <span class="field">
            <span class="line">
              <select v-model="form.javaPath" class="input grow">
                <option :value="null">跟随全局（{{ globalJava }}）</option>
                <option v-for="j in javaOptions" :key="j.path" :value="j.path">{{ j.label }}</option>
              </select>
              <button class="btn small" @click="pickJava">选择…</button>
            </span>
            <span v-if="form.javaPath" class="muted small break">{{ form.javaPath }}</span>
          </span>
        </div>

        <div class="row">
          <span class="label">最大内存</span>
          <span class="field">
            <label class="check"><input v-model="customMemory" type="checkbox" /> 这个版本单独设置</label>
            <span v-if="form.memoryMb != null" class="line">
              <input v-model.number="form.memoryMb" type="range" min="1024" :max="memory?.totalMb ?? 16384" step="256" class="grow" />
              <input v-model.number="form.memoryMb" type="number" min="512" step="256" class="input num" /> MB
            </span>
            <span v-else class="muted small">跟随全局：{{ globalMemory ? `${globalMemory} MB` : "自动" }}</span>
            <span v-if="memory && form.memoryMb != null && form.memoryMb > memory.totalMb * 0.8" class="warn small">超过了本机内存（{{ memory.totalMb }} MB）的八成，系统可能会卡。</span>
          </span>
        </div>

        <label class="row">
          <span class="label">JVM 参数</span>
          <span class="field">
            <input v-model="form.jvmArgs" class="input" placeholder="例如 -XX:+UseZGC，追加在全局 JVM 参数之后" />
          </span>
        </label>
        <label class="row">
          <span class="label">游戏参数</span>
          <span class="field">
            <input v-model="form.gameArgs" class="input" placeholder="追加在全局游戏参数之后" />
          </span>
        </label>

        <div class="row">
          <span class="label" />
          <span class="field"><span class="line">
            <button class="btn primary" :disabled="saving || !dirty" @click="saveSettings">{{ saving ? "保存中…" : "保存" }}</button>
            <button class="btn" :disabled="saving || !dirty" @click="form = { ...info.settings }">撤销修改</button>
          </span></span>
        </div>
      </div>

      <!-- 模组 -->
      <template v-else-if="tab === 'mods'">
        <div class="box-body bar">
          <input v-model="modFilter" class="input grow" placeholder="搜索已装的模组…" />
          <button class="btn small" :disabled="modsBusy" @click="loadMods">刷新</button>
          <button class="btn small" @click="openFolder(sub('mods'))">打开模组文件夹</button>
          <RouterLink :to="{ path: '/resources', query: { version: id } }" class="btn small primary">下载模组</RouterLink>
        </div>
        <div v-if="version && !version.loader" class="box-body">
          <div class="mod-tip">【版主提示】这个版本没有装模组加载器（Fabric / Forge 等），放进去的模组不会生效。到「下载中心」装一个带加载器的版本吧。</div>
        </div>
        <table class="thread-table">
          <thead>
            <tr><th>模组（已启用 {{ enabledCount }} / {{ mods?.length ?? 0 }}）</th><th class="num">大小</th><th class="num">操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="m in shownMods" :key="m.filename" :class="{ off: !m.enabled }">
              <td>
                <b>{{ m.title ?? m.filename.replace(/\.disabled$/, "") }}</b>
                <span v-if="m.version" class="muted small ver">{{ m.version }}</span>
                <span v-if="!m.enabled" class="tag off-tag">已禁用</span>
                <div v-if="m.title" class="muted small break">{{ m.filename }}</div>
              </td>
              <td class="num muted">{{ formatBytes(m.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="running" :title="running ? '先退出游戏' : ''" @click="toggleMod(m)">{{ m.enabled ? "禁用" : "启用" }}</button>
                <button class="btn small" :disabled="running" @click="deleteMod(m)">删除</button>
              </span></td>
            </tr>
            <tr v-if="modsBusy && !mods"><td colspan="3" class="muted">正在读取模组列表…</td></tr>
            <tr v-else-if="!shownMods.length"><td colspan="3" class="muted">{{ modFilter ? "没有找到匹配的模组" : "还没有装模组" }}</td></tr>
          </tbody>
        </table>
      </template>

      <!-- 存档 -->
      <template v-else>
        <div class="box-body bar">
          <span class="muted small grow">备份存在 <code>godbbs-backups</code> 文件夹里。游戏开着时也能备份，但最好先退出到标题画面，避免备份到写了一半的数据。</span>
          <button class="btn small" @click="loadSaves">刷新</button>
          <button class="btn small" @click="openFolder(sub('saves'))">打开存档文件夹</button>
        </div>
        <table class="thread-table">
          <thead>
            <tr><th>存档</th><th class="num">上次游玩</th><th class="num">大小</th><th class="num">操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="w in worlds ?? []" :key="w.folder">
              <td><b>{{ w.folder }}</b></td>
              <td class="num muted">{{ timeAgo(w.modified) }}</td>
              <td class="num muted">{{ formatBytes(w.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="!!savesBusy" @click="backupWorld(w)">{{ savesBusy === w.folder ? "备份中…" : "备份" }}</button>
              </span></td>
            </tr>
            <tr v-if="worlds && !worlds.length"><td colspan="4" class="muted">还没有存档，进游戏创建一个世界吧</td></tr>
            <tr v-if="!worlds"><td colspan="4" class="muted">正在读取存档…</td></tr>
          </tbody>
        </table>

        <div class="box-title sub-title">备份</div>
        <table class="thread-table">
          <thead>
            <tr><th>存档</th><th class="num">备份时间</th><th class="num">大小</th><th class="num">操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="b in backups" :key="b.file">
              <td><b>{{ b.world }}</b></td>
              <td class="num muted">{{ fullTime(b.created) }}</td>
              <td class="num muted">{{ formatBytes(b.size) }}</td>
              <td class="num"><span class="row-actions">
                <button class="btn small" :disabled="!!savesBusy || running" :title="running ? '先退出游戏' : ''" @click="restore(b)">{{ savesBusy === b.file ? "恢复中…" : "恢复" }}</button>
                <button class="btn small" :disabled="!!savesBusy" @click="deleteBackup(b)">删除</button>
              </span></td>
            </tr>
            <tr v-if="!backups.length"><td colspan="4" class="muted">还没有备份</td></tr>
          </tbody>
        </table>
      </template>
    </template>

    <div v-else class="box-body muted">正在读取…</div>
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
.sub-title { border-top: 1px solid var(--border); }
.tag.ld-fabric { background: #b39b72; }
.tag.ld-quilt { background: #8e5fb4; }
.tag.ld-forge { background: #3e4b61; }
.tag.ld-neoforge { background: #d2691e; }
</style>
