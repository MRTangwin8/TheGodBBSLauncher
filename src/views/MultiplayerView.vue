<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri, onMultiplayerState } from "../api";
import type { MpState, MpStatus } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { useGameStore } from "../stores/game";
import { useAccountsStore } from "../stores/accounts";
import { useSettingsStore } from "../stores/settings";
import { errText, t } from "../i18n";

// 联机：接入陶瓦联机（Terracotta）。按它的许可证例外条款，页面底部必须显眼地标注它的版权信息，不能去掉
const TERRACOTTA_URL = "https://github.com/burningtnt/Terracotta";

const versions = useVersionsStore();
const game = useGameStore();
const accounts = useAccountsStore();
const router = useRouter();
const settingsStore = useSettingsStore();

const status = ref<MpStatus | null>(null);
const mp = ref<MpState | null>(null);
const roomInput = ref("");
const busy = ref(false);
const installing = ref(false);
const error = ref<string | null>(null);
const copied = ref(false);

/** 陶瓦的 index 单调递增：只接受更新的状态；陶瓦退出（stopped）或重启后从头算 */
function apply(s: MpState | null) {
  const cur = mp.value;
  if (!s || !cur || s.state === "stopped" || cur.state === "stopped" || s.index > cur.index) mp.value = s;
}

async function refresh() {
  try {
    status.value = await api.mpStatus();
    mp.value = status.value.state;
  } catch (e) {
    error.value = errText(e);
  }
}

let unlisten: (() => void) | null = null;
let timer: number | undefined;
onMounted(async () => {
  if (!versions.installed.length) versions.loadInstalled().catch(() => {});
  await refresh();
  unlisten = await onMultiplayerState(apply);
  // 浏览器预览里没有事件推送，轮询假数据
  if (!inTauri) timer = window.setInterval(() => mp.value && api.mpStatus().then((s) => apply(s.state)), 1000);
});
onUnmounted(() => {
  unlisten?.();
  window.clearInterval(timer);
});

const phase = computed(() => {
  const s = mp.value?.state ?? "waiting";
  if (s.startsWith("host-")) return "host";
  if (s.startsWith("guest-")) return "guest";
  if (s === "exception") return "error";
  return "idle";
});
const me = computed(() => accounts.selected?.name ?? null);

async function install() {
  error.value = null;
  installing.value = true;
  const jobId = t("mp.job");
  const job = versions.startJob(jobId);
  try {
    await api.mpInstall(jobId);
    job.status = "done";
    job.doneText = t("mp.installDone");
    await refresh();
  } catch (e) {
    job.status = "failed";
    job.error = errText(e);
    error.value = errText(e);
  } finally {
    installing.value = false;
  }
}

async function run(action: () => Promise<MpState | void>) {
  error.value = null;
  busy.value = true;
  try {
    const s = await action();
    if (s) apply(s);
    else await refresh();
  } catch (e) {
    error.value = errText(e);
  } finally {
    busy.value = false;
  }
}

const host = () => run(() => api.mpHost());

// 开房间前游戏还没运行：提醒一下，可以直接启动上次玩的版本
const askLaunch = ref(false);
const launchTarget = computed(() => {
  const last = settingsStore.settings?.lastVersion;
  return versions.installed.find((v) => v.id === last)?.id ?? versions.installed[0]?.id ?? null;
});
function tryHost() {
  if (!game.running.length) askLaunch.value = true;
  else host();
}
async function launchForHost() {
  askLaunch.value = false;
  const id = launchTarget.value;
  if (!id) return router.push("/download");
  if (!accounts.selected) return router.push("/account");
  error.value = null;
  if (!(await game.launch(id))) error.value = game.error ? errText(game.error) : null;
}
function hostAnyway() {
  askLaunch.value = false;
  host();
}
const join = () => run(() => api.mpJoin(roomInput.value));
const leave = () => run(() => api.mpLeave());

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch {
    copied.value = false;
  }
}

async function openLink(url: string) {
  if (inTauri) await openUrl(url);
  else window.open(url, "_blank", "noopener,noreferrer");
}

const stepKey = computed(() => `mp.state.${mp.value?.state ?? "waiting"}`);
/** 陶瓦定义的异常类型 0~5，以后新增的类型显示通用说明 */
const errorKey = computed(() => {
  const n = mp.value?.error ?? -1;
  return n >= 0 && n <= 5 ? `mp.error.e${n}` : "mp.error.unknown";
});
</script>

<template>
  <div class="mp">
    <div class="box">
      <div class="box-title">{{ $t("mp.title") }} <span class="extra muted">{{ $t("mp.subtitle") }}</span></div>

      <div v-if="!status" class="box-body muted">{{ $t("mp.loading") }}</div>

      <div v-else-if="!status.supported" class="box-body">{{ $t("common.modTip") }}{{ $t("mp.unsupported") }}</div>

      <div v-else-if="!status.installed" class="box-body intro">
        <p>{{ $t("mp.intro") }}</p>
        <ul>
          <li>{{ $t("mp.introRoom") }}</li>
          <li>{{ $t("mp.introP2p") }}</li>
          <li>{{ $t("mp.introPrivacy") }}</li>
        </ul>
        <p class="region">{{ $t("common.modTip") }}{{ $t("mp.region") }}</p>
        <button class="btn primary" :disabled="installing" @click="install">
          {{ installing ? $t("mp.installing") : $t("mp.enable", { version: status.version }) }}
        </button>
      </div>

      <template v-else>
        <!-- 待机：开房间 / 加入房间 -->
        <div v-if="phase === 'idle'" class="box-body choose">
          <section>
            <h3>{{ $t("mp.hostTitle") }}</h3>
            <ol class="steps">
              <li>{{ $t("mp.hostStep1") }}</li>
              <li>{{ $t("mp.hostStep2") }}</li>
              <li>{{ $t("mp.hostStep3") }}</li>
            </ol>
            <p v-if="!game.running.length" class="muted small">{{ $t("mp.noGameRunning") }}</p>
            <button class="btn primary" :disabled="busy" @click="tryHost">{{ $t("mp.host") }}</button>
            <div v-if="askLaunch" class="ask">
              <b>{{ $t("mp.notRunningTitle") }}</b>
              <p class="muted small">{{ $t("mp.notRunningText") }}</p>
              <div class="ask-actions">
                <button class="btn small primary" :disabled="!!game.launching" @click="launchForHost">
                  {{ launchTarget ? $t("mp.launchVersion", { version: launchTarget }) : $t("mp.goDownload") }}
                </button>
                <button class="btn small" @click="hostAnyway">{{ $t("mp.alreadyRunning") }}</button>
                <button class="btn small" @click="askLaunch = false">{{ $t("common.cancel") }}</button>
              </div>
            </div>
          </section>
          <section>
            <h3>{{ $t("mp.joinTitle") }}</h3>
            <p class="muted small">{{ $t("mp.joinHint") }}</p>
            <div class="join-row">
              <input v-model="roomInput" class="input code" placeholder="U/XXXX-XXXX-XXXX-XXXX" spellcheck="false" @keyup.enter="roomInput.trim() && join()" />
              <button class="btn primary" :disabled="busy || !roomInput.trim()" @click="join">{{ $t("mp.join") }}</button>
            </div>
          </section>
        </div>

        <!-- 房主 -->
        <div v-else-if="phase === 'host'" class="box-body">
          <p class="step">{{ $t(stepKey) }}</p>
          <template v-if="mp?.state === 'host-scanning'">
            <p class="muted">{{ $t("mp.scanningHint") }}</p>
          </template>
          <template v-if="mp?.room">
            <div class="room">
              <span class="muted">{{ $t("mp.roomCode") }}</span>
              <b class="code big">{{ mp.room }}</b>
              <button class="btn small" @click="copy(mp.room)">{{ copied ? $t("mp.copied") : $t("mp.copy") }}</button>
            </div>
            <p class="muted small">{{ $t("mp.shareHint") }}</p>
          </template>
          <p class="muted small">{{ $t("mp.closeHint") }}</p>
          <button class="btn" :disabled="busy" @click="leave">{{ $t("mp.closeRoom") }}</button>
        </div>

        <!-- 房客 -->
        <div v-else-if="phase === 'guest'" class="box-body">
          <p class="step">{{ $t(stepKey) }}</p>
          <p v-if="mp?.difficulty" :class="mp.difficulty === 'TOUGH' ? 'error-text' : 'muted'">
            {{ $t("mp.difficultyLabel") }}{{ $t(`mp.difficulty.${mp.difficulty}`) }}
            <template v-if="mp.difficulty === 'TOUGH'">{{ $t("mp.toughHint") }}</template>
          </p>
          <template v-if="mp?.state === 'guest-ok'">
            <p>
              <i18n-t keypath="mp.joinedHint"><b>{{ $t("mp.lobbyName") }}</b></i18n-t>
            </p>
            <p v-if="mp.address" class="muted small">
              {{ $t("mp.directAddress") }} <b class="code">{{ mp.address }}</b>
              <a @click="copy(mp.address)">{{ copied ? $t("mp.copied") : $t("mp.copy") }}</a>
            </p>
          </template>
          <button class="btn" :disabled="busy" @click="leave">{{ $t("mp.leaveRoom") }}</button>
        </div>

        <!-- 异常 -->
        <div v-else class="box-body">
          <p class="error-text">{{ $t(errorKey) }}</p>
          <button class="btn" :disabled="busy" @click="leave">{{ $t("mp.backToStart") }}</button>
        </div>

        <!-- 成员列表 -->
        <table v-if="mp?.profiles.length" class="thread-table members">
          <tbody>
            <tr v-for="(p, i) in mp.profiles" :key="i">
              <td><span class="tag" :class="'kind-' + p.kind">{{ $t(`mp.kind.${p.kind}`) }}</span> {{ p.name || $t("mp.anonymous") }}</td>
            </tr>
          </tbody>
        </table>
        <p v-if="phase === 'idle' && me" class="box-body muted small name-note">{{ $t("mp.nameNote", { name: me }) }}</p>
        <p v-if="phase === 'idle'" class="box-body muted small name-note">{{ $t("mp.region") }}</p>
      </template>

      <p v-if="error" class="box-body error-text">{{ error }}</p>
    </div>

    <!-- 陶瓦联机许可证例外条款要求的版权标注，不能去掉 -->
    <p class="credit muted small">
      <i18n-t keypath="mp.credit"><a @click="openLink(TERRACOTTA_URL)">Terracotta | 陶瓦联机</a></i18n-t>
    </p>
  </div>
</template>

<style scoped>
.mp { max-width: 860px; }
.intro ul { margin: 8px 0 14px; padding-left: 20px; line-height: 1.8; }
.choose { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; }
.choose h3 { margin: 0 0 8px; font-size: 14px; color: var(--brown-dark); }
.steps { margin: 0 0 10px; padding-left: 20px; line-height: 1.8; }
.join-row { display: flex; gap: 8px; margin-top: 10px; }
.join-row .input { flex: 1; min-width: 0; }
.code { font-family: ui-monospace, Consolas, monospace; letter-spacing: 0.5px; }
.code.big { font-size: 20px; color: var(--text); }
.room { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; margin: 10px 0 6px; }
.step { font-weight: bold; }
.small { font-size: 12px; }
.members { margin: 0; }
.tag.kind-HOST { background: var(--gold); }
.tag.kind-LOCAL { background: var(--grass); }
.tag.kind-GUEST { background: var(--link); }
.name-note { padding-top: 0; }
.credit { margin: 8px 2px; }
.region { margin: 0 0 14px; color: var(--text-muted); }
.ask { margin-top: 12px; padding: 10px 12px; border: 1px dashed var(--border); border-radius: var(--radius-sm); background: var(--bg-box-alt); }
.ask p { margin: 4px 0 8px; }
.ask-actions { display: flex; gap: 6px; flex-wrap: wrap; }
@media (max-width: 720px) {
  .choose { grid-template-columns: 1fr; }
}
</style>
