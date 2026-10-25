<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "../api";
import type { ThemePreview } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { reloadBackground } from "../appearance";

const props = defineProps<{ path: string }>();
const emit = defineEmits<{ close: [] }>();
const store = useSettingsStore();

const preview = ref<ThemePreview | null>(null);
const error = ref<string | null>(null);
const busy = ref(false);
const fileName = computed(() => props.path.split(/[\\/]/).pop());

const layoutName: Record<string, string> = { classic: "经典论坛", sidebar: "木板侧栏", modern: "完全现代" };
const paletteName: Record<string, string> = {
  mcbbs: "MCBBS 经典",
  grass: "草方块",
  nether: "下界",
  end: "末地",
  ocean: "海洋",
  cherry: "樱花",
  stone: "石头",
  custom: "自定义",
};
const themeName: Record<string, string> = { light: "浅色", dark: "深色", system: "跟随系统" };

onMounted(async () => {
  try {
    preview.value = await api.themePreview(props.path);
  } catch (e) {
    error.value = String(e);
  }
});

async function apply() {
  busy.value = true;
  try {
    store.settings = await api.themeImport(props.path);
    await reloadBackground();
    emit("close");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        导入主题
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body">
        <p class="muted small">{{ fileName }}</p>
        <p v-if="error" class="error-text">{{ error }}</p>
        <div v-else-if="!preview" class="muted">正在读取主题…</div>
        <template v-else>
          <div class="hero" :style="preview.background ? { backgroundImage: `url(${preview.background})` } : undefined">
            <div v-if="preview.palette === 'custom' && preview.customPalette" class="chips">
              <i v-for="c in preview.customPalette" :key="c" :style="{ background: c }" />
            </div>
            <span v-if="!preview.background" class="muted">没有背景图（用布局自带的）</span>
          </div>
          <h3 class="name">{{ preview.name || "未命名主题" }} <span v-if="preview.author" class="muted">by {{ preview.author }}</span></h3>
          <dl class="facts">
            <dt>布局</dt><dd>{{ preview.layout ? layoutName[preview.layout] : "不改" }}</dd>
            <dt>配色</dt><dd>{{ preview.palette ? paletteName[preview.palette] : "不改" }}</dd>
            <dt>明暗</dt><dd>{{ preview.theme ? themeName[preview.theme] : "不改" }}</dd>
          </dl>
          <p class="muted small">导入后会替换当前的外观细调和背景图，之后随时可以在主题中心改回来。</p>
        </template>
      </div>
      <div class="actions">
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn primary" :disabled="!preview || busy" @click="apply">{{ busy ? "应用中…" : "应用主题" }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 480px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.small { font-size: 12px; margin: 4px 0; }
.hero {
  position: relative;
  display: grid;
  place-items: center;
  height: 150px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-box-alt) center / cover;
}
.chips { position: absolute; right: 10px; bottom: 10px; display: flex; width: 80px; height: 24px; border-radius: 6px; overflow: hidden; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3); }
.chips i { flex: 1; }
.name { font-size: 17px; margin: 10px 0 4px; color: var(--brown-dark); }
.name .muted { font-size: 12px; font-weight: normal; }
.facts { display: grid; grid-template-columns: 50px 1fr; gap: 4px 10px; margin: 8px 0; padding: 8px 10px; background: var(--bg-box-alt); border-radius: var(--radius); }
.facts dt { color: var(--text-muted); }
.facts dd { margin: 0; }
.actions { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
</style>
