<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { api, inTauri } from "../api";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { CustomPalette, Settings, Tweaks } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { useAccountsStore } from "../stores/accounts";
import { backgroundUrl, currentLayout, DEFAULT_TWEAKS, pendingThemeImport, type Layout } from "../appearance";
import { contrast } from "../colors";
import ButtonSettings from "../components/ButtonSettings.vue";
import { errText, t } from "../i18n";

// 主题中心：布局、配色（含自定义）、预设、外观细调、明暗、背景图、主题分享。改动立刻生效
const store = useSettingsStore();
const accounts = useAccountsStore();
const message = ref<{ ok: boolean; text: string } | null>(null);

const layouts: Layout[] = ["classic", "sidebar", "modern"];

// 内置配色的基础色，也是"以它为起点自定义"时复制过去的值
// 名字在语言包 appearance.palettes.<key>
const palettes: { key: string; colors: CustomPalette }[] = [
  { key: "mcbbs", colors: { accent: "#5e9c35", surface: "#fbf2db", text: "#3d2f1f", wood: "#6b4a2b" } },
  { key: "grass", colors: { accent: "#4fa52f", surface: "#f2f9ea", text: "#24331a", wood: "#3f6e22" } },
  { key: "nether", colors: { accent: "#d4502a", surface: "#fceee8", text: "#3a1a12", wood: "#5a1c14" } },
  { key: "end", colors: { accent: "#8e5fc0", surface: "#f6f1e0", text: "#2b2238", wood: "#3a2a52" } },
  { key: "ocean", colors: { accent: "#23a093", surface: "#eaf6f8", text: "#15303a", wood: "#1f4f6e" } },
  { key: "cherry", colors: { accent: "#df6a9c", surface: "#fdf0f4", text: "#3d2030", wood: "#7a3a52" } },
  { key: "stone", colors: { accent: "#5b7083", surface: "#f1f2f3", text: "#23282d", wood: "#3d4650" } },
];

interface Preset {
  /** 名字和说明在语言包 theme.presets.<key> */
  key: string;
  patch: Partial<Settings>;
}
// 预设主题：一键套用的配色 + 细调组合
const presets: Preset[] = [
  { key: "redstone", patch: { palette: "custom", customPalette: { accent: "#c0392b", surface: "#f8ece8", text: "#3b1d1a", wood: "#5e1f17" } } },
  { key: "snow", patch: { palette: "custom", customPalette: { accent: "#3d8fc4", surface: "#f5f9fc", text: "#1f2d3a", wood: "#587089" }, tweaks: { ...DEFAULT_TWEAKS, radius: 12 } } },
  { key: "gold", patch: { palette: "custom", customPalette: { accent: "#cf9415", surface: "#fdf6e3", text: "#3f3216", wood: "#7a5a12" } } },
  { key: "lapis", patch: { palette: "custom", customPalette: { accent: "#2f5fb8", surface: "#eef2fb", text: "#1a2440", wood: "#22346b" } } },
  { key: "cave", patch: { palette: "custom", theme: "dark", customPalette: { accent: "#3fbf9c", surface: "#e8ece9", text: "#20262a", wood: "#3b4246" }, tweaks: { ...DEFAULT_TWEAKS, bgDim: 30 } } },
  { key: "oldForum", patch: { layout: "classic", palette: "mcbbs", tweaks: { ...DEFAULT_TWEAKS, radius: 0, cardOpacity: 100, cardBlur: 0, animations: false } } },
];

const themes: Settings["theme"][] = ["light", "dark", "system"];

const fonts = ["Microsoft YaHei UI", "DengXian", "Source Han Sans SC", "Noto Sans SC", "LXGW WenKai", "HarmonyOS Sans SC", "MiSans", "SimSun"];

const tweaks = computed<Tweaks>(() => ({ ...DEFAULT_TWEAKS, ...store.settings?.tweaks }));
const custom = computed<CustomPalette>(() => store.settings?.customPalette ?? palettes[0].colors);
const isCustom = computed(() => store.settings?.palette === "custom");

/** 点击类的改动：立刻保存 */
async function patch(p: Partial<Settings>) {
  if (!store.settings) return;
  message.value = null;
  await flush();
  try {
    await store.save({ ...store.settings, ...p });
  } catch (e) {
    message.value = { ok: false, text: errText(e) };
  }
}

// 拖滑块、选颜色会连续触发：本地立刻生效，停手 400ms 后再写盘
let timer: ReturnType<typeof setTimeout> | undefined;
function live(p: Partial<Settings>) {
  if (!store.settings) return;
  store.settings = { ...store.settings, ...p };
  clearTimeout(timer);
  timer = setTimeout(flush, 400);
}
async function flush() {
  if (timer === undefined) return;
  clearTimeout(timer);
  timer = undefined;
  if (!store.settings) return;
  try {
    await store.save(store.settings);
  } catch (e) {
    message.value = { ok: false, text: errText(e) };
  }
}
onUnmounted(flush);

function setTweak<K extends keyof Tweaks>(key: K, value: Tweaks[K]) {
  live({ tweaks: { ...tweaks.value, [key]: value } });
}
function setColor(key: keyof CustomPalette, value: string) {
  if (!/^#[0-9a-f]{6}$/i.test(value)) return;
  live({ customPalette: { ...custom.value, [key]: value.toLowerCase() } });
}
function customizeFrom(key: string) {
  const base = palettes.find((p) => p.key === key) ?? palettes[0];
  patch({ palette: "custom", customPalette: { ...base.colors } });
}

const colorFields: (keyof CustomPalette)[] = ["accent", "surface", "text", "wood"];
const warnings = computed(() => {
  const c = custom.value;
  const out: string[] = [];
  if (contrast(c.text, c.surface) < 4.5) out.push(t("theme.warnText"));
  if (contrast("#ffffff", c.accent) < 2.5) out.push(t("theme.warnAccent"));
  if (contrast("#ffffff", c.wood) < 3) out.push(t("theme.warnWood"));
  return out;
});

const sliders: { key: "radius" | "cardOpacity" | "cardBlur"; min: number; max: number; unit: string }[] = [
  { key: "radius", min: 0, max: 24, unit: "px" },
  { key: "cardOpacity", min: 40, max: 100, unit: "%" },
  { key: "cardBlur", min: 0, max: 30, unit: "px" },
];

async function pickBackground() {
  if (!inTauri) {
    message.value = { ok: false, text: t("theme.previewNoPick") };
    return;
  }
  const file = await open({ multiple: false, filters: [{ name: t("theme.image"), extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
  if (typeof file === "string") patch({ backgroundImage: file });
}

// ---------- 分享 ----------
const exportName = ref(t("theme.myTheme"));
const exportAuthor = ref(accounts.selected?.name ?? "");

async function exportTheme() {
  if (!inTauri) {
    message.value = { ok: false, text: t("theme.previewNoExport") };
    return;
  }
  const name = exportName.value.trim() || t("theme.myTheme");
  const path = await save({ defaultPath: `${name.replace(/[\\/:*?"<>|]/g, "_")}.godtheme`, filters: [{ name: t("theme.fileType"), extensions: ["godtheme"] }] });
  if (!path) return;
  await flush(); // 导出读的是磁盘上的设置，先把没保存的改动写进去
  try {
    await api.themeExport(path, name, exportAuthor.value);
    message.value = { ok: true, text: t("theme.exported", { path }) };
  } catch (e) {
    message.value = { ok: false, text: errText(e) };
  }
}

async function importTheme() {
  if (!inTauri) {
    message.value = { ok: false, text: t("theme.previewNoPick") };
    return;
  }
  const file = await open({ multiple: false, filters: [{ name: t("theme.fileType"), extensions: ["godtheme"] }] });
  if (typeof file === "string") pendingThemeImport.value = file;
}

async function resetTweaks() {
  await patch({ tweaks: { ...DEFAULT_TWEAKS } });
}

// 一键恢复默认外观：点两下才生效，防误触
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
  await patch({
    layout: "classic",
    palette: "mcbbs",
    theme: "light",
    backgroundImage: null,
    customPalette: { ...palettes[0].colors },
    tweaks: { ...DEFAULT_TWEAKS },
  });
  message.value = { ok: true, text: t("theme.resetDone") };
}
</script>

<template>
  <div v-if="store.settings" class="theme-center">
    <div class="topbar">
      <span class="muted">{{ $t("theme.topHint") }}</span>
      <button class="btn small" :class="{ danger: confirmReset }" @click="resetAll">
        {{ confirmReset ? $t("theme.resetConfirm") : `↺ ${$t("theme.reset")}` }}
      </button>
    </div>
    <div class="box">
      <div class="box-title">{{ $t("theme.layoutTitle") }}</div>
      <div class="box-body cards">
        <button v-for="l in layouts" :key="l" class="layout-card" :class="{ active: currentLayout === l }" @click="patch({ layout: l })">
          <!-- 迷你示意图 -->
          <div class="mini" :class="l">
            <template v-if="l === 'classic'">
              <i class="m-banner" /><i class="m-nav" />
              <div class="m-body"><i class="m-box w2" /><i class="m-box" /></div>
            </template>
            <template v-else-if="l === 'sidebar'">
              <i class="m-side" />
              <div class="m-body"><i class="m-title" /><i class="m-box w2" /><i class="m-box" /></div>
            </template>
            <template v-else>
              <i class="m-rail" />
              <div class="m-body"><i class="m-title big" /><i class="m-box round w2" /><i class="m-box round" /></div>
            </template>
          </div>
          <div class="card-text">
            <b>{{ $t(`appearance.layouts.${l}.name`) }}<span v-if="l === 'classic'" class="default">{{ $t("settings.default") }}</span></b>
            <span class="muted">{{ $t(`theme.layoutDesc.${l}`) }}</span>
          </div>
        </button>
      </div>
    </div>

    <div class="box">
      <div class="box-title">{{ $t("theme.paletteTitle") }} <span class="extra muted">{{ $t("theme.paletteHint") }}</span></div>
      <div class="box-body">
        <div class="swatches">
          <button v-for="p in palettes" :key="p.key" class="swatch" :class="{ active: store.settings.palette === p.key }" @click="patch({ palette: p.key })">
            <span class="chips">
              <i :style="{ background: p.colors.accent }" /><i :style="{ background: p.colors.surface }" /><i :style="{ background: p.colors.wood }" />
            </span>
            <span>{{ $t(`appearance.palettes.${p.key}`) }}</span>
          </button>
          <button class="swatch" :class="{ active: isCustom }" @click="isCustom || patch({ palette: 'custom' })">
            <span class="chips">
              <i :style="{ background: custom.accent }" /><i :style="{ background: custom.surface }" /><i :style="{ background: custom.wood }" />
            </span>
            <span>✎ {{ $t("appearance.palettes.custom") }}</span>
          </button>
        </div>

        <div v-if="isCustom" class="editor">
          <div v-for="f in colorFields" :key="f" class="color-row">
            <input type="color" :value="custom[f]" @input="setColor(f, ($event.target as HTMLInputElement).value)" />
            <div class="color-text">
              <b>{{ $t(`theme.color.${f}.label`) }}</b>
              <span class="muted">{{ $t(`theme.color.${f}.hint`) }}</span>
            </div>
            <input class="input hex" :value="custom[f]" maxlength="7" spellcheck="false" @change="setColor(f, ($event.target as HTMLInputElement).value)" />
          </div>
          <div class="editor-foot">
            <span class="muted">{{ $t("theme.startFrom") }}</span>
            <a v-for="p in palettes" :key="p.key" class="from" :title="$t(`appearance.palettes.${p.key}`)" :style="{ background: p.colors.accent }" @click="customizeFrom(p.key)" />
            <span class="muted dark-hint">{{ $t("theme.darkAuto") }}</span>
          </div>
          <div v-for="w in warnings" :key="w" class="mod-tip warn">{{ $t("common.modTip") }}{{ w }}</div>
        </div>
      </div>
    </div>

    <div class="box">
      <div class="box-title">{{ $t("theme.presetTitle") }} <span class="extra muted">{{ $t("theme.presetHint") }}</span></div>
      <div class="box-body presets">
        <button v-for="p in presets" :key="p.key" class="preset" @click="patch(p.patch)">
          <span class="chips small">
            <template v-if="p.patch.customPalette">
              <i :style="{ background: p.patch.customPalette.accent }" /><i :style="{ background: p.patch.customPalette.surface }" /><i :style="{ background: p.patch.customPalette.wood }" />
            </template>
            <template v-else>
              <i :style="{ background: palettes[0].colors.accent }" /><i :style="{ background: palettes[0].colors.surface }" /><i :style="{ background: palettes[0].colors.wood }" />
            </template>
          </span>
          <span class="preset-text">
            <b>{{ $t(`theme.presets.${p.key}.name`) }}</b>
            <span class="muted">{{ $t(`theme.presets.${p.key}.desc`) }}</span>
          </span>
        </button>
      </div>
    </div>

    <div class="box">
      <div class="box-title">
        {{ $t("theme.tweaksTitle") }}
        <a class="extra" @click="resetTweaks">{{ $t("theme.resetTweaks") }}</a>
      </div>
      <div class="box-body">
        <div v-for="s in sliders" :key="s.key" class="row">
          <label>{{ $t(`theme.slider.${s.key}`) }}</label>
          <input type="range" :min="s.min" :max="s.max" :value="tweaks[s.key] ?? s.max" class="range" :class="{ unset: tweaks[s.key] === null }" @input="setTweak(s.key, Number(($event.target as HTMLInputElement).value))" />
          <span class="val">{{ tweaks[s.key] === null ? $t("theme.layoutDefault") : tweaks[s.key] + s.unit }}</span>
          <a v-if="tweaks[s.key] !== null" class="reset" @click="setTweak(s.key, null)">{{ $t("settings.default") }}</a>
        </div>
        <div class="row">
          <label>{{ $t("theme.bgBlur") }}</label>
          <input type="range" min="0" max="20" :value="tweaks.bgBlur" class="range" @input="setTweak('bgBlur', Number(($event.target as HTMLInputElement).value))" />
          <span class="val">{{ tweaks.bgBlur }}px</span>
        </div>
        <div class="row">
          <label>{{ $t("theme.bgDim") }}</label>
          <input type="range" min="0" max="80" :value="tweaks.bgDim" class="range" @input="setTweak('bgDim', Number(($event.target as HTMLInputElement).value))" />
          <span class="val">{{ tweaks.bgDim }}%</span>
        </div>
        <div class="row">
          <label>{{ $t("theme.fontSize") }}</label>
          <input type="range" min="12" max="16" :value="tweaks.fontSize" class="range" @input="setTweak('fontSize', Number(($event.target as HTMLInputElement).value))" />
          <span class="val">{{ tweaks.fontSize }}px</span>
        </div>
        <div class="row">
          <label>{{ $t("theme.font") }}</label>
          <input class="input font" list="font-list" :value="tweaks.font" :placeholder="$t('theme.fontDefault')" @change="setTweak('font', ($event.target as HTMLInputElement).value.trim())" />
          <datalist id="font-list"><option v-for="f in fonts" :key="f" :value="f" /></datalist>
          <span class="muted small">{{ $t("theme.fontHint") }}</span>
        </div>
        <div class="row">
          <label>{{ $t("theme.animations") }}</label>
          <label class="check"><input type="checkbox" :checked="tweaks.animations" @change="setTweak('animations', ($event.target as HTMLInputElement).checked)" /> {{ $t("theme.animationsHint") }}</label>
        </div>
      </div>
    </div>

    <div class="box">
      <div class="box-title">{{ $t("theme.modeTitle") }}</div>
      <div class="box-body">
        <div class="row">
          <label>{{ $t("appearance.mode") }}</label>
          <div class="seg">
            <a v-for="m in themes" :key="m" :class="{ active: store.settings.theme === m }" @click="patch({ theme: m })">{{ $t(`appearance.modes.${m}`) }}</a>
          </div>
        </div>
        <div class="row">
          <label>{{ $t("theme.background") }}</label>
          <div class="bg-field">
            <div class="bg-thumb" :style="backgroundUrl ? { backgroundImage: `url(${backgroundUrl})` } : undefined">
              <span v-if="!backgroundUrl" class="muted">{{ $t("theme.bgBuiltin") }}</span>
            </div>
            <div class="bg-actions">
              <span class="muted path">{{ store.settings.backgroundImage ?? $t("theme.bgUnset") }}</span>
              <div>
                <button class="btn small" @click="pickBackground">{{ $t("theme.pickImage") }}</button>
                <button v-if="store.settings.backgroundImage" class="btn small" @click="patch({ backgroundImage: null })">{{ $t("theme.restoreDefault") }}</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="box">
      <div class="box-title">{{ $t("theme.shareTitle") }} <span class="extra muted">{{ $t("theme.shareHint") }}</span></div>
      <div class="box-body">
        <div class="row">
          <label>{{ $t("versions.export") }}</label>
          <input v-model="exportName" class="input" :placeholder="$t('theme.themeName')" maxlength="40" />
          <input v-model="exportAuthor" class="input" :placeholder="$t('export.author')" maxlength="40" />
          <button class="btn primary" @click="exportTheme">{{ $t("theme.exportCurrent") }}</button>
        </div>
        <div class="row">
          <label>{{ $t("theme.import") }}</label>
          <button class="btn" @click="importTheme">{{ $t("theme.pickTheme") }}</button>
          <span class="muted small">{{ $t("theme.importHint") }}</span>
        </div>
      </div>
    </div>

    <ButtonSettings />

    <p v-if="message" :class="message.ok ? 'ok' : 'error-text'">{{ message.text }}</p>
  </div>
</template>

<style scoped>
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
.layout-card, .swatch, .preset {
  font: inherit;
  color: var(--text);
  text-align: left;
  background: var(--bg-box-alt);
  border: 2px solid var(--border);
  border-radius: var(--radius);
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s, box-shadow 0.15s;
}
.layout-card { display: flex; flex-direction: column; padding: 10px; gap: 10px; }
.layout-card:hover, .swatch:hover, .preset:hover { border-color: var(--border-strong); transform: translateY(-1px); }
.layout-card.active, .swatch.active { border-color: var(--grass); box-shadow: 0 0 0 3px color-mix(in srgb, var(--grass) 25%, transparent); }
.card-text { display: flex; flex-direction: column; gap: 4px; font-size: 12px; line-height: 1.5; }
.card-text b { font-size: 14px; }
.default { margin-left: 6px; padding: 0 6px; border-radius: 8px; font-size: 11px; font-weight: normal; background: var(--gold); color: #3d2f1f; }

/* 迷你示意图（固定配色，展示的是布局本身） */
.mini { display: flex; height: 96px; border-radius: 6px; overflow: hidden; border: 1px solid var(--border); background: var(--page-bg); }
.mini i { display: block; }
.mini.classic { flex-direction: column; background: #d9c9a3; }
.m-banner { height: 22px; background: linear-gradient(to bottom, #8ec5f0 60%, #6fb33d 60%, #6fb33d 75%, #6b4a2b 75%); }
.m-nav { height: 8px; background: #8b6a43; }
.mini .m-body { flex: 1; display: flex; flex-wrap: wrap; align-content: flex-start; gap: 5px; padding: 6px; }
.m-box { height: 22px; flex: 1 1 30%; background: #fbf2db; border: 1px solid #c9b083; border-radius: 2px; }
.m-box.w2 { flex-basis: 60%; }
.mini.sidebar { background: linear-gradient(to bottom, #8ec5f0, #dff0fb 70%, #6fb33d 70%, #6fb33d 75%, #866043 75%); }
.m-side { width: 26px; background: linear-gradient(to right, #6b4a2b, #4a3220); }
.m-title { height: 6px; flex-basis: 100%; max-width: 40%; border-radius: 3px; background: #fff; opacity: 0.9; }
.mini.sidebar .m-box { background: rgba(251, 242, 219, 0.9); border-radius: 4px; }
.mini.modern { background: radial-gradient(at 20% 10%, #b9e3a0, transparent 60%), radial-gradient(at 90% 80%, #bcd6f5, transparent 60%), #f3f4f7; }
.m-rail { width: 16px; background: rgba(255, 255, 255, 0.6); border-right: 1px solid rgba(0, 0, 0, 0.06); }
.m-title.big { height: 9px; background: #1c1c22; opacity: 0.75; }
.m-box.round { background: rgba(255, 255, 255, 0.8); border: none; border-radius: 6px; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08); }

.swatches { display: flex; flex-wrap: wrap; gap: 10px; }
.swatch { display: flex; flex-direction: column; align-items: center; gap: 6px; width: 96px; padding: 10px 6px 8px; font-size: 12px; }
.chips { display: flex; width: 60px; height: 34px; border-radius: 8px; overflow: hidden; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12); flex-shrink: 0; }
.chips.small { width: 44px; height: 28px; }
.chips i { flex: 1; }
.chips i:first-child { flex: 2; }

/* 自定义配色编辑器 */
.editor { margin-top: 14px; padding: 12px; border-radius: var(--radius); background: var(--bg-box-alt); border: 1px dashed var(--border-strong); }
.color-row { display: flex; align-items: center; gap: 12px; padding: 6px 0; }
.color-row input[type="color"] { width: 44px; height: 32px; padding: 0; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: none; cursor: pointer; }
.color-text { display: flex; flex-direction: column; flex: 1; font-size: 12px; }
.color-text b { font-size: 13px; }
.hex { width: 90px; font-family: var(--font-pixel); text-transform: lowercase; }
.editor-foot { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-top: 8px; font-size: 12px; }
.from { width: 18px; height: 18px; border-radius: 50%; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.2); cursor: pointer; }
.from:hover { transform: scale(1.15); }
.dark-hint { margin-left: auto; }
.warn { margin-top: 8px; font-size: 12px; }

.presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 10px; }
.preset { display: flex; align-items: center; gap: 10px; padding: 10px; }
.preset-text { display: flex; flex-direction: column; gap: 2px; font-size: 12px; min-width: 0; }
.preset-text b { font-size: 13px; }

.row { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); flex-wrap: wrap; }
.row:last-child { border-bottom: none; }
.row > label:first-child { width: 84px; flex-shrink: 0; text-align: right; color: var(--text-muted); }
.range { width: 220px; accent-color: var(--grass); }
.range.unset { opacity: 0.5; }
.val { width: 70px; font-family: var(--font-pixel); font-size: 12px; }
.reset { font-size: 12px; }
.font { width: 220px; }
.small { font-size: 12px; }
.check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.seg { display: inline-flex; border: 1px solid var(--border-strong); border-radius: 6px; overflow: hidden; }
.seg a { padding: 5px 16px; color: var(--text); background: var(--bg-box-alt); border-right: 1px solid var(--border); }
.seg a:last-child { border-right: none; }
.seg a:hover { text-decoration: none; filter: brightness(1.05); }
.seg a.active { background: var(--grass); color: #fff; font-weight: bold; }
.bg-field { display: flex; align-items: center; gap: 14px; flex: 1; min-width: 0; }
.bg-thumb { display: grid; place-items: center; width: 160px; height: 90px; flex-shrink: 0; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg-box-alt) center / cover; font-size: 12px; }
.bg-actions { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.path { font-size: 12px; word-break: break-all; }
.bg-actions .btn + .btn { margin-left: 6px; }
.ok { color: var(--grass); }
.topbar { display: flex; align-items: center; justify-content: flex-end; gap: 10px; margin-bottom: 10px; font-size: 12px; }
.btn.danger { color: #fff; background: var(--red); border-color: var(--red); }
</style>
