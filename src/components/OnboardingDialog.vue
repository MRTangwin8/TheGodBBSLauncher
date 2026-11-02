<script setup lang="ts">
import { computed, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, inTauri } from "../api";
import type { DeviceCode, Settings } from "../api/types";
import { useAccountsStore } from "../stores/accounts";
import { useSettingsStore } from "../stores/settings";
import { isStore, storeBlocks } from "../edition";
import { LOCALES, LOCALE_NAMES, errText, t } from "../i18n";

// 首次启动引导：选风格 → 登录账号 → 完成。每一步都能跳过，随时可以「跳过全部」。
// 选的语言和风格立刻生效：整个窗口当场换成所选的样子（风格这一步遮罩调淡，能看清背后）；之后都能在「主题中心」「设置」「个人空间」里再改。
const settings = useSettingsStore();
const accounts = useAccountsStore();
const emit = defineEmits<{ close: [] }>();

const step = ref(0); // 0 欢迎 / 1 风格 / 2 账号 / 3 完成
const busy = ref(false);
const message = ref<{ ok: boolean; text: string } | null>(null);

const layouts: Settings["layout"][] = ["classic", "sidebar", "modern"];
const palettes = [
  { key: "mcbbs", color: "#5e9c35" },
  { key: "grass", color: "#4fa52f" },
  { key: "nether", color: "#d4502a" },
  { key: "end", color: "#8e5fc0" },
  { key: "ocean", color: "#23a093" },
  { key: "cherry", color: "#df6a9c" },
  { key: "stone", color: "#5b7083" },
];
const modes: Settings["theme"][] = ["light", "dark", "system"];
const languages: Settings["language"][] = ["auto", ...LOCALES];
const languageName = (l: Settings["language"]) => (l === "auto" ? t("language.auto") : LOCALE_NAMES[l]);

async function patch(p: Partial<Settings>) {
  if (settings.settings) await settings.save({ ...settings.settings, ...p });
}

// ---- 账号 ----
type Mode = "microsoft" | "offline" | "yggdrasil";
const mode = ref<Mode>("microsoft");
const tabs = computed(() =>
  (["microsoft", "offline", "yggdrasil"] as Mode[]).filter((k) => !storeBlocks(k)).map((key) => ({ key, label: t(`account.tab.${key}`) })),
);
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
    message.value = { ok: false, text: errText(e) };
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

const title = computed(() => t(`onboarding.title.${step.value}`));
</script>

<template>
  <div class="mask" :class="{ 'see-through': step === 1 }">
    <!-- 窗口右下角的语言选择：选了立刻生效 -->
    <label v-if="settings.settings" class="lang" :title="$t('language.label')">
      <span aria-hidden="true">🌐</span>
      <select class="input" :value="settings.settings.language" :aria-label="$t('language.label')" @change="patch({ language: ($event.target as HTMLSelectElement).value as Settings['language'] })">
        <option v-for="l in languages" :key="l" :value="l">{{ languageName(l) }}</option>
      </select>
    </label>

    <div class="dialog box" role="dialog" aria-modal="true" :aria-label="$t('onboarding.aria')">
      <div class="box-title">{{ title }} <span class="extra muted">{{ step + 1 }} / 4</span></div>
      <div class="box-body">
        <template v-if="step === 0">
          <p>{{ $t("onboarding.intro") }}</p>
          <p class="muted small">{{ $t("onboarding.introHint") }}</p>
        </template>

        <template v-else-if="step === 1">
          <div v-if="settings.settings" class="pick">
            <p class="muted small live">{{ $t("onboarding.liveHint") }}</p>
            <div class="label muted">{{ $t("appearance.layout") }}</div>
            <div class="cards">
              <button v-for="l in layouts" :key="l" class="card" :class="{ active: settings.settings.layout === l }" @click="patch({ layout: l })">
                <b>{{ $t(`appearance.layouts.${l}.name`) }}</b><span class="muted small">{{ $t(`appearance.layouts.${l}.desc`) }}</span>
              </button>
            </div>
            <div class="label muted">{{ $t("appearance.palette") }}</div>
            <div class="swatches">
              <button v-for="p in palettes" :key="p.key" class="swatch" :class="{ active: settings.settings.palette === p.key }" :title="$t(`appearance.palettes.${p.key}`)" @click="patch({ palette: p.key })">
                <i :style="{ background: p.color }" />{{ $t(`appearance.palettes.${p.key}`) }}
              </button>
            </div>
            <div class="label muted">{{ $t("appearance.mode") }}</div>
            <div class="swatches">
              <button v-for="m in modes" :key="m" class="swatch" :class="{ active: settings.settings.theme === m }" @click="patch({ theme: m })">{{ $t(`appearance.modes.${m}`) }}</button>
            </div>
          </div>
        </template>

        <template v-else-if="step === 2">
          <div class="tabs">
            <a v-for="tab in tabs" :key="tab.key" :class="{ active: mode === tab.key }" @click="!busy && (mode = tab.key)">{{ tab.label }}</a>
          </div>
          <div class="acc-body">
            <template v-if="mode === 'microsoft'">
              <p class="small">{{ $t("onboarding.msHint") }}</p>
              <p v-if="isStore" class="small">{{ $t("onboarding.storeHint") }}</p>
              <template v-if="device">
                <p>{{ $t("onboarding.code") }}<b class="code">{{ device.userCode }}</b></p>
                <button class="btn" @click="copyAndOpen">{{ copied ? $t("onboarding.copied") : $t("onboarding.copyAndOpen") }}</button>
                <p class="muted small">{{ $t("onboarding.waiting") }}</p>
              </template>
              <button v-else class="btn primary" :disabled="busy" @click="startMicrosoft">{{ $t("onboarding.msStart") }}</button>
            </template>
            <template v-else-if="mode === 'offline'">
              <p class="small">{{ $t("onboarding.offlineHint") }}</p>
              <div class="form-row"><input v-model="offlineName" class="input" :placeholder="$t('onboarding.offlinePlaceholder')" :disabled="busy" @keyup.enter="addOffline" /><button class="btn primary" :disabled="busy || !offlineName.trim()" @click="addOffline">{{ $t("common.add") }}</button></div>
            </template>
            <template v-else>
              <p class="small">{{ $t("onboarding.yggHint") }}</p>
              <input v-model="ygg.server" class="input" :placeholder="$t('onboarding.yggServer')" :disabled="busy" />
              <input v-model="ygg.username" class="input" :placeholder="$t('account.yggUser')" :disabled="busy" />
              <input v-model="ygg.password" class="input" type="password" :placeholder="$t('account.password')" :disabled="busy" />
              <button class="btn primary" :disabled="busy || !ygg.server.trim() || !ygg.username.trim() || !ygg.password" @click="addYgg">{{ $t("shell.login") }}</button>
            </template>
            <p v-if="message" :class="message.ok ? 'ok-text' : 'error-text'" class="small break">{{ message.text }}</p>
          </div>
        </template>

        <template v-else>
          <p v-if="addedName" class="ok-text">{{ $t("onboarding.loggedIn", { name: addedName }) }}</p>
          <p v-else class="muted">{{ $t("onboarding.noAccount") }}</p>
          <p>{{ $t("onboarding.next") }}</p>
        </template>
      </div>
      <div class="actions">
        <a v-if="step < 3" class="skip" @click="finish">{{ $t("onboarding.skipAll") }}</a>
        <span class="grow" />
        <button v-if="step > 0 && step < 3" class="btn" :disabled="busy" @click="step--">{{ $t("onboarding.prev") }}</button>
        <button v-if="step === 0" class="btn primary" @click="step = 1">{{ $t("onboarding.start") }}</button>
        <button v-else-if="step === 1" class="btn primary" @click="step = 2">{{ $t("onboarding.nextStep") }}</button>
        <button v-else-if="step === 2" class="btn" :disabled="busy" @click="step = 3">{{ $t("onboarding.skipStep") }}</button>
        <button v-else class="btn primary" @click="finish">{{ $t("onboarding.done") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.5); display: flex; align-items: center; justify-content: center; z-index: 40; transition: background 0.2s; }
/* 选风格时遮罩调淡，背后的整个窗口就是实时预览 */
.mask.see-through { background: rgba(0, 0, 0, 0.12); backdrop-filter: none; }
.lang { position: fixed; right: 14px; bottom: 12px; display: flex; align-items: center; gap: 6px; padding: 4px 6px 4px 10px; border-radius: var(--radius-sm); background: var(--bg-box-solid); border: 1px solid var(--border); box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2); font-size: 12px; }
.lang .input { padding: 2px 4px; font-size: 12px; }
.live { margin: 0 0 4px; }
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
