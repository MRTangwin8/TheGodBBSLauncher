<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { api } from "../../api";
import type { Screenshot } from "../../api/types";
import { formatBytes, timeAgo } from "../../utils";

// 版本管理 → 截图：screenshots 文件夹里的图片。图片经命令读成 blob 显示，滚到哪张才读哪张
const props = defineProps<{ versionId: string; gameDir: string }>();
const emit = defineEmits<{ message: [ok: boolean, text: string]; openFolder: [path: string] }>();

const PAGE = 48;
const shots = ref<Screenshot[] | null>(null);
const shown = ref(PAGE);
const urls = reactive<Record<string, string>>({});
const viewing = ref<number | null>(null);
const grid = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const visible = computed(() => (shots.value ?? []).slice(0, shown.value));
const current = computed(() => (viewing.value === null ? null : (shots.value ?? [])[viewing.value] ?? null));
const folder = computed(() => `${props.gameDir}${props.gameDir.includes("\\") ? "\\" : "/"}screenshots`);

onMounted(() => {
  observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        const name = (e.target as HTMLElement).dataset.name;
        if (name) fetchImage(name);
        observer?.unobserve(e.target);
      }
    },
    { rootMargin: "200px" },
  );
  window.addEventListener("keydown", onKey);
  load();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("keydown", onKey);
  revokeAll();
});

function revokeAll() {
  for (const k of Object.keys(urls)) {
    URL.revokeObjectURL(urls[k]);
    delete urls[k];
  }
}

async function load() {
  revokeAll();
  try {
    shots.value = await api.screenshotList(props.versionId);
  } catch (e) {
    shots.value = [];
    emit("message", false, `${e}`);
  }
  observe();
}

async function observe() {
  await nextTick();
  grid.value?.querySelectorAll<HTMLElement>("[data-name]").forEach((el) => {
    if (!urls[el.dataset.name!]) observer?.observe(el);
  });
}

async function fetchImage(name: string) {
  if (urls[name]) return;
  try {
    const data = await api.screenshotData(props.versionId, name);
    const type = /\.png$/i.test(name) ? "image/png" : "image/jpeg";
    urls[name] = URL.createObjectURL(new Blob([data], { type }));
  } catch {
    // 读不到（被删了、太大）就留空白格
  }
}

function more() {
  shown.value += PAGE;
  observe();
}

function open(i: number) {
  viewing.value = i;
  const s = shots.value?.[i];
  if (s) fetchImage(s.name);
}

function step(delta: number) {
  if (viewing.value === null || !shots.value?.length) return;
  open((viewing.value + delta + shots.value.length) % shots.value.length);
}

function onKey(e: KeyboardEvent) {
  if (viewing.value === null) return;
  if (e.key === "Escape") viewing.value = null;
  else if (e.key === "ArrowLeft") step(-1);
  else if (e.key === "ArrowRight") step(1);
}

async function remove(s: Screenshot) {
  if (!confirm(`删除截图「${s.name}」？\n文件会直接删掉，不进回收站。`)) return;
  try {
    await api.screenshotDelete(props.versionId, s.name);
    if (urls[s.name]) URL.revokeObjectURL(urls[s.name]);
    delete urls[s.name];
    const list = shots.value ?? [];
    const i = list.indexOf(s);
    shots.value = list.filter((x) => x !== s);
    if (viewing.value !== null) viewing.value = shots.value.length ? Math.min(i, shots.value.length - 1) : null;
    if (viewing.value !== null) fetchImage(shots.value[viewing.value].name);
  } catch (e) {
    emit("message", false, `${e}`);
  }
}

function fullTime(secs: number) {
  return new Date(secs * 1000).toLocaleString("zh-CN", { hour12: false });
}
</script>

<template>
  <div class="box-body bar">
    <span class="muted small grow">游戏里按 F2 截图，图片存在 <code>screenshots</code> 文件夹。点图片看大图，左右方向键切换。</span>
    <button class="btn small" @click="load">刷新</button>
    <button class="btn small" @click="emit('openFolder', folder)">打开截图文件夹</button>
  </div>
  <div v-if="!shots" class="box-body muted">正在读取截图…</div>
  <div v-else-if="!shots.length" class="box-body muted">还没有截图</div>
  <template v-else>
    <div ref="grid" class="box-body grid">
      <figure v-for="(s, i) in visible" :key="s.name" :data-name="s.name" class="shot" :title="s.name" @click="open(i)">
        <img v-if="urls[s.name]" :src="urls[s.name]" :alt="s.name" loading="lazy" />
        <div v-else class="ph"></div>
        <figcaption class="small"><span class="cap">{{ s.name }}</span><span class="muted">{{ timeAgo(s.modified) }}</span></figcaption>
      </figure>
    </div>
    <div v-if="shots.length > shown" class="box-body more">
      <button class="btn small" @click="more">再显示 {{ Math.min(PAGE, shots.length - shown) }} 张（共 {{ shots.length }} 张）</button>
    </div>
  </template>

  <Teleport to="body">
    <div v-if="current" class="viewer" @click.self="viewing = null">
      <button class="nav prev" title="上一张（←）" @click="step(-1)">‹</button>
      <img v-if="urls[current.name]" :src="urls[current.name]" :alt="current.name" class="big" />
      <div v-else class="big loading">正在读取…</div>
      <button class="nav next" title="下一张（→）" @click="step(1)">›</button>
      <div class="viewer-bar">
        <span class="grow">{{ current.name }} · {{ formatBytes(current.size) }} · {{ fullTime(current.modified) }}</span>
        <button class="btn small" @click="api.screenshotOpen(versionId, current.name).catch((e) => emit('message', false, `${e}`))">用看图程序打开</button>
        <button class="btn small" @click="api.screenshotReveal(versionId, current.name).catch((e) => emit('message', false, `${e}`))">在文件夹中显示</button>
        <button class="btn small" @click="remove(current)">删除</button>
        <button class="btn small" @click="viewing = null">关闭</button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.grow { flex: 1; min-width: 0; }
.small { font-size: 12px; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
.shot { margin: 0; cursor: pointer; border: 1px solid var(--border); background: var(--bg-box-alt); border-radius: var(--radius-sm); overflow: hidden; }
.shot:hover { border-color: var(--grass); }
.shot img, .shot .ph { display: block; width: 100%; aspect-ratio: 16 / 9; object-fit: cover; }
.shot .ph { background: var(--bg-title); }
figcaption { display: flex; gap: 6px; padding: 4px 6px; }
.cap { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.more { text-align: center; padding-top: 0; }
.viewer { position: fixed; inset: 0; z-index: 1000; background: rgba(0, 0, 0, 0.82); display: flex; align-items: center; justify-content: center; padding: 24px 64px 72px; }
.big { max-width: 100%; max-height: 100%; object-fit: contain; box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5); }
.big.loading { color: #ddd; }
.nav { position: absolute; top: 50%; transform: translateY(-50%); width: 44px; height: 64px; border: none; background: rgba(255, 255, 255, 0.12); color: #fff; font-size: 36px; cursor: pointer; border-radius: 6px; }
.nav:hover { background: rgba(255, 255, 255, 0.25); }
.prev { left: 12px; }
.next { right: 12px; }
.viewer-bar { position: absolute; left: 0; right: 0; bottom: 0; display: flex; align-items: center; gap: 8px; padding: 12px 16px; background: rgba(0, 0, 0, 0.55); color: #eee; font-size: 13px; flex-wrap: wrap; }
</style>
