<script setup lang="ts">
import { ref } from "vue";
import { inTauri } from "../api";
import { open } from "@tauri-apps/plugin-dialog";
import type { Settings } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { backgroundUrl, currentLayout, type Layout } from "../appearance";

// 主题中心：布局、配色、深浅色、背景图。点一下立刻保存生效
const store = useSettingsStore();
const message = ref<string | null>(null);

const layouts: { key: Layout; name: string; desc: string }[] = [
  { key: "classic", name: "经典论坛", desc: "第一版：横幅 + 顶部导航 + 面包屑，原汁原味的 MCBBS 版块" },
  { key: "sidebar", name: "木板侧栏", desc: "木板侧边栏、像素天空草地、半透明羊皮纸卡片，可收起成图标条" },
  { key: "modern", name: "完全现代", desc: "图标导航条、亚克力卡片、渐变晕染，只保留配色的强调色" },
];

// 色块：[强调色, 表面, 边框木头]
const palettes = [
  { key: "mcbbs", name: "MCBBS 经典", colors: ["#5e9c35", "#fbf2db", "#6b4a2b"] },
  { key: "grass", name: "草方块", colors: ["#4fa52f", "#f2f9ea", "#3f6e22"] },
  { key: "nether", name: "下界", colors: ["#d4502a", "#fceee8", "#5a1c14"] },
  { key: "end", name: "末地", colors: ["#8e5fc0", "#f6f1e0", "#3a2a52"] },
  { key: "ocean", name: "海洋", colors: ["#23a093", "#eaf6f8", "#1f4f6e"] },
  { key: "cherry", name: "樱花", colors: ["#df6a9c", "#fdf0f4", "#7a3a52"] },
  { key: "stone", name: "石头", colors: ["#5b7083", "#f1f2f3", "#3d4650"] },
];

const themes: { key: Settings["theme"]; label: string }[] = [
  { key: "light", label: "浅色" },
  { key: "dark", label: "深色" },
  { key: "system", label: "跟随系统" },
];

async function patch(p: Partial<Settings>) {
  if (!store.settings) return;
  message.value = null;
  // 先改本地立刻生效，连点几下也不会互相覆盖；保存失败再读回磁盘上的设置
  const next = { ...store.settings, ...p };
  store.settings = next;
  try {
    await store.save(next);
  } catch (e) {
    message.value = String(e);
    await store.load();
  }
}

async function pickBackground() {
  if (!inTauri) {
    message.value = "浏览器预览模式不能选择文件";
    return;
  }
  const file = await open({ multiple: false, filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
  if (typeof file === "string") patch({ backgroundImage: file });
}
</script>

<template>
  <div v-if="store.settings" class="theme-center">
    <div class="box">
      <div class="box-title">布局风格</div>
      <div class="box-body cards">
        <button v-for="l in layouts" :key="l.key" class="layout-card" :class="{ active: currentLayout === l.key }" @click="patch({ layout: l.key })">
          <!-- 迷你示意图 -->
          <div class="mini" :class="l.key">
            <template v-if="l.key === 'classic'">
              <i class="m-banner" /><i class="m-nav" />
              <div class="m-body"><i class="m-box w2" /><i class="m-box" /></div>
            </template>
            <template v-else-if="l.key === 'sidebar'">
              <i class="m-side" />
              <div class="m-body"><i class="m-title" /><i class="m-box w2" /><i class="m-box" /></div>
            </template>
            <template v-else>
              <i class="m-rail" />
              <div class="m-body"><i class="m-title big" /><i class="m-box round w2" /><i class="m-box round" /></div>
            </template>
          </div>
          <div class="card-text">
            <b>{{ l.name }}<span v-if="l.key === 'classic'" class="default">默认</span></b>
            <span class="muted">{{ l.desc }}</span>
          </div>
        </button>
      </div>
    </div>

    <div class="box">
      <div class="box-title">配色方案 <span class="extra muted">现代布局只取强调色</span></div>
      <div class="box-body swatches">
        <button v-for="p in palettes" :key="p.key" class="swatch" :class="{ active: (store.settings.palette || 'mcbbs') === p.key }" @click="patch({ palette: p.key })">
          <span class="chips">
            <i v-for="c in p.colors" :key="c" :style="{ background: c }" />
          </span>
          <span>{{ p.name }}</span>
        </button>
      </div>
    </div>

    <div class="box">
      <div class="box-title">明暗与背景</div>
      <div class="box-body">
        <div class="row">
          <label>明暗</label>
          <div class="seg">
            <a v-for="t in themes" :key="t.key" :class="{ active: store.settings.theme === t.key }" @click="patch({ theme: t.key })">{{ t.label }}</a>
          </div>
        </div>
        <div class="row">
          <label>背景图</label>
          <div class="bg-field">
            <div class="bg-thumb" :style="backgroundUrl ? { backgroundImage: `url(${backgroundUrl})` } : undefined">
              <span v-if="!backgroundUrl" class="muted">布局自带</span>
            </div>
            <div class="bg-actions">
              <span class="muted path">{{ store.settings.backgroundImage ?? "未设置：经典用论坛点阵，侧栏用像素风景，现代用渐变" }}</span>
              <div>
                <button class="btn small" @click="pickBackground">选择图片…</button>
                <button v-if="store.settings.backgroundImage" class="btn small" @click="patch({ backgroundImage: null })">恢复默认</button>
              </div>
            </div>
          </div>
        </div>
        <p v-if="message" class="error-text">{{ message }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
.layout-card, .swatch {
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
.layout-card:hover, .swatch:hover { border-color: var(--border-strong); transform: translateY(-1px); }
.layout-card.active, .swatch.active { border-color: var(--grass); box-shadow: 0 0 0 3px color-mix(in srgb, var(--grass) 25%, transparent); }
.card-text { display: flex; flex-direction: column; gap: 4px; font-size: 12px; line-height: 1.5; }
.card-text b { font-size: 14px; }
.default { margin-left: 6px; padding: 0 6px; border-radius: 8px; font-size: 11px; font-weight: normal; background: var(--gold); color: #3d2f1f; }

/* 迷你示意图 */
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
.chips { display: flex; width: 60px; height: 34px; border-radius: 8px; overflow: hidden; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12); }
.chips i { flex: 1; }
.chips i:first-child { flex: 2; }

.row { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); }
.row:last-of-type { border-bottom: none; }
.row > label { width: 60px; flex-shrink: 0; text-align: right; color: var(--text-muted); }
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
</style>
