<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { inTauri } from "../api";
import type { Buttons, LaunchStyle, QuickAction, QuickButton } from "../api/types";
import { DEFAULT_BUTTONS, playLaunchSound, useButtons } from "../buttons";
import { ALWAYS_VISIBLE, MAX_QUICK, describeQuick, fullOrder, moveItem, newQuickId, validateQuick } from "../buttons-logic";
import { navItems } from "../router";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";
import { errText, t } from "../i18n";

// 主题中心里的「按钮与导航」：启动按钮外观、导航栏顺序、首页快捷按钮。
// 这些设置不会进 .godtheme 主题文件：快捷按钮能运行本地程序，不能随别人分享的主题带进来。
const store = useSettingsStore();
const versions = useVersionsStore();
const { buttons, launchLabel, launchClass, launchStyle } = useButtons();
const error = ref("");

onMounted(() => {
  if (!versions.installed.length) versions.loadInstalled();
});

async function save(patch: Partial<Buttons>) {
  if (!store.settings) return;
  error.value = "";
  try {
    await store.save({ ...store.settings, buttons: { ...buttons.value, ...patch } });
  } catch (e) {
    error.value = errText(e);
  }
}

// 选颜色会连续触发：界面立刻变，停手 400ms 再写盘
let timer: ReturnType<typeof setTimeout> | undefined;
function live(patch: Partial<Buttons>) {
  if (!store.settings) return;
  store.settings = { ...store.settings, buttons: { ...buttons.value, ...patch } };
  clearTimeout(timer);
  timer = setTimeout(flush, 400);
}
async function flush() {
  if (timer === undefined || !store.settings) return;
  clearTimeout(timer);
  timer = undefined;
  try {
    await store.save(store.settings);
  } catch (e) {
    error.value = errText(e);
  }
}
onUnmounted(flush);

// ---------- 启动按钮 ----------
const styles: LaunchStyle[] = ["stone", "flat", "pill"];
const isMac = /Mac/i.test(navigator.userAgent);

// ---------- 导航栏 ----------
const defaultPaths = navItems.map((n) => n.path);
const order = computed(() => fullOrder(defaultPaths, buttons.value.navOrder));
const navName = (path: string) => navItems.find((n) => n.path === path)?.name ?? path;
const isHidden = (path: string) => !ALWAYS_VISIBLE.includes(path) && buttons.value.navHidden.includes(path);
function toggleNav(path: string, visible: boolean) {
  const hidden = new Set(buttons.value.navHidden);
  if (visible) hidden.delete(path);
  else hidden.add(path);
  save({ navHidden: [...hidden] });
}

// ---------- 首页快捷按钮 ----------
type ActionType = QuickAction["type"];
const actionTypes: ActionType[] = ["openFolder", "openUrl", "launch", "runProgram"];
const describe = (a: QuickAction) => {
  const d = describeQuick(a);
  return t(d.key, d.params);
};
const editing = ref<string | null>(null); // 正在编辑的按钮 id；"new" 表示新增
const formError = ref("");
const form = reactive({ label: "", icon: "", type: "openFolder" as ActionType, path: "", url: "", versionId: "", server: "" });

function toAction(): QuickAction {
  switch (form.type) {
    case "openFolder":
      return { type: "openFolder", path: form.path.trim() };
    case "runProgram":
      return { type: "runProgram", path: form.path.trim() };
    case "openUrl":
      return { type: "openUrl", url: form.url.trim() };
    case "launch":
      return { type: "launch", versionId: form.versionId, server: form.server.trim() || null };
  }
}

function startAdd() {
  Object.assign(form, { label: "", icon: "", type: "openFolder", path: "", url: "", versionId: versions.installed[0]?.id ?? "", server: "" });
  formError.value = "";
  editing.value = "new";
}
function startEdit(b: QuickButton) {
  const a = b.action;
  Object.assign(form, {
    label: b.label,
    icon: b.icon,
    type: a.type,
    path: a.type === "openFolder" || a.type === "runProgram" ? a.path : "",
    url: a.type === "openUrl" ? a.url : "",
    versionId: a.type === "launch" ? a.versionId : (versions.installed[0]?.id ?? ""),
    server: a.type === "launch" ? (a.server ?? "") : "",
  });
  formError.value = "";
  editing.value = b.id;
}
async function saveForm() {
  const draft: QuickButton = {
    id: editing.value === "new" ? newQuickId(buttons.value.quick.map((q) => q.id)) : (editing.value as string),
    label: form.label.trim(),
    icon: form.icon.trim(),
    action: toAction(),
  };
  const problem = validateQuick(draft);
  if (problem) {
    formError.value = t(problem);
    return;
  }
  const list = editing.value === "new" ? [...buttons.value.quick, draft] : buttons.value.quick.map((q) => (q.id === draft.id ? draft : q));
  editing.value = null;
  await save({ quick: list });
}
function removeQuick(id: string) {
  save({ quick: buttons.value.quick.filter((q) => q.id !== id) });
}
function moveQuick(i: number, delta: -1 | 1) {
  save({ quick: moveItem(buttons.value.quick, i, delta) });
}

async function pick(directory: boolean) {
  if (!inTauri) {
    formError.value = t("theme.previewNoPick");
    return;
  }
  const chosen = await open({ multiple: false, directory });
  if (typeof chosen === "string") form.path = chosen;
}

// ---------- 恢复默认 ----------
const confirmReset = ref(false);
let resetTimer: ReturnType<typeof setTimeout> | undefined;
async function resetAll() {
  if (!confirmReset.value) {
    confirmReset.value = true;
    resetTimer = setTimeout(() => (confirmReset.value = false), 3000);
    return;
  }
  clearTimeout(resetTimer);
  confirmReset.value = false;
  editing.value = null;
  await save({ ...DEFAULT_BUTTONS, quick: [] });
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      {{ $t("buttons.launchTitle") }}
      <button class="btn small extra-btn" :class="{ danger: confirmReset }" @click="resetAll">{{ confirmReset ? $t("buttons.confirmAgain") : `↺ ${$t("buttons.reset")}` }}</button>
    </div>
    <div class="box-body">
      <div class="row">
        <label>{{ $t("buttons.preview") }}</label>
        <button class="btn primary preview" :class="launchClass" :style="launchStyle" type="button">▶ {{ launchLabel }}</button>
      </div>
      <div class="row">
        <label>{{ $t("buttons.text") }}</label>
        <input class="input" maxlength="12" :value="buttons.launchLabel" :placeholder="$t('launch.defaultLabel')" @change="save({ launchLabel: ($event.target as HTMLInputElement).value.trim() })" />
        <span class="muted small">{{ $t("buttons.textHint") }}</span>
      </div>
      <div class="row">
        <label>{{ $t("buttons.style") }}</label>
        <div class="seg">
          <a v-for="s in styles" :key="s" :class="{ active: buttons.launchStyle === s }" @click="save({ launchStyle: s })">{{ $t(`buttons.styles.${s}`) }}</a>
        </div>
      </div>
      <div class="row">
        <label>{{ $t("buttons.color") }}</label>
        <input type="color" class="color" :value="buttons.launchColor || '#5e9c35'" @input="live({ launchColor: ($event.target as HTMLInputElement).value })" />
        <span v-if="buttons.launchColor" class="mono">{{ buttons.launchColor }}</span>
        <a v-if="buttons.launchColor" @click="save({ launchColor: '' })">{{ $t("buttons.followPalette") }}</a>
        <span v-else class="muted small">{{ $t("buttons.followingPalette") }}</span>
      </div>
      <div class="row">
        <label>{{ $t("buttons.other") }}</label>
        <label class="check"><input type="checkbox" :checked="buttons.launchSound" @change="save({ launchSound: ($event.target as HTMLInputElement).checked })" /> {{ $t("buttons.sound") }}</label>
        <a class="small" @click="playLaunchSound()">{{ $t("buttons.listen") }}</a>
        <label class="check"><input type="checkbox" :checked="buttons.launchShortcut" @change="save({ launchShortcut: ($event.target as HTMLInputElement).checked })" /> {{ $t("buttons.shortcut", { key: isMac ? "⌘" : "Ctrl" }) }}</label>
      </div>
    </div>
  </div>

  <div class="box">
    <div class="box-title">{{ $t("buttons.navTitle") }} <span class="extra muted">{{ $t("buttons.navHint") }}</span></div>
    <div class="box-body">
      <div v-for="(path, i) in order" :key="path" class="nav-row" :class="{ off: isHidden(path) }">
        <span class="nav-name">{{ navName(path) }}</span>
        <label class="check" :title="ALWAYS_VISIBLE.includes(path) ? $t('buttons.cantHide') : undefined">
          <input type="checkbox" :checked="!isHidden(path)" :disabled="ALWAYS_VISIBLE.includes(path)" @change="toggleNav(path, ($event.target as HTMLInputElement).checked)" />
          {{ $t("buttons.show") }}
        </label>
        <button class="btn small" :disabled="i === 0" :title="$t('buttons.up')" @click="save({ navOrder: moveItem(order, i, -1) })">↑</button>
        <button class="btn small" :disabled="i === order.length - 1" :title="$t('buttons.down')" @click="save({ navOrder: moveItem(order, i, 1) })">↓</button>
      </div>
      <p class="muted small hint">{{ $t("buttons.navFoot") }}</p>
    </div>
  </div>

  <div class="box">
    <div class="box-title">{{ $t("buttons.quickTitle") }} <span class="extra muted">{{ $t("buttons.quickHint", { n: MAX_QUICK }) }}</span></div>
    <div class="box-body">
      <div v-if="!buttons.quick.length && editing !== 'new'" class="muted small">{{ $t("buttons.quickEmpty") }}</div>
      <div v-for="(b, i) in buttons.quick" :key="b.id" class="quick-row">
        <div class="quick-main">
          <b><span v-if="b.icon">{{ b.icon }} </span>{{ b.label }}</b>
          <span class="muted small">{{ describe(b.action) }}</span>
        </div>
        <button class="btn small" :disabled="i === 0" @click="moveQuick(i, -1)">↑</button>
        <button class="btn small" :disabled="i === buttons.quick.length - 1" @click="moveQuick(i, 1)">↓</button>
        <button class="btn small" @click="startEdit(b)">{{ $t("buttons.edit") }}</button>
        <button class="btn small" @click="removeQuick(b.id)">{{ $t("common.delete") }}</button>
      </div>

      <div v-if="editing" class="editor">
        <div class="row">
          <label>{{ $t("export.name") }}</label>
          <input v-model="form.label" class="input" maxlength="16" :placeholder="$t('buttons.namePlaceholder')" />
          <input v-model="form.icon" class="input icon" maxlength="4" :placeholder="$t('buttons.icon')" :title="$t('buttons.iconHint')" />
        </div>
        <div class="row">
          <label>{{ $t("buttons.action") }}</label>
          <select v-model="form.type" class="input">
            <option v-for="a in actionTypes" :key="a" :value="a">{{ $t(`buttons.actions.${a}`) }}</option>
          </select>
        </div>
        <div v-if="form.type === 'openFolder' || form.type === 'runProgram'" class="row">
          <label>{{ form.type === "openFolder" ? $t("buttons.folder") : $t("buttons.program") }}</label>
          <input v-model="form.path" class="input wide" :placeholder="form.type === 'openFolder' ? $t('buttons.folderPlaceholder') : $t('buttons.programPlaceholder')" />
          <button class="btn small" type="button" @click="pick(form.type === 'openFolder')">{{ $t("common.browse") }}</button>
        </div>
        <p v-if="form.type === 'runProgram'" class="mod-tip warn">{{ $t("common.modTip") }}{{ $t("buttons.programWarn") }}</p>
        <div v-if="form.type === 'openUrl'" class="row">
          <label>{{ $t("buttons.url") }}</label>
          <input v-model="form.url" class="input wide" placeholder="https://www.mcbbs.co/" />
        </div>
        <template v-if="form.type === 'launch'">
          <div class="row">
            <label>{{ $t("launchHome.version") }}</label>
            <select v-model="form.versionId" class="input wide">
              <option v-if="!versions.installed.length" value="">{{ $t("forumHome.noVersions") }}</option>
              <option v-for="v in versions.installed" :key="v.id" :value="v.id">{{ v.id }}</option>
            </select>
          </div>
          <div class="row">
            <label>{{ $t("buttons.server") }}</label>
            <input v-model="form.server" class="input wide" :placeholder="$t('buttons.serverPlaceholder')" />
          </div>
        </template>
        <p v-if="formError" class="error-text small">{{ formError }}</p>
        <div class="row">
          <label></label>
          <button class="btn primary" @click="saveForm">{{ editing === "new" ? $t("common.add") : $t("common.save") }}</button>
          <button class="btn" @click="editing = null">{{ $t("common.cancel") }}</button>
        </div>
      </div>
      <div v-else class="row add">
        <button class="btn" :disabled="buttons.quick.length >= MAX_QUICK" @click="startAdd">＋ {{ $t("buttons.addQuick") }}</button>
        <span v-if="buttons.quick.length >= MAX_QUICK" class="muted small">{{ $t("buttons.limit") }}</span>
      </div>
    </div>
  </div>
  <p v-if="error" class="error-text">{{ error }}</p>
</template>

<style scoped>
.row { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); flex-wrap: wrap; }
.row:last-child { border-bottom: none; }
.row > label:first-child { width: 60px; flex-shrink: 0; text-align: right; color: var(--text-muted); }
.small { font-size: 12px; }
.mono { font-family: var(--font-pixel); font-size: 12px; }
.color { width: 44px; height: 30px; padding: 0; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: none; cursor: pointer; }
.check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.preview { min-width: 150px; padding: 8px 22px; font-size: 14px; font-weight: bold; }
.extra-btn { margin-left: auto; font-weight: normal; }
.btn.danger { color: #fff; background: var(--red); border-color: var(--red); }
.seg { display: inline-flex; border: 1px solid var(--border-strong); border-radius: 6px; overflow: hidden; }
.seg a { padding: 5px 16px; color: var(--text); background: var(--bg-box-alt); border-right: 1px solid var(--border); }
.seg a:last-child { border-right: none; }
.seg a:hover { text-decoration: none; filter: brightness(1.05); }
.seg a.active { background: var(--grass); color: #fff; font-weight: bold; }

.nav-row { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px dashed var(--border); }
.nav-row.off .nav-name { color: var(--text-muted); text-decoration: line-through; }
.nav-name { flex: 1; font-weight: bold; }
.hint { margin: 8px 0 0; }

.quick-row { display: flex; align-items: center; gap: 8px; padding: 8px 0; border-bottom: 1px dashed var(--border); }
.quick-main { display: flex; flex-direction: column; flex: 1; min-width: 0; gap: 2px; }
.quick-main .muted { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.editor { margin-top: 10px; padding: 10px 12px; border-radius: var(--radius); background: var(--bg-box-alt); border: 1px dashed var(--border-strong); }
.input.icon { width: 70px; }
.input.wide { flex: 1; min-width: 260px; }
.add { border-bottom: none; padding-bottom: 0; }
.warn { margin: 6px 0; font-size: 12px; }
</style>
