<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { api } from "../api";
import type { JavaInstall, MemoryInfo, ProbeResult, Settings } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";

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

onMounted(async () => {
  memory.value = await api.memoryInfo();
  await scanJava(false);
});

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
      concurrency: Math.min(256, Math.max(1, Number(form.value.concurrency) || 64)),
      javaPath: form.value.javaPath || null,
      memoryMb: optNumber(form.value.memoryMb),
      windowWidth: optNumber(form.value.windowWidth),
      windowHeight: optNumber(form.value.windowHeight),
    });
    message.value = { ok: true, text: "保存成功，神权已生效" };
    versions.loadInstalled();
  } catch (e) {
    message.value = { ok: false, text: String(e) };
  }
}

async function runProbe() {
  probing.value = true;
  probe.value = null;
  try {
    probe.value = await api.probeMirrors();
  } catch (e) {
    message.value = { ok: false, text: String(e) };
  } finally {
    probing.value = false;
  }
}

const sourceName = { official: "Mojang 官方", bmclapi: "BMCLAPI" };
</script>

<template>
  <div class="box">
    <div class="box-title">设置</div>
    <div v-if="form" class="box-body">
      <div class="row">
        <label>游戏目录</label>
        <div class="field">
          <input v-model="form.gameDir" class="input wide" />
          <div class="muted hint">libraries / assets 在所有版本间共享，每个版本的存档以后会做隔离</div>
        </div>
      </div>
      <div class="row">
        <label>下载源</label>
        <div class="field">
          <select v-model="form.downloadSource" class="input">
            <option value="auto">自动（测速选最快）</option>
            <option value="official">Mojang 官方</option>
            <option value="bmclapi">BMCLAPI 镜像</option>
          </select>
          <button class="btn" :disabled="probing" @click="runProbe">{{ probing ? "测速中…" : "测速" }}</button>
          <span v-if="probe" class="probe">
            <span v-for="p in probe" :key="p.source">
              {{ sourceName[p.source] }}：<b>{{ p.millis === null ? "超时" : p.millis + " ms" }}</b>
            </span>
          </span>
          <div class="muted hint">无论选哪个，失败时都会自动换另一个源重试</div>
        </div>
      </div>
      <div class="row">
        <label>最大并发</label>
        <div class="field">
          <input v-model.number="form.concurrency" type="number" min="1" max="256" class="input narrow" />
          <span class="muted hint">默认 64，网络差可以调低</span>
        </div>
      </div>
      <h3 class="section">游戏与 Java <span class="muted">—— 完全神权，参数你说了算</span></h3>
      <div class="row">
        <label>Java</label>
        <div class="field">
          <select v-model="form.javaPath" class="input wide">
            <option :value="null">自动选择（按版本要求，没有就自动下载 Mojang 官方 Java）</option>
            <option v-for="j in javas" :key="j.path" :value="j.path">
              Java {{ j.version }} {{ j.arch }} {{ j.vendor ?? "" }} — {{ j.path }}
            </option>
            <option v-if="form.javaPath && !javas.some((j) => j.path === form!.javaPath)" :value="form.javaPath">
              {{ form.javaPath }}
            </option>
          </select>
          <button class="btn" :disabled="scanning" @click="scanJava(true)">{{ scanning ? "扫描中…" : "重新扫描" }}</button>
          <div class="muted hint">本机找到 {{ javas.length }} 个 Java</div>
        </div>
      </div>
      <div class="row">
        <label>最大内存</label>
        <div class="field">
          <input v-model="form.memoryMb" type="number" min="512" step="256" class="input narrow" placeholder="自动" />
          <span class="muted">MB</span>
          <span v-if="memory" class="muted hint">
            留空为自动（本机 {{ (memory.totalMb / 1024).toFixed(1) }} GB 内存，自动分配 {{ memory.autoMb }} MB）
          </span>
        </div>
      </div>
      <div class="row">
        <label>窗口大小</label>
        <div class="field">
          <input v-model="form.windowWidth" type="number" class="input narrow" placeholder="默认" />
          ×
          <input v-model="form.windowHeight" type="number" class="input narrow" placeholder="默认" />
        </div>
      </div>
      <div class="row">
        <label>版本隔离</label>
        <div class="field">
          <label class="check"><input v-model="form.isolateVersions" type="checkbox" /> 每个版本使用独立的游戏目录（存档、模组、设置互不影响）</label>
        </div>
      </div>
      <div class="row">
        <label>JVM 参数</label>
        <div class="field">
          <input v-model="form.jvmArgs" class="input wide mono" placeholder="例如 -XX:+UseZGC -Dsome.prop=1" />
        </div>
      </div>
      <div class="row">
        <label>游戏参数</label>
        <div class="field">
          <input v-model="form.gameArgs" class="input wide mono" placeholder="例如 --fullscreen" />
        </div>
      </div>

      <div class="row">
        <label></label>
        <div class="field">
          <button class="btn primary" @click="save">保存</button>
          <span v-if="message" :class="message.ok ? 'ok' : 'error-text'">{{ message.text }}</span>
        </div>
      </div>
    </div>
  </div>

  <div class="box">
    <div class="box-title">关于</div>
    <div class="box-body about">
      <p><b>神权论坛启动器</b> v0.1.0 —— 献给 MCBBS（我的世界中文论坛）的一封情书。</p>
      <p>本程序是<b>非官方粉丝纪念作品</b>，与 MCBBS、Mojang Studios、Microsoft 均无关联；界面元素全部为原创绘制，未使用任何官方 Logo 或图片素材。</p>
      <p>本程序不分发 Minecraft 游戏文件，所有文件均从 Mojang 官方服务器或 BMCLAPI 镜像下载。感谢 bangbang93 提供 BMCLAPI。</p>
    </div>
  </div>
</template>

<style scoped>
.row { display: flex; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); }
.row:last-child { border-bottom: none; }
.row label { width: 90px; flex-shrink: 0; text-align: right; padding-top: 6px; color: var(--text-muted); }
.field { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; flex: 1; }
.wide { flex: 1; min-width: 300px; }
.narrow { width: 90px; }
.hint { width: 100%; font-size: 12px; }
.probe { display: flex; gap: 12px; }
.ok { color: var(--grass); }
.about p { margin: 6px 0; line-height: 1.7; }
.section { font-size: 14px; margin: 14px 0 4px; padding-top: 10px; border-top: 1px solid var(--border); color: var(--brown-dark); }
.section .muted { font-size: 12px; font-weight: normal; }
.check { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.mono { font-family: var(--font-pixel); }
.field label.check { width: auto; text-align: left; padding-top: 0; color: var(--text); }
</style>
