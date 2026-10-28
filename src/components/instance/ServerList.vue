<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import { api } from "../../api";
import type { SavedServer, ServerStatus } from "../../api/types";
import { useGameStore } from "../../stores/game";

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

const lockedTip = computed(() => (props.running ? "游戏开着时改了也会被游戏盖掉，先退出游戏" : ""));

onMounted(load);

async function load() {
  try {
    servers.value = await api.serverList(props.versionId);
    pingAll();
  } catch (e) {
    servers.value = [];
    emit("message", false, `${e}`);
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
    pings[address] = { state: "error", error: `${e}` };
  }
}

async function edit(run: () => Promise<SavedServer[]>, done?: string) {
  busy.value = true;
  try {
    servers.value = await run();
    if (done) emit("message", true, done);
    return true;
  } catch (e) {
    emit("message", false, `${e}`);
    return false;
  } finally {
    busy.value = false;
  }
}

async function add() {
  const { name, address } = draft.value;
  if (!address.trim()) return;
  if (await edit(() => api.serverAdd(props.versionId, name, address), `已添加「${name.trim() || address.trim()}」，游戏的多人游戏列表里也能看到。`)) {
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
  if (!confirm(`从列表里删除「${s.name}」（${s.ip}）？`)) return;
  await edit(() => api.serverRemove(props.versionId, index));
}

function move(index: number, delta: number) {
  edit(() => api.serverMove(props.versionId, index, index + delta));
}

async function join(s: SavedServer) {
  if (await game.launch(props.versionId, s.ip)) router.push("/log");
  else if (game.error) emit("message", false, game.error);
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
    <input v-model="draft.name" class="input name-input" placeholder="名称（可不填）" />
    <input v-model="draft.address" class="input grow" placeholder="服务器地址，例如 mc.example.com 或 1.2.3.4:25565" @keydown.enter="add" />
    <button class="btn small primary" :disabled="busy || running || !draft.address.trim()" :title="lockedTip" @click="add">添加</button>
    <button class="btn small" :disabled="!servers?.length" @click="pingAll">刷新状态</button>
  </div>
  <div class="box-body hint muted small">
    和游戏里「多人游戏」的列表是同一份（<code>servers.dat</code>）。<template v-if="running"><b class="warn">游戏正开着，先退出游戏再改列表。</b></template>
  </div>
  <table class="thread-table">
    <thead>
      <tr><th class="icon-col"></th><th>服务器</th><th class="num">人数</th><th class="num">延迟</th><th class="num">操作</th></tr>
    </thead>
    <tbody>
      <tr v-for="(s, i) in servers ?? []" :key="i + s.ip">
        <td class="icon-col">
          <img v-if="icon(s)" :src="icon(s)!" class="srv-icon" alt="" />
          <div v-else class="srv-icon blank"></div>
        </td>
        <td v-if="editing?.index === i" colspan="3">
          <div class="line">
            <input v-model="editing.name" class="input name-input" placeholder="名称" />
            <input v-model="editing.address" class="input grow" placeholder="地址" @keydown.enter="saveEdit" />
          </div>
        </td>
        <template v-else>
          <td>
            <b>{{ s.name }}</b> <span class="muted small">{{ s.ip }}</span>
            <div v-if="status(s)" class="motd small">{{ status(s)!.motd || "（没有简介）" }}</div>
            <div v-else-if="pingError(s)" class="small err">{{ pingError(s) }}</div>
            <div v-else-if="pings[s.ip]?.state === 'loading'" class="small muted">正在查询…</div>
          </td>
          <td class="num">
            <template v-if="status(s)">
              <span :title="status(s)!.sample.join('、')">{{ status(s)!.online }} / {{ status(s)!.max }}</span>
              <div class="muted small ver" :title="'协议号 ' + status(s)!.protocol">{{ status(s)!.version }}</div>
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
            <button class="btn small primary" :disabled="busy" @click="saveEdit">保存</button>
            <button class="btn small" @click="editing = null">取消</button>
          </span>
          <span v-else class="row-actions">
            <button class="btn small primary" :disabled="!!game.launching" title="启动这个版本并直接进入服务器" @click="join(s)">加入</button>
            <button class="btn small" :disabled="busy || running || i === 0" :title="lockedTip || '上移'" @click="move(i, -1)">↑</button>
            <button class="btn small" :disabled="busy || running || i === (servers?.length ?? 0) - 1" :title="lockedTip || '下移'" @click="move(i, 1)">↓</button>
            <button class="btn small" :disabled="busy || running" :title="lockedTip" @click="editing = { index: i, name: s.name, address: s.ip }">编辑</button>
            <button class="btn small" :disabled="busy || running" :title="lockedTip" @click="remove(i, s)">删除</button>
          </span>
        </td>
      </tr>
      <tr v-if="servers && !servers.length"><td colspan="5" class="muted">列表是空的，在上面添加一个服务器吧</td></tr>
      <tr v-if="!servers"><td colspan="5" class="muted">正在读取服务器列表…</td></tr>
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
