<script setup lang="ts">
import { onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { DeviceCode } from "../api/types";
import { kindLabel, useAccountsStore } from "../stores/accounts";
import UserCard from "../components/UserCard.vue";
import AccountAvatar from "../components/AccountAvatar.vue";
import { isStore, storeBlocks } from "../edition";

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
// 微软
const device = ref<DeviceCode | null>(null);
const copied = ref(false);

function done(text: string) {
  message.value = { ok: true, text };
}
function fail(e: unknown) {
  message.value = { ok: false, text: String(e) };
}

async function addOffline() {
  busy.value = true;
  message.value = null;
  try {
    const a = await api.addOfflineAccount(offlineName.value);
    await store.added(a);
    offlineName.value = "";
    done(`离线账号 ${a.name} 已添加`);
  } catch (e) {
    fail(e);
  } finally {
    busy.value = false;
  }
}

async function startMicrosoft() {
  busy.value = true;
  message.value = null;
  copied.value = false;
  try {
    device.value = await api.microsoftLoginStart();
    // 一直等到用户在浏览器里完成授权
    const a = await api.microsoftLoginFinish();
    await store.added(a);
    done(`欢迎回来，${a.name}！正版登录成功`);
  } catch (e) {
    fail(e);
  } finally {
    device.value = null;
    busy.value = false;
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

async function cancelMicrosoft() {
  await api.microsoftLoginCancel();
}

async function addYgg() {
  busy.value = true;
  message.value = null;
  try {
    const a = await api.addYggdrasilAccount(ygg.value.server, ygg.value.username, ygg.value.password);
    await store.added(a);
    ygg.value.password = "";
    done(`外置账号 ${a.name}（${a.serverName}）登录成功`);
  } catch (e) {
    fail(e);
  } finally {
    busy.value = false;
  }
}

async function remove(id: string, name: string) {
  if (!confirm(`确定删除账号 ${name} 吗？`)) return;
  await store.remove(id);
}

// 商店版只能用正版账号，不显示另外两种
const tabs: { key: Mode; label: string }[] = [
  { key: "microsoft" as Mode, label: "微软正版" },
  { key: "offline" as Mode, label: "离线账号" },
  { key: "yggdrasil" as Mode, label: "外置登录" },
].filter((t) => !storeBlocks(t.key));
</script>

<template>
  <div class="account">
    <div>
      <div class="box">
        <div class="box-title">我的账号 <span class="extra muted">点击设为当前账号</span></div>
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
                <span class="tag" :class="'acc-' + a.kind">{{ kindLabel[a.kind] }}</span>
                <b>{{ a.name }}</b>
                <span v-if="a.id === store.selectedId" class="current-mark">✔ 当前</span>
                <span v-if="storeBlocks(a.kind)" class="tag blocked">商店版不能用</span>
                <div class="muted small">{{ a.serverName ?? (a.kind === "microsoft" ? "Microsoft 账号" : "仅限单机和离线服务器") }}</div>
              </td>
              <td class="num"><a class="del" @click.stop="remove(a.id, a.name)">删除</a></td>
            </tr>
            <tr v-if="!store.accounts.length">
              <td colspan="3" class="muted">还没有账号。在下面添加一个，才能启动游戏。</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="box">
        <div class="box-title">添加账号</div>
        <div v-if="isStore" class="box-body store-note">【版主提示】微软商店版只能使用正版账号：请用购买了 Minecraft Java 版的微软账号登录。</div>
        <div class="tabs">
          <a v-for="t in tabs" :key="t.key" :class="{ active: mode === t.key }" @click="mode = t.key">{{ t.label }}</a>
        </div>
        <div class="box-body">
          <template v-if="mode === 'microsoft'">
            <div v-if="device" class="device">
              <p>请在浏览器中打开 <b>{{ device.verificationUri }}</b>，输入下面的代码并用你的微软账号登录：</p>
              <div class="code">{{ device.userCode }}</div>
              <div class="device-actions">
                <button class="btn primary" @click="copyAndOpen">{{ copied ? "已复制，正在打开浏览器" : "复制代码并打开浏览器" }}</button>
                <button class="btn" @click="cancelMicrosoft">取消</button>
              </div>
              <p class="muted small">⏳ 等待你在浏览器中完成登录…（代码 {{ Math.round(device.expiresIn / 60) }} 分钟内有效）</p>
            </div>
            <div v-else>
              <p>使用购买了 Minecraft Java 版的微软账号登录。启动器不会接触你的微软密码。</p>
              <button class="btn primary" :disabled="busy" @click="startMicrosoft">{{ busy ? "请求中…" : "微软账号登录" }}</button>
            </div>
          </template>

          <template v-else-if="mode === 'offline'">
            <form class="form-row" @submit.prevent="addOffline">
              <input v-model="offlineName" class="input" placeholder="游戏名（3~16 位字母数字下划线）" maxlength="16" />
              <button class="btn primary" :disabled="busy || !offlineName">添加</button>
            </form>
            <p class="muted small">离线账号只能进单人游戏和开启了离线模式的服务器。</p>
          </template>

          <template v-else>
            <form class="ygg" @submit.prevent="addYgg">
              <label>认证服务器</label>
              <input v-model="ygg.server" class="input" placeholder="如 littleskin.cn/api/yggdrasil 或皮肤站首页地址" />
              <label>邮箱 / 用户名</label>
              <input v-model="ygg.username" class="input" autocomplete="username" />
              <label>密码</label>
              <input v-model="ygg.password" class="input" type="password" autocomplete="current-password" />
              <span></span>
              <div>
                <button class="btn primary" :disabled="busy || !ygg.server || !ygg.username || !ygg.password">
                  {{ busy ? "登录中…" : "登录" }}
                </button>
                <span class="muted small"> 密码只用于这一次登录，不会被保存</span>
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
.del { color: var(--text-muted); }
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
