<script setup lang="ts">
import { ref, watch } from "vue";
import { api } from "../api";
import type { ProbeResult, Settings } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";

const store = useSettingsStore();
const versions = useVersionsStore();
const form = ref<Settings | null>(null);
const message = ref<{ ok: boolean; text: string } | null>(null);
const probe = ref<ProbeResult[] | null>(null);
const probing = ref(false);

watch(() => store.settings, (s) => (form.value = s ? { ...s } : null), { immediate: true });

async function save() {
  if (!form.value) return;
  try {
    await store.save({ ...form.value, concurrency: Math.min(256, Math.max(1, Number(form.value.concurrency) || 64)) });
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
</style>
