<script setup lang="ts">
import { computed, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { DeviceCode, Settings } from "../api/types";
import { useAccountsStore } from "../stores/accounts";
import { useSettingsStore } from "../stores/settings";
import { isStore, storeBlocks } from "../edition";

// 首次启动引导：选风格 → 登录账号 → 完成。每一步都能跳过，随时可以「跳过全部」。
// 选的风格立刻生效（可以直接看到效果）；之后都能在「主题中心」「个人空间」里再改。
const settings = useSettingsStore();
const accounts = useAccountsStore();
const emit = defineEmits<{ close: [] }>();

const step = ref(0); // 0 欢迎 / 1 风格 / 2 账号 / 3 完成
const busy = ref(false);
const message = ref<{ ok: boolean; text: string } | null>(null);

const layouts: { key: Settings["layout"]; name: string; desc: string }[] = [
  { key: "classic", name: "经典论坛", desc: "横幅 + 顶部导航，原汁原味的论坛版块" },
  { key: "sidebar", name: "木板侧栏", desc: "木板侧边栏，像素天空和草地" },
  { key: "modern", name: "完全现代", desc: "图标导航条 + 亚克力卡片" },
];
const palettes = [
  { key: "mcbbs", name: "MCBBS 经典", color: "#5e9c35" },
  { key: "grass", name: "草方块", color: "#4fa52f" },
  { key: "nether", name: "下界", color: "#d4502a" },
  { key: "end", name: "末地", color: "#8e5fc0" },
  { key: "ocean", name: "海洋", color: "#23a093" },
  { key: "cherry", name: "樱花", color: "#df6a9c" },
  { key: "stone", name: "石头", color: "#5b7083" },
];
const modes: { key: Settings["theme"]; name: string }[] = [
  { key: "light", name: "浅色" },
  { key: "dark", name: "深色" },
  { key: "system", name: "跟随系统" },
];

async function patch(p: Partial<Settings>) {
  if (settings.settings) await settings.save({ ...settings.settings, ...p });
}

// ---- 账号 ----
type Mode = "microsoft" | "offline" | "yggdrasil";
const mode = ref<Mode>("microsoft");
const tabs: { key: Mode; label: string }[] = [
  { key: "microsoft" as Mode, label: "微软正版" },
  { key: "offline" as Mode, label: "离线账号" },
  { key: "yggdrasil" as Mode, label: "外置登录" },
].filter((t) => !storeBlocks(t.key));
const offlineName = ref("");
const ygg = ref({ server: "", username: "", password: "" });
const device = ref<DeviceCode | null>(null);
const copied = ref(false);
const addedName = ref("");

async function run(task: () => Promise<{ name: string }>) {
  busy.value = true;
  message.value = null;
  try {
    const a = await task();
    addedName.value = a.name;
    step.value = 3;
  } catch (e) {
    message.value = { ok: false, text: String(e) };
  } finally {
    device.value = null;
    busy.value = false;
  }
}

const startMicrosoft = () =>
  run(async () => {
    copied.value = false;
    device.value = await api.microsoftLoginStart();
    const a = await api.microsoftLoginFinish();
    await accounts.added(a);
    return a;
  });
const addOffline = () =>
  run(async () => {
    const a = await api.addOfflineAccount(offlineName.value);
    await accounts.added(a);
    return a;
  });
const addYgg = () =>
  run(async () => {
    const a = await api.addYggdrasilAccount(ygg.value.server, ygg.value.username, ygg.value.password);
    ygg.value.password = "";
    await accounts.added(a);
    return a;
  });

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

async function finish() {
  if (busy.value) await api.microsoftLoginCancel().catch(() => {});
  await patch({ onboarded: true }).catch(() => {});
  emit("close");
}

const title = computed(() => ["欢迎来到神权论坛", "挑个喜欢的风格", "登录账号", "都准备好了"][step.value]);
</script>

<template>
  <div class="mask">
    <div class="dialog box" role="dialog" aria-modal="true" aria-label="首次启动引导">
      <div class="box-title">{{ title }} <span class="extra muted">{{ step + 1 }} / 4</span></div>
      <div class="box-body">
        <template v-if="step === 0">
          <p>这是神权论坛启动器（MCBBS 纪念版）。花一分钟挑好外观、登录账号，就能开始玩了。</p>
          <p class="muted small">每一步都可以跳过，之后也都能在「主题中心」和「个人空间」里再改。</p>
        </template>

        <template v-else-if="step === 1">
          <div v-if="settings.settings" class="pick">
            <div class="label muted">布局</div>
            <div class="cards">
              <button v-for="l in layouts" :key="l.key" class="card" :class="{ active: settings.settings.layout === l.key }" @click="patch({ layout: l.key })">
                <b>{{ l.name }}</b><span class="muted small">{{ l.desc }}</span>
              </button>
            </div>
            <div class="label muted">配色</div>
            <div class="swatches">
              <button v-for="p in palettes" :key="p.key" class="swatch" :class="{ active: settings.settings.palette === p.key }" :title="p.name" @click="patch({ palette: p.key })">
                <i :style="{ background: p.color }" />{{ p.name }}
              </button>
            </div>
            <div class="label muted">明暗</div>
            <div class="swatches">
              <button v-for="m in modes" :key="m.key" class="swatch" :class="{ active: settings.settings.theme === m.key }" @click="patch({ theme: m.key })">{{ m.name }}</button>
            </div>
          </div>
        </template>

        <template v-else-if="step === 2">
          <div class="tabs">
            <a v-for="t in tabs" :key="t.key" :class="{ active: mode === t.key }" @click="!busy && (mode = t.key)">{{ t.label }}</a>
          </div>
          <div class="acc-body">
            <template v-if="mode === 'microsoft'">
              <p class="small">用微软账号登录，可以联机、使用正版皮肤。点下面的按钮后，在浏览器里输入验证码授权即可。</p>
              <p v-if="isStore" class="small">微软商店版只能使用正版账号，需要这个微软账号购买了 Minecraft Java 版。</p>
              <template v-if="device">
                <p>验证码：<b class="code">{{ device.userCode }}</b></p>
                <button class="btn" @click="copyAndOpen">{{ copied ? "已复制，已打开浏览器" : "复制验证码并打开授权页面" }}</button>
                <p class="muted small">等待你在浏览器里完成授权…</p>
              </template>
              <button v-else class="btn primary" :disabled="busy" @click="startMicrosoft">开始微软登录</button>
            </template>
            <template v-else-if="mode === 'offline'">
              <p class="small">离线账号不需要联网验证，只能玩单人和不验证正版的服务器。</p>
              <div class="form-row"><input v-model="offlineName" class="input" placeholder="玩家名（3~16 个字母数字或下划线）" :disabled="busy" @keyup.enter="addOffline" /><button class="btn primary" :disabled="busy || !offlineName.trim()" @click="addOffline">添加</button></div>
            </template>
            <template v-else>
              <p class="small">用第三方认证服务器（Authlib-Injector / Yggdrasil）登录。</p>
              <input v-model="ygg.server" class="input" placeholder="认证服务器地址" :disabled="busy" />
              <input v-model="ygg.username" class="input" placeholder="邮箱 / 用户名" :disabled="busy" />
              <input v-model="ygg.password" class="input" type="password" placeholder="密码" :disabled="busy" />
              <button class="btn primary" :disabled="busy || !ygg.server.trim() || !ygg.username.trim() || !ygg.password" @click="addYgg">登录</button>
            </template>
            <p v-if="message" :class="message.ok ? 'ok-text' : 'error-text'" class="small break">{{ message.text }}</p>
          </div>
        </template>

        <template v-else>
          <p v-if="addedName" class="ok-text">已登录：{{ addedName }}</p>
          <p v-else class="muted">还没有登录账号。到「个人空间」添加账号后才能启动游戏。</p>
          <p>接下来到「下载中心」安装一个游戏版本，回首页点启动就行。</p>
        </template>
      </div>
      <div class="actions">
        <a v-if="step < 3" class="skip" @click="finish">跳过全部</a>
        <span class="grow" />
        <button v-if="step > 0 && step < 3" class="btn" :disabled="busy" @click="step--">上一步</button>
        <button v-if="step === 0" class="btn primary" @click="step = 1">开始</button>
        <button v-else-if="step === 1" class="btn primary" @click="step = 2">下一步</button>
        <button v-else-if="step === 2" class="btn" :disabled="busy" @click="step = 3">跳过这一步</button>
        <button v-else class="btn primary" @click="finish">开始使用</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.5); display: flex; align-items: center; justify-content: center; z-index: 40; }
.dialog { width: 600px; max-width: calc(100vw - 32px); margin: 0; }
.small { font-size: 12px; margin: 4px 0; }
.break { word-break: break-all; }
.grow { flex: 1; }
.label { margin: 10px 0 4px; font-size: 12px; }
.cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.card, .swatch { font: inherit; color: var(--text); background: var(--bg-box-alt); border: 2px solid var(--border); border-radius: var(--radius-sm); cursor: pointer; }
.card { display: flex; flex-direction: column; gap: 4px; padding: 8px; text-align: left; }
.swatches { display: flex; flex-wrap: wrap; gap: 6px; }
.swatch { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; }
.swatch i { width: 14px; height: 14px; border-radius: 50%; border: 1px solid rgba(0, 0, 0, 0.3); }
.card.active, .swatch.active { border-color: var(--grass); box-shadow: 0 0 0 3px color-mix(in srgb, var(--grass) 25%, transparent); }
.tabs { display: flex; border-bottom: 1px solid var(--border); margin-bottom: 8px; }
.tabs a { padding: 4px 14px; border: 1px solid var(--border); border-bottom: none; margin-right: 4px; background: var(--bg-box-alt); color: var(--text); border-radius: 3px 3px 0 0; cursor: pointer; }
.tabs a.active { background: var(--bg-box); font-weight: bold; position: relative; top: 1px; }
.acc-body { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; min-height: 130px; }
.acc-body .input { width: 100%; }
.form-row { display: flex; gap: 8px; width: 100%; }
.form-row .input { flex: 1; }
.code { font-size: 22px; letter-spacing: 3px; color: var(--grass); }
.ok-text { color: var(--grass); }
.actions { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
.skip { font-size: 12px; cursor: pointer; }
</style>
