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
    error.value = String(e);
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
    error.value = String(e);
  }
}
onUnmounted(flush);

// ---------- 启动按钮 ----------
const styles: { key: LaunchStyle; label: string }[] = [
  { key: "stone", label: "石头" },
  { key: "flat", label: "扁平" },
  { key: "pill", label: "胶囊" },
];
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
const actionTypes: { key: ActionType; label: string }[] = [
  { key: "openFolder", label: "打开文件夹" },
  { key: "openUrl", label: "打开网址" },
  { key: "launch", label: "启动游戏" },
  { key: "runProgram", label: "运行程序" },
];
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
    formError.value = problem;
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
    formError.value = "浏览器预览模式不能选择文件";
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
      启动按钮
      <button class="btn small extra-btn" :class="{ danger: confirmReset }" @click="resetAll">{{ confirmReset ? "再点一次确认" : "↺ 恢复按钮与导航默认" }}</button>
    </div>
    <div class="box-body">
      <div class="row">
        <label>预览</label>
        <button class="btn primary preview" :class="launchClass" :style="launchStyle" type="button">▶ {{ launchLabel }}</button>
      </div>
      <div class="row">
        <label>文字</label>
        <input class="input" maxlength="12" :value="buttons.launchLabel" placeholder="开始游戏" @change="save({ launchLabel: ($event.target as HTMLInputElement).value.trim() })" />
        <span class="muted small">最多 12 个字，留空用默认的「开始游戏」</span>
      </div>
      <div class="row">
        <label>样式</label>
        <div class="seg">
          <a v-for="s in styles" :key="s.key" :class="{ active: buttons.launchStyle === s.key }" @click="save({ launchStyle: s.key })">{{ s.label }}</a>
        </div>
      </div>
      <div class="row">
        <label>颜色</label>
        <input type="color" class="color" :value="buttons.launchColor || '#5e9c35'" @input="live({ launchColor: ($event.target as HTMLInputElement).value })" />
        <span v-if="buttons.launchColor" class="mono">{{ buttons.launchColor }}</span>
        <a v-if="buttons.launchColor" @click="save({ launchColor: '' })">跟随配色方案</a>
        <span v-else class="muted small">现在跟随当前配色方案的强调色</span>
      </div>
      <div class="row">
        <label>其他</label>
        <label class="check"><input type="checkbox" :checked="buttons.launchSound" @change="save({ launchSound: ($event.target as HTMLInputElement).checked })" /> 点击启动时发出提示音</label>
        <a class="small" @click="playLaunchSound()">试听</a>
        <label class="check"><input type="checkbox" :checked="buttons.launchShortcut" @change="save({ launchShortcut: ($event.target as HTMLInputElement).checked })" /> 用 {{ isMac ? "⌘" : "Ctrl" }}+Enter 直接启动</label>
      </div>
    </div>
  </div>

  <div class="box">
    <div class="box-title">导航栏 <span class="extra muted">调整顺序、隐藏用不到的页面</span></div>
    <div class="box-body">
      <div v-for="(path, i) in order" :key="path" class="nav-row" :class="{ off: isHidden(path) }">
        <span class="nav-name">{{ navName(path) }}</span>
        <label class="check" :title="ALWAYS_VISIBLE.includes(path) ? '这个页面不能隐藏，否则就进不去设置了' : undefined">
          <input type="checkbox" :checked="!isHidden(path)" :disabled="ALWAYS_VISIBLE.includes(path)" @change="toggleNav(path, ($event.target as HTMLInputElement).checked)" />
          显示
        </label>
        <button class="btn small" :disabled="i === 0" title="上移" @click="save({ navOrder: moveItem(order, i, -1) })">↑</button>
        <button class="btn small" :disabled="i === order.length - 1" title="下移" @click="save({ navOrder: moveItem(order, i, 1) })">↓</button>
      </div>
      <p class="muted small hint">首页、主题中心、设置不能隐藏。想恢复默认顺序，用上面的「恢复按钮与导航默认」。</p>
    </div>
  </div>

  <div class="box">
    <div class="box-title">首页快捷按钮 <span class="extra muted">最多 {{ MAX_QUICK }} 个，显示在首页启动区域下面</span></div>
    <div class="box-body">
      <div v-if="!buttons.quick.length && editing !== 'new'" class="muted small">还没有快捷按钮。可以加一个"打开存档文件夹""启动并直接进服务器"之类的。</div>
      <div v-for="(b, i) in buttons.quick" :key="b.id" class="quick-row">
        <div class="quick-main">
          <b><span v-if="b.icon">{{ b.icon }} </span>{{ b.label }}</b>
          <span class="muted small">{{ describeQuick(b.action) }}</span>
        </div>
        <button class="btn small" :disabled="i === 0" @click="moveQuick(i, -1)">↑</button>
        <button class="btn small" :disabled="i === buttons.quick.length - 1" @click="moveQuick(i, 1)">↓</button>
        <button class="btn small" @click="startEdit(b)">编辑</button>
        <button class="btn small" @click="removeQuick(b.id)">删除</button>
      </div>

      <div v-if="editing" class="editor">
        <div class="row">
          <label>名称</label>
          <input v-model="form.label" class="input" maxlength="16" placeholder="例如：存档" />
          <input v-model="form.icon" class="input icon" maxlength="4" placeholder="图标" title="一个 emoji 或一两个字，可以留空" />
        </div>
        <div class="row">
          <label>动作</label>
          <select v-model="form.type" class="input">
            <option v-for="t in actionTypes" :key="t.key" :value="t.key">{{ t.label }}</option>
          </select>
        </div>
        <div v-if="form.type === 'openFolder' || form.type === 'runProgram'" class="row">
          <label>{{ form.type === "openFolder" ? "文件夹" : "程序" }}</label>
          <input v-model="form.path" class="input wide" :placeholder="form.type === 'openFolder' ? '要打开的文件夹路径' : '要运行的程序或脚本的完整路径'" />
          <button class="btn small" type="button" @click="pick(form.type === 'openFolder')">选择…</button>
        </div>
        <p v-if="form.type === 'runProgram'" class="mod-tip warn">【版主提示】运行程序等于在文件管理器里双击它。每次点这个按钮之前都会弹窗让你确认完整路径；只添加你自己信得过的程序。</p>
        <div v-if="form.type === 'openUrl'" class="row">
          <label>网址</label>
          <input v-model="form.url" class="input wide" placeholder="https://www.mcbbs.co/" />
        </div>
        <template v-if="form.type === 'launch'">
          <div class="row">
            <label>版本</label>
            <select v-model="form.versionId" class="input wide">
              <option v-if="!versions.installed.length" value="">还没有安装任何版本</option>
              <option v-for="v in versions.installed" :key="v.id" :value="v.id">{{ v.id }}</option>
            </select>
          </div>
          <div class="row">
            <label>服务器</label>
            <input v-model="form.server" class="input wide" placeholder="可选，例如 mc.example.com:25565，启动后直接进入" />
          </div>
        </template>
        <p v-if="formError" class="error-text small">{{ formError }}</p>
        <div class="row">
          <label></label>
          <button class="btn primary" @click="saveForm">{{ editing === "new" ? "添加" : "保存" }}</button>
          <button class="btn" @click="editing = null">取消</button>
        </div>
      </div>
      <div v-else class="row add">
        <button class="btn" :disabled="buttons.quick.length >= MAX_QUICK" @click="startAdd">＋ 添加快捷按钮</button>
        <span v-if="buttons.quick.length >= MAX_QUICK" class="muted small">已经到上限了</span>
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
