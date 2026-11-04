<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { api, inTauri } from "../api";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import type { JavaInstall, MemoryInfo, ProbeResult, RulesInfo, Settings } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";
import { checkForUpdate } from "../update";
import GameDirDialog from "../components/GameDirDialog.vue";
import type { GameDirInfo } from "../api/types";
import { announcementsLoading, announcementsNote, loadAnnouncements, restoreAnnouncements } from "../announcements";
import { isStore } from "../edition";
import { LOCALES, LOCALE_NAMES, errText, t } from "../i18n";

const store = useSettingsStore();
const versions = useVersionsStore();
const form = ref<Settings | null>(null);
const message = ref<{ ok: boolean; text: string } | null>(null);
const probe = ref<ProbeResult[] | null>(null);
const probing = ref(false);

const javas = ref<JavaInstall[]>([]);
const scanning = ref(false);
const memory = ref<MemoryInfo | null>(null);

watch(() => store.settings, (s) => (form.value = s ? { ...s } : null), { immediate: true });

const dirInfo = ref<GameDirInfo | null>(null);
const dirDialog = ref(false);
const dirInitial = ref<string | undefined>();
const dirFromLegacy = ref(false);
async function showDirDialog(fromLegacy = false) {
  dirInitial.value = undefined;
  dirFromLegacy.value = fromLegacy === true;
  dirInfo.value = await api.gameDirInfo();
  dirDialog.value = true;
}
async function pickGameDir() {
  if (!inTauri) {
    message.value = { ok: false, text: t("gameDir.previewOnly") };
    return;
  }
  const chosen = await open({ multiple: false, directory: true });
  if (typeof chosen !== "string") return;
  dirInfo.value = await api.gameDirInfo();
  dirInitial.value = chosen;
  dirFromLegacy.value = false;
  dirDialog.value = true;
}
async function onDirClosed() {
  dirDialog.value = false;
  const i = await api.gameDirInfo().catch(() => null);
  if (i) suggestMove.value = i.legacyHasData ? "fromLegacy" : i.currentIsLegacy && i.current !== i.recommended ? "toRecommended" : null;
}
async function openGameDir() {
  if (form.value) await api.quickOpenFolder(form.value.gameDir).catch((e) => (message.value = { ok: false, text: errText(e) }));
}
// 老版本默认放在 AppData 里：提醒一下可以搬到启动器旁边
const suggestMove = ref<"toRecommended" | "fromLegacy" | null>(null);

const rules = ref<RulesInfo | null>(null);
const rulesBusy = ref(false);
const rulesMsg = ref<{ ok: boolean; text: string } | null>(null);

onMounted(async () => {
  api.gameDirInfo().then((i) => (suggestMove.value = i.legacyHasData ? "fromLegacy" : i.currentIsLegacy && i.current !== i.recommended ? "toRecommended" : null)).catch(() => {});
  memory.value = await api.memoryInfo();
  rules.value = await api.crashRulesInfo();
  await scanJava(false);
});

async function updateRules() {
  if (!form.value?.crashRulesUrl) return;
  rulesBusy.value = true;
  rulesMsg.value = null;
  try {
    rules.value = await api.updateCrashRules(form.value.crashRulesUrl);
    rulesMsg.value = { ok: true, text: t("settings.rulesUpdated", { n: rules.value.count }) };
  } catch (e) {
    rulesMsg.value = { ok: false, text: errText(e) };
  } finally {
    rulesBusy.value = false;
  }
}

async function resetRules() {
  rules.value = await api.resetCrashRules();
  rulesMsg.value = { ok: true, text: t("settings.rulesReset") };
}

async function scanJava(force: boolean) {
  scanning.value = true;
  try {
    javas.value = await api.listJava(force);
  } finally {
    scanning.value = false;
  }
}

/** 空字符串 / 0 / 非数字都视为"自动" */
function optNumber(v: unknown): number | null {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? Math.round(n) : null;
}

async function save() {
  if (!form.value) return;
  try {
    await store.save({
      ...form.value,
      concurrency: Math.min(256, Math.max(1, Number(form.value.concurrency) || 8)),
      javaPath: form.value.javaPath || null,
      memoryMb: optNumber(form.value.memoryMb),
      windowWidth: optNumber(form.value.windowWidth),
      windowHeight: optNumber(form.value.windowHeight),
    });
    message.value = { ok: true, text: t("settings.saved") };
    versions.loadInstalled();
  } catch (e) {
    message.value = { ok: false, text: errText(e) };
  }
}

async function runProbe() {
  probing.value = true;
  probe.value = null;
  try {
    probe.value = await api.probeMirrors();
  } catch (e) {
    message.value = { ok: false, text: errText(e) };
  } finally {
    probing.value = false;
  }
}

// 界面语言：改了立刻生效、立刻保存，和下面那张表单的「保存」无关
const languages: Settings["language"][] = ["auto", ...LOCALES];
const langMsg = ref("");
async function setLanguage(language: Settings["language"]) {
  if (!store.settings) return;
  langMsg.value = "";
  try {
    await store.save({ ...store.settings, language });
  } catch (e) {
    langMsg.value = errText(e);
  }
}

// 检查更新：手动查，查到新版本会弹窗；自动检查的开关改了立刻保存
const checking = ref(false);
const updateMsg = ref<{ ok: boolean; text: string } | null>(null);
async function manualCheck() {
  checking.value = true;
  updateMsg.value = null;
  try {
    const info = await checkForUpdate(true);
    if (!info?.latest) updateMsg.value = { ok: true, text: t("settings.noRelease") };
    else if (info.hasUpdate) updateMsg.value = { ok: true, text: t("settings.hasUpdate", { v: info.latest }) };
    else updateMsg.value = { ok: true, text: t("settings.upToDate", { v: info.current }) };
  } catch (e) {
    updateMsg.value = { ok: false, text: errText(e) };
  } finally {
    checking.value = false;
  }
}
async function setAnnouncements(on: boolean) {
  if (!store.settings) return;
  try {
    await store.save({ ...store.settings, announcementsEnabled: on });
    if (on) loadAnnouncements();
  } catch (e) {
    updateMsg.value = { ok: false, text: errText(e) };
  }
}
async function setAutoCheck(on: boolean) {
  if (!store.settings) return;
  try {
    await store.save({ ...store.settings, autoCheckUpdate: on });
  } catch (e) {
    updateMsg.value = { ok: false, text: errText(e) };
  }
}

// 版本号读打包时写进程序的那个，不在这里写死
const appVersion = ref("");
if (inTauri) getVersion().then((v) => (appVersion.value = v));
</script>

<template>
  <div class="box">
    <div class="box-title">{{ $t("settings.appearance") }}</div>
    <div class="box-body">
      <div class="row">
        <label>{{ $t("language.label") }}</label>
        <div class="field">
          <select class="input" :value="store.settings?.language ?? 'auto'" @change="setLanguage(($event.target as HTMLSelectElement).value as Settings['language'])">
            <option v-for="l in languages" :key="l" :value="l">{{ l === "auto" ? $t("language.auto") : LOCALE_NAMES[l] }}</option>
          </select>
          <span class="muted hint">{{ $t("settings.languageHint") }}</span>
          <span v-if="langMsg" class="error-text">{{ langMsg }}</span>
        </div>
      </div>
      <i18n-t keypath="settings.appearanceHint" tag="p" class="appearance-hint"><RouterLink to="/theme">{{ $t("nav.theme") }}</RouterLink></i18n-t>
    </div>
  </div>

  <div class="box">
    <div class="box-title">{{ $t("nav.settings") }}</div>
    <div v-if="form" class="box-body">
      <div class="row">
        <label>{{ $t("settings.gameDir") }}</label>
        <div class="field">
          <div class="dirline">
            <code class="dirpath">{{ form.gameDir }}</code>
            <button class="btn small" @click="pickGameDir">{{ $t("settings.pickFolder") }}</button>
            <button class="btn small" @click="showDirDialog()">{{ $t("settings.changeDir") }}</button>
            <button class="btn small" @click="openGameDir">{{ $t("launchHome.openFolder") }}</button>
          </div>
          <div class="muted hint">{{ $t("settings.gameDirHint") }}</div>
          <div v-if="suggestMove === 'toRecommended'" class="hint"><a @click="showDirDialog()">{{ $t("settings.suggestRecommended") }} »</a></div>
          <div v-else-if="suggestMove === 'fromLegacy'" class="hint"><a @click="showDirDialog(true)">{{ $t("settings.suggestLegacy") }} »</a></div>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.source") }}</label>
        <div class="field">
          <select v-model="form.downloadSource" class="input">
            <option value="auto">{{ $t("settings.sourceAuto") }}</option>
            <option value="official">{{ $t("settings.sourceOfficial") }}</option>
            <option value="bmclapi">{{ $t("settings.sourceBmclapi") }}</option>
          </select>
          <button class="btn" :disabled="probing" @click="runProbe">{{ probing ? $t("settings.probing") : $t("settings.probe") }}</button>
          <span v-if="probe" class="probe">
            <span v-for="p in probe" :key="p.source">
              {{ p.source === "official" ? $t("settings.sourceOfficial") : "BMCLAPI" }}: <b>{{ p.millis === null ? $t("settings.timeout") : p.millis + " ms" }}</b>
            </span>
          </span>
          <div class="muted hint">{{ $t("settings.sourceHint") }}</div>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.concurrency") }}</label>
        <div class="field">
          <input v-model.number="form.concurrency" type="number" min="1" max="256" class="input narrow" />
          <span class="muted hint">{{ $t("settings.concurrencyHint") }}</span>
        </div>
      </div>
      <div class="row">
        <label>CurseForge Key</label>
        <div class="field">
          <input v-model="form.curseforgeApiKey" type="password" class="input wide mono" :placeholder="$t('settings.cfPlaceholder')" autocomplete="off" />
          <div class="muted hint">
            {{ $t("settings.cfHint") }}
          </div>
        </div>
      </div>

      <h3 class="section">{{ $t("settings.gameJava") }} <span class="muted">{{ $t("settings.gameJavaTag") }}</span></h3>
      <div class="row">
        <label>Java</label>
        <div class="field">
          <select v-model="form.javaPath" class="input wide">
            <option :value="null">{{ $t("settings.javaAuto") }}</option>
            <option v-for="j in javas" :key="j.path" :value="j.path">
              Java {{ j.version }} {{ j.arch }} {{ j.vendor ?? "" }} — {{ j.path }}
            </option>
            <option v-if="form.javaPath && !javas.some((j) => j.path === form!.javaPath)" :value="form.javaPath">
              {{ form.javaPath }}
            </option>
          </select>
          <button class="btn" :disabled="scanning" @click="scanJava(true)">{{ scanning ? $t("settings.scanning") : $t("settings.rescan") }}</button>
          <div class="muted hint">{{ $t("settings.javaFound", { n: javas.length }) }}</div>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.memory") }}</label>
        <div class="field">
          <input v-model="form.memoryMb" type="number" min="512" step="256" class="input narrow" :placeholder="$t('settings.auto')" />
          <span class="muted">MB</span>
          <span v-if="memory" class="muted hint">
            {{ $t("settings.memoryHint", { total: (memory.totalMb / 1024).toFixed(1), auto: memory.autoMb }) }}
          </span>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.windowSize") }}</label>
        <div class="field">
          <input v-model="form.windowWidth" type="number" class="input narrow" :placeholder="$t('settings.default')" />
          ×
          <input v-model="form.windowHeight" type="number" class="input narrow" :placeholder="$t('settings.default')" />
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.isolate") }}</label>
        <div class="field">
          <label class="check"><input v-model="form.isolateVersions" type="checkbox" /> {{ $t("settings.isolateHint") }}</label>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.jvmArgs") }}</label>
        <div class="field">
          <input v-model="form.jvmArgs" class="input wide mono" :placeholder="$t('settings.example', { v: '-XX:+UseZGC -Dsome.prop=1' })" />
        </div>
      </div>
      <div class="row">
        <label>{{ $t("settings.gameArgs") }}</label>
        <div class="field">
          <input v-model="form.gameArgs" class="input wide mono" :placeholder="$t('settings.example', { v: '--fullscreen' })" />
        </div>
      </div>

      <div class="row">
        <label></label>
        <div class="field">
          <button class="btn primary" @click="save">{{ $t("common.save") }}</button>
          <span v-if="message" :class="message.ok ? 'ok' : 'error-text'">{{ message.text }}</span>
        </div>
      </div>
    </div>
  </div>

  <div v-if="form" class="box">
    <div class="box-title">{{ $t("settings.rules") }}</div>
    <div class="box-body">
      <p v-if="rules" class="rules-info">
        <i18n-t keypath="settings.rulesInfo"><b>{{ rules.count }}</b><span>{{ rules.version }}</span><span>{{ rules.custom ? $t("settings.rulesCustom") : $t("settings.rulesBuiltin") }}</span></i18n-t>
      </p>
      <div class="row">
        <label>{{ $t("settings.rulesUrl") }}</label>
        <div class="field">
          <input v-model="form.crashRulesUrl" class="input wide mono" :placeholder="$t('settings.rulesPlaceholder')" />
          <button class="btn" :disabled="!form.crashRulesUrl || rulesBusy" @click="updateRules">{{ rulesBusy ? $t("update.updating") : $t("update.now") }}</button>
          <button v-if="rules?.custom" class="btn" :disabled="rulesBusy" @click="resetRules">{{ $t("settings.rulesRestore") }}</button>
          <div class="muted hint">{{ $t("settings.rulesHint") }}</div>
        </div>
      </div>
      <p v-if="rulesMsg" :class="rulesMsg.ok ? 'ok' : 'error-text'">{{ rulesMsg.text }}</p>
    </div>
  </div>

  <div class="box">
    <div class="box-title">{{ $t("settings.about") }}</div>
    <div class="box-body about">
      <p><b>{{ $t("app.name") }}</b> {{ appVersion ? `v${appVersion}` : $t("settings.browserPreview") }}<template v-if="isStore">{{ $t("settings.storeEdition") }}</template> {{ $t("settings.loveLetter") }}</p>
      <i18n-t keypath="settings.unofficial" tag="p"><b>{{ $t("settings.unofficialBold") }}</b></i18n-t>
      <p>{{ $t("settings.noDistribution") }}</p>
      <p v-if="!isStore">{{ $t("settings.thirdParty") }}</p>
      <p v-if="isStore" class="muted">{{ $t("settings.storeUpdates") }}</p>
      <div v-else class="update-row">
        <button class="btn small" :disabled="checking" @click="manualCheck">{{ checking ? $t("settings.checking") : $t("settings.checkUpdate") }}</button>
        <label class="check">
          <input type="checkbox" :checked="store.settings?.autoCheckUpdate !== false" @change="setAutoCheck(($event.target as HTMLInputElement).checked)" />
          {{ $t("settings.autoCheck") }}
        </label>
        <span v-if="updateMsg" :class="updateMsg.ok ? 'ok' : 'error-text'">{{ updateMsg.text }}</span>
      </div>
      <div class="update-row">
        <button class="btn small" :disabled="announcementsLoading" @click="loadAnnouncements(true)">{{ announcementsLoading ? $t("settings.refreshing") : $t("settings.refreshNotice") }}</button>
        <label class="check">
          <input type="checkbox" :checked="store.settings?.announcementsEnabled !== false" @change="setAnnouncements(($event.target as HTMLInputElement).checked)" />
          {{ $t("settings.fetchNotice") }}
        </label>
        <a v-if="store.settings?.dismissedAnnouncements?.length" @click="restoreAnnouncements()">{{ $t("settings.restoreNotice") }}</a>
        <span v-if="announcementsNote" :class="announcementsNote.ok ? 'ok' : 'error-text'">{{ announcementsNote.text }}</span>
      </div>
      <p v-if="isStore" class="muted hint">{{ $t("settings.privacyStore") }}</p>
      <p v-else class="muted hint">{{ $t("settings.privacy") }}</p>
    </div>
  </div>
  <GameDirDialog v-if="dirDialog && dirInfo" :info="dirInfo" :initial="dirInitial" :from-legacy="dirFromLegacy" @close="onDirClosed" />
</template>

<style scoped>
.row { display: flex; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); }
.row:last-child { border-bottom: none; }
.appearance-hint { margin: 8px 0 0; }
.row label { width: 110px; flex-shrink: 0; text-align: right; padding-top: 6px; color: var(--text-muted); }
.field { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; flex: 1; }
.wide { flex: 1 1 300px; min-width: 0; }
.narrow { width: 90px; }
.hint { width: 100%; font-size: 12px; }
.dirline { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.dirpath { word-break: break-all; padding: 2px 6px; background: var(--bg-box-alt); border: 1px solid var(--border); border-radius: var(--radius-sm); }
.probe { display: flex; gap: 12px; }
.ok { color: var(--grass); }
.about p { margin: 6px 0; line-height: 1.7; }
.section { font-size: 14px; margin: 14px 0 4px; padding-top: 10px; border-top: 1px solid var(--border); color: var(--brown-dark); }
.section .muted { font-size: 12px; font-weight: normal; }
.check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.mono { font-family: var(--font-pixel); }
.update-row { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; margin: 10px 0 4px; }
.rules-info { margin: 0 0 6px; }
.field label.check { width: auto; flex-shrink: 1; text-align: left; padding-top: 0; color: var(--text); }
</style>
