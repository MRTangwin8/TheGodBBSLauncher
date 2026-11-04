<script setup lang="ts">
import { ask } from "../utils";
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { DeviceCode } from "../api/types";
import { kindLabel, useAccountsStore } from "../stores/accounts";
import UserCard from "../components/UserCard.vue";
import AccountAvatar from "../components/AccountAvatar.vue";
import { isStore, storeBlocks } from "../edition";
import { errText, t } from "../i18n";

const store = useAccountsStore();
onMounted(() => store.load());

type Mode = "microsoft" | "offline" | "yggdrasil";
const mode = ref<Mode>("microsoft");
const busy = ref(false);
const message = ref<{ ok: boolean; text: string } | null>(null);

// 离线
const offlineName = ref("");
// 外置
const ygg = ref({ server: "", username: "", password: "" });
// 微软：和另外两种登录分开记"忙"，等浏览器授权期间也能切到离线 / 外置登录
const device = ref<DeviceCode | null>(null);
const copied = ref(false);
const msBusy = ref(false);
/** 每次开始登录加一；取消或重新开始后，旧的那次结果直接丢掉 */
let msAttempt = 0;

function done(text: string) {
  message.value = { ok: true, text };
}
function fail(e: unknown) {
  message.value = { ok: false, text: errText(e) };
}

async function addOffline() {
  busy.value = true;
  message.value = null;
  try {
    const a = await api.addOfflineAccount(offlineName.value);
    await store.added(a);
    offlineName.value = "";
    done(t("account.offlineAdded", { name: a.name }));
  } catch (e) {
    fail(e);
  } finally {
    busy.value = false;
  }
}

async function startMicrosoft() {
  const attempt = ++msAttempt;
  msBusy.value = true;
  message.value = null;
  copied.value = false;
  try {
    const dc = await api.microsoftLoginStart();
    if (attempt !== msAttempt) return;
    device.value = dc;
    // 一直等到用户在浏览器里完成授权（或过期 / 取消）
    const a = await api.microsoftLoginFinish();
    if (attempt !== msAttempt) return;
    await store.added(a);
    done(t("account.msLoggedIn", { name: a.name }));
  } catch (e) {
    if (attempt === msAttempt) fail(e);
  } finally {
    if (attempt === msAttempt) {
      device.value = null;
      msBusy.value = false;
    }
  }
}

async function copyAndOpen() {
  if (!device.value) return;
  try {
    await navigator.clipboard.writeText(device.value.userCode);
    copied.value = true;
  } catch {
    /* 复制失败不影响，用户可以手动输入 */
  }
  if (inTauri) await openUrl(device.value.verificationUri);
  else window.open(device.value.verificationUri, "_blank");
}

/** 不等后端轮询结束，界面立刻回到可以重新选择的状态 */
async function cancelMicrosoft() {
  msAttempt++;
  device.value = null;
  msBusy.value = false;
  try {
    await api.microsoftLoginCancel();
  } catch {
    /* 取消失败也无所谓，那次登录的结果已经不会被采用 */
  }
}

async function addYgg() {
  busy.value = true;
  message.value = null;
  try {
    const a = await api.addYggdrasilAccount(ygg.value.server, ygg.value.username, ygg.value.password);
    await store.added(a);
    ygg.value.password = "";
    done(t("account.yggLoggedIn", { name: a.name, server: a.serverName ?? "" }));
  } catch (e) {
    fail(e);
  } finally {
    busy.value = false;
  }
}

// 离线账号改名
const renamingId = ref<string | null>(null);
const newName = ref("");
function startRename(id: string, name: string) {
  renamingId.value = id;
  newName.value = name;
  message.value = null;
}
async function saveRename() {
  const id = renamingId.value;
  if (!id) return;
  try {
    await store.rename(id, newName.value);
    done(t("account.renamed", { name: newName.value.trim() }));
    renamingId.value = null;
  } catch (e) {
    fail(e);
  }
}

async function remove(id: string, name: string) {
  if (!(await ask(t("account.confirmDelete", { name })))) return;
  await store.remove(id);
}

// 商店版只能用正版账号，不显示另外两种
const tabs = computed(() =>
  (["microsoft", "offline", "yggdrasil"] as Mode[]).filter((k) => !storeBlocks(k)).map((key) => ({ key, label: t(`account.tab.${key}`) })),
);
</script>

<template>
  <div class="account">
    <div>
      <div class="box">
        <div class="box-title">{{ $t("account.mine") }} <span class="extra muted">{{ $t("account.clickToSelect") }}</span></div>
        <table class="thread-table">
          <tbody>
            <tr
              v-for="a in store.accounts"
              :key="a.id"
              class="acc-row"
              :class="{ current: a.id === store.selectedId }"
              @click="store.select(a.id)"
            >
              <td class="head-cell"><AccountAvatar :account="a" :size="32" /></td>
              <td>
                <span class="tag" :class="'acc-' + a.kind">{{ kindLabel(a.kind) }}</span>
                <form v-if="renamingId === a.id" class="rename" @click.stop @submit.prevent="saveRename">
                  <input v-model="newName" class="input" maxlength="16" :placeholder="$t('account.offlinePlaceholder')" @keyup.esc="renamingId = null" />
                  <button class="btn small primary" :disabled="!newName.trim()">{{ $t("common.save") }}</button>
                  <button type="button" class="btn small" @click="renamingId = null">{{ $t("common.cancel") }}</button>
                </form>
                <b v-else>{{ a.name }}</b>
                <span v-if="a.id === store.selectedId" class="current-mark">✔ {{ $t("account.current") }}</span>
                <span v-if="storeBlocks(a.kind)" class="tag blocked">{{ $t("account.storeBlocked") }}</span>
                <div v-if="renamingId === a.id" class="muted small">{{ $t("account.renameHint") }}</div>
                <div v-else class="muted small">{{ a.serverName ?? (a.kind === "microsoft" ? $t("account.msAccount") : $t("account.offlineOnly")) }}</div>
              </td>
              <td class="num">
                <a v-if="a.kind === 'offline' && renamingId !== a.id" class="del" @click.stop="startRename(a.id, a.name)">{{ $t("account.rename") }}</a>
                <a class="del" @click.stop="remove(a.id, a.name)">{{ $t("common.delete") }}</a>
              </td>
            </tr>
            <tr v-if="!store.accounts.length">
              <td colspan="3" class="muted">{{ $t("account.empty") }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="box">
        <div class="box-title">{{ $t("quickLaunch.addAccount") }}</div>
        <div v-if="isStore" class="box-body store-note">{{ $t("common.modTip") }}{{ $t("account.storeNote") }}</div>
        <div class="tabs">
          <a v-for="tab in tabs" :key="tab.key" :class="{ active: mode === tab.key }" @click="mode = tab.key">{{ tab.label }}</a>
        </div>
        <div class="box-body">
          <template v-if="mode === 'microsoft'">
            <div v-if="device" class="device">
              <p><i18n-t keypath="account.deviceHint"><b>{{ device.verificationUri }}</b></i18n-t></p>
              <div class="code">{{ device.userCode }}</div>
              <div class="device-actions">
                <button class="btn primary" @click="copyAndOpen">{{ copied ? $t("account.copiedOpening") : $t("account.copyAndOpen") }}</button>
                <button class="btn" @click="cancelMicrosoft">{{ $t("common.cancel") }}</button>
              </div>
              <p class="muted small">⏳ {{ $t("account.waiting", { n: Math.round(device.expiresIn / 60) }) }}</p>
            </div>
            <div v-else>
              <p>{{ $t("account.msHint") }}</p>
              <button class="btn primary" :disabled="msBusy" @click="startMicrosoft">{{ msBusy ? $t("account.requesting") : $t("account.msLogin") }}</button>
              <button v-if="msBusy" class="btn" @click="cancelMicrosoft">{{ $t("common.cancel") }}</button>
            </div>
          </template>

          <template v-else-if="mode === 'offline'">
            <form class="form-row" @submit.prevent="addOffline">
              <input v-model="offlineName" class="input" :placeholder="$t('account.offlinePlaceholder')" maxlength="16" />
              <button class="btn primary" :disabled="busy || !offlineName">{{ $t("common.add") }}</button>
            </form>
            <p class="muted small">{{ $t("account.offlineHint") }}</p>
          </template>

          <template v-else>
            <form class="ygg" @submit.prevent="addYgg">
              <label>{{ $t("account.yggServer") }}</label>
              <input v-model="ygg.server" class="input" :placeholder="$t('account.yggServerPlaceholder')" />
              <label>{{ $t("account.yggUser") }}</label>
              <input v-model="ygg.username" class="input" autocomplete="username" />
              <label>{{ $t("account.password") }}</label>
              <input v-model="ygg.password" class="input" type="password" autocomplete="current-password" />
              <span></span>
              <div>
                <button class="btn primary" :disabled="busy || !ygg.server || !ygg.username || !ygg.password">
                  {{ busy ? $t("account.loggingIn") : $t("shell.login") }}
                </button>
                <span class="muted small"> {{ $t("account.passwordHint") }}</span>
              </div>
            </form>
          </template>

          <p v-if="message" :class="message.ok ? 'ok' : 'error-text'" class="msg">{{ message.text }}</p>
        </div>
      </div>
    </div>
    <UserCard />
  </div>
</template>

<style scoped>
.account { display: grid; grid-template-columns: 1fr 230px; gap: 12px; align-items: start; }
.acc-row { cursor: pointer; }
.acc-row.current td { background: rgba(94, 156, 53, 0.1); }
.head-cell { width: 48px; }
.current-mark { color: var(--grass); font-size: 12px; margin-left: 6px; }
.small { font-size: 12px; }
.del { color: var(--text-muted); margin-left: 10px; }
.rename { display: inline-flex; gap: 6px; align-items: center; }
.rename .input { width: 180px; }
.tag.acc-microsoft { background: #2f7d32; }
.tag.acc-offline { background: #8b8b8b; }
.tag.acc-yggdrasil { background: #7b6a9e; }
.tag.blocked { background: var(--red); margin-left: 6px; }
.store-note { padding-bottom: 0; color: var(--text-muted); }
.tabs { display: flex; padding: 8px 12px 0; border-bottom: 1px solid var(--border); }
.tabs a { padding: 4px 14px; border: 1px solid var(--border); border-bottom: none; margin-right: 4px; background: var(--bg-box-alt); color: var(--text); border-radius: 3px 3px 0 0; }
.tabs a:hover { text-decoration: none; }
.tabs a.active { background: var(--bg-box); font-weight: bold; color: var(--brown-dark); position: relative; top: 1px; }
.form-row { display: flex; gap: 8px; }
.form-row .input { flex: 1; max-width: 320px; }
.ygg { display: grid; grid-template-columns: 90px 1fr; gap: 8px 10px; align-items: center; max-width: 520px; }
.ygg label { text-align: right; color: var(--text-muted); }
.device { text-align: center; }
.code {
  display: inline-block;
  margin: 8px 0 12px;
  padding: 8px 20px;
  font-family: var(--font-pixel);
  font-size: 30px;
  font-weight: bold;
  letter-spacing: 6px;
  color: var(--brown-dark);
  background: var(--bg-box-alt);
  border: 2px dashed var(--border-strong);
  user-select: all;
}
.device-actions { display: flex; gap: 8px; justify-content: center; }
.msg { margin: 10px 0 0; }
.ok { color: var(--grass); }
</style>
