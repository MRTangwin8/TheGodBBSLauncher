<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import { api } from "../../api";
import type { SavedServer, ServerStatus } from "../../api/types";
import { useGameStore } from "../../stores/game";
import { errText, t } from "../../i18n";

// 版本管理 → 服务器：游戏「多人游戏」里那张列表（servers.dat），能查延迟和在线人数，一键启动并进服
const props = defineProps<{ versionId: string; gameDir: string; running: boolean }>();
const emit = defineEmits<{ message: [ok: boolean, text: string] }>();
const router = useRouter();
const game = useGameStore();

type Ping = { state: "loading" } | { state: "ok"; status: ServerStatus } | { state: "error"; error: string };

const servers = ref<SavedServer[] | null>(null);
const pings = reactive<Record<string, Ping>>({});
const busy = ref(false);
const draft = ref({ name: "", address: "" });
const editing = ref<{ index: number; name: string; address: string } | null>(null);

const lockedTip = computed(() => (props.running ? t("servers.locked") : ""));

onMounted(load);

async function load() {
  try {
    servers.value = await api.serverList(props.versionId);
    pingAll();
  } catch (e) {
    servers.value = [];
    emit("message", false, errText(e));
  }
}

function pingAll() {
  for (const s of servers.value ?? []) ping(s.ip);
}

async function ping(address: string) {
  pings[address] = { state: "loading" };
  try {
    pings[address] = { state: "ok", status: await api.serverPing(address) };
  } catch (e) {
    pings[address] = { state: "error", error: errText(e) };
  }
}

async function edit(run: () => Promise<SavedServer[]>, done?: string) {
  busy.value = true;
  try {
    servers.value = await run();
    if (done) emit("message", true, done);
    return true;
  } catch (e) {
    emit("message", false, errText(e));
    return false;
  } finally {
    busy.value = false;
  }
}

async function add() {
  const { name, address } = draft.value;
  if (!address.trim()) return;
  if (await edit(() => api.serverAdd(props.versionId, name, address), t("servers.added", { name: name.trim() || address.trim() }))) {
    ping(address.trim());
    draft.value = { name: "", address: "" };
  }
}

async function saveEdit() {
  const e = editing.value;
  if (!e) return;
  if (await edit(() => api.serverUpdate(props.versionId, e.index, e.name, e.address))) {
    if (!pings[e.address.trim()]) ping(e.address.trim());
    editing.value = null;
  }
}

async function remove(index: number, s: SavedServer) {
  if (!confirm(t("servers.confirmDelete", { name: s.name, ip: s.ip }))) return;
  await edit(() => api.serverRemove(props.versionId, index));
}

function move(index: number, delta: number) {
  edit(() => api.serverMove(props.versionId, index, index + delta));
}

async function join(s: SavedServer) {
  if (await game.launch(props.versionId, s.ip)) router.push("/log");
  else if (game.error) emit("message", false, errText(game.error));
}

function icon(s: SavedServer): string | null {
  const p = pings[s.ip];
  const b64 = (p?.state === "ok" && p.status.favicon) || s.icon;
  return b64 ? `data:image/png;base64,${b64}` : null;
}

function status(s: SavedServer): ServerStatus | null {
  const p = pings[s.ip];
  return p?.state === "ok" ? p.status : null;
}

function pingError(s: SavedServer): string | null {
  const p = pings[s.ip];
  return p?.state === "error" ? p.error : null;
}

function latencyClass(ms: number) {
  return ms < 100 ? "good" : ms < 300 ? "mid" : "bad";
}
</script>

<template>
  <div class="box-body bar">
    <input v-model="draft.name" class="input name-input" :placeholder="$t('servers.namePlaceholder')" />
    <input v-model="draft.address" class="input grow" :placeholder="$t('servers.addressPlaceholder')" @keydown.enter="add" />
    <button class="btn small primary" :disabled="busy || running || !draft.address.trim()" :title="lockedTip" @click="add">{{ $t("common.add") }}</button>
    <button class="btn small" :disabled="!servers?.length" @click="pingAll">{{ $t("servers.refresh") }}</button>
  </div>
  <div class="box-body hint muted small">
    <i18n-t keypath="servers.hint"><code>servers.dat</code></i18n-t><template v-if="running"> <b class="warn">{{ $t("servers.runningWarn") }}</b></template>
  </div>
  <table class="thread-table">
    <thead>
      <tr><th class="icon-col"></th><th>{{ $t("buttons.server") }}</th><th class="num">{{ $t("servers.players") }}</th><th class="num">{{ $t("servers.latency") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
    </thead>
    <tbody>
      <tr v-for="(s, i) in servers ?? []" :key="i + s.ip">
        <td class="icon-col">
          <img v-if="icon(s)" :src="icon(s)!" class="srv-icon" alt="" />
          <div v-else class="srv-icon blank"></div>
        </td>
        <td v-if="editing?.index === i" colspan="3">
          <div class="line">
            <input v-model="editing.name" class="input name-input" :placeholder="$t('export.name')" />
            <input v-model="editing.address" class="input grow" :placeholder="$t('servers.address')" @keydown.enter="saveEdit" />
          </div>
        </td>
        <template v-else>
          <td>
            <b>{{ s.name }}</b> <span class="muted small">{{ s.ip }}</span>
            <div v-if="status(s)" class="motd small">{{ status(s)!.motd || $t("servers.noMotd") }}</div>
            <div v-else-if="pingError(s)" class="small err">{{ pingError(s) }}</div>
            <div v-else-if="pings[s.ip]?.state === 'loading'" class="small muted">{{ $t("servers.querying") }}</div>
          </td>
          <td class="num">
            <template v-if="status(s)">
              <span :title="status(s)!.sample.join($t('common.listComma'))">{{ status(s)!.online }} / {{ status(s)!.max }}</span>
              <div class="muted small ver" :title="$t('servers.protocol', { n: status(s)!.protocol })">{{ status(s)!.version }}</div>
            </template>
            <span v-else class="muted">—</span>
          </td>
          <td class="num">
            <span v-if="status(s)" :class="latencyClass(status(s)!.latencyMs)">{{ status(s)!.latencyMs }} ms</span>
            <span v-else class="muted">—</span>
          </td>
        </template>
        <td class="num">
          <span v-if="editing?.index === i" class="row-actions">
            <button class="btn small primary" :disabled="busy" @click="saveEdit">{{ $t("common.save") }}</button>
            <button class="btn small" @click="editing = null">{{ $t("common.cancel") }}</button>
          </span>
          <span v-else class="row-actions">
            <button class="btn small primary" :disabled="!!game.launching" :title="$t('servers.joinTip')" @click="join(s)">{{ $t("servers.join") }}</button>
            <button class="btn small" :disabled="busy || running || i === 0" :title="lockedTip || $t('buttons.up')" @click="move(i, -1)">↑</button>
            <button class="btn small" :disabled="busy || running || i === (servers?.length ?? 0) - 1" :title="lockedTip || $t('buttons.down')" @click="move(i, 1)">↓</button>
            <button class="btn small" :disabled="busy || running" :title="lockedTip" @click="editing = { index: i, name: s.name, address: s.ip }">{{ $t("buttons.edit") }}</button>
            <button class="btn small" :disabled="busy || running" :title="lockedTip" @click="remove(i, s)">{{ $t("common.delete") }}</button>
          </span>
        </td>
      </tr>
      <tr v-if="servers && !servers.length"><td colspan="5" class="muted">{{ $t("servers.empty") }}</td></tr>
      <tr v-if="!servers"><td colspan="5" class="muted">{{ $t("servers.reading") }}</td></tr>
    </tbody>
  </table>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.hint { padding-top: 0; }
.line { display: flex; align-items: center; gap: 8px; }
.grow { flex: 1; min-width: 0; }
.name-input { width: 160px; }
.small { font-size: 12px; }
.warn { color: var(--gold); }
.err { color: var(--red); }
.icon-col { width: 44px; }
.srv-icon { width: 32px; height: 32px; image-rendering: pixelated; display: block; border-radius: var(--radius-sm); }
.srv-icon.blank { background: var(--bg-box-alt); border: 1px solid var(--border); }
.motd { white-space: pre-line; color: var(--text-muted); margin-top: 2px; word-break: break-word; }
.ver { max-width: 140px; margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row-actions { display: inline-flex; gap: 6px; }
.good { color: var(--grass); }
.mid { color: var(--gold); }
.bad { color: var(--red); }
</style>
