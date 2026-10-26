<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { api, inTauri } from "../api";
import { getVersion } from "@tauri-apps/api/app";
import type { JavaInstall, MemoryInfo, ProbeResult, RulesInfo, Settings } from "../api/types";
import { useSettingsStore } from "../stores/settings";
import { useVersionsStore } from "../stores/versions";
import { checkForUpdate } from "../update";
import GameDirDialog from "../components/GameDirDialog.vue";
import type { GameDirInfo } from "../api/types";
import { announcementsLoading, announcementsNote, loadAnnouncements, restoreAnnouncements } from "../announcements";

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
async function showDirDialog() {
  dirInfo.value = await api.gameDirInfo();
  dirDialog.value = true;
}
async function openGameDir() {
  if (form.value) await api.quickOpenFolder(form.value.gameDir).catch((e) => (message.value = { ok: false, text: `${e}` }));
}
// 老版本默认放在 AppData 里：提醒一下可以搬到启动器旁边
const suggestMove = ref(false);

const rules = ref<RulesInfo | null>(null);
const rulesBusy = ref(false);
const rulesMsg = ref<{ ok: boolean; text: string } | null>(null);

onMounted(async () => {
  api.gameDirInfo().then((i) => (suggestMove.value = i.legacyHasData && i.current !== i.recommended)).catch(() => {});
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
    rulesMsg.value = { ok: true, text: `更新成功，现在共 ${rules.value.count} 条规则` };
  } catch (e) {
    rulesMsg.value = { ok: false, text: String(e) };
  } finally {
    rulesBusy.value = false;
  }
}

async function resetRules() {
  rules.value = await api.resetCrashRules();
  rulesMsg.value = { ok: true, text: "已恢复内置规则库" };
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

// 检查更新：手动查，查到新版本会弹窗；自动检查的开关改了立刻保存
const checking = ref(false);
const updateMsg = ref<{ ok: boolean; text: string } | null>(null);
async function manualCheck() {
  checking.value = true;
  updateMsg.value = null;
  try {
    const info = await checkForUpdate(true);
    if (!info?.latest) updateMsg.value = { ok: true, text: "GitHub 上还没有发布版本" };
    else if (info.hasUpdate) updateMsg.value = { ok: true, text: `发现新版本 v${info.latest}` };
    else updateMsg.value = { ok: true, text: `已经是最新版（v${info.current}）` };
  } catch (e) {
    updateMsg.value = { ok: false, text: String(e) };
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
    updateMsg.value = { ok: false, text: String(e) };
  }
}
async function setAutoCheck(on: boolean) {
  if (!store.settings) return;
  try {
    await store.save({ ...store.settings, autoCheckUpdate: on });
  } catch (e) {
    updateMsg.value = { ok: false, text: String(e) };
  }
}

// 版本号读打包时写进程序的那个，不在这里写死
const appVersion = ref("");
if (inTauri) getVersion().then((v) => (appVersion.value = v));
</script>

<template>
  <div class="box">
    <div class="box-title">外观</div>
    <div class="box-body">
      布局、配色、深浅色和背景图都在 <RouterLink to="/theme">主题中心</RouterLink> 里切换。
    </div>
  </div>

  <div class="box">
    <div class="box-title">设置</div>
    <div v-if="form" class="box-body">
      <div class="row">
        <label>游戏目录</label>
        <div class="field">
          <div class="dirline">
            <code class="dirpath">{{ form.gameDir }}</code>
            <button class="btn small" @click="showDirDialog">更改 / 迁移…</button>
            <button class="btn small" @click="openGameDir">打开文件夹</button>
          </div>
          <div class="muted hint">默认放在启动器旁边的 .minecraft 里，卸载启动器不会删它。libraries / assets 在所有版本间共享</div>
          <div v-if="suggestMove" class="hint"><a @click="showDirDialog">你的游戏数据不在启动器旁边，可以一键迁移过去 »</a></div>
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
        <label>CurseForge Key</label>
        <div class="field">
          <input v-model="form.curseforgeApiKey" type="password" class="input wide mono" placeholder="留空则通过 MCIM 社区镜像访问 CurseForge" autocomplete="off" />
          <div class="muted hint">
            可在 console.curseforge.com 免费申请。填写后直连官方接口；Modrinth 不需要 Key。
          </div>
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

  <div v-if="form" class="box">
    <div class="box-title">崩溃分析规则库</div>
    <div class="box-body">
      <p v-if="rules" class="rules-info">
        当前 <b>{{ rules.count }}</b> 条规则（版本 {{ rules.version }}，{{ rules.custom ? "已加载自定义/在线规则" : "内置规则" }}）
      </p>
      <div class="row">
        <label>更新地址</label>
        <div class="field">
          <input v-model="form.crashRulesUrl" class="input wide mono" placeholder="https://…/crash-rules.json（留空只用内置规则）" />
          <button class="btn" :disabled="!form.crashRulesUrl || rulesBusy" @click="updateRules">{{ rulesBusy ? "更新中…" : "立即更新" }}</button>
          <button v-if="rules?.custom" class="btn" :disabled="rulesBusy" @click="resetRules">恢复内置</button>
          <div class="muted hint">规则是 JSON 格式，和内置规则同 id 的会覆盖、新 id 会追加。改完记得点上面的「保存」记住地址。</div>
        </div>
      </div>
      <p v-if="rulesMsg" :class="rulesMsg.ok ? 'ok' : 'error-text'">{{ rulesMsg.text }}</p>
    </div>
  </div>

  <div class="box">
    <div class="box-title">关于</div>
    <div class="box-body about">
      <p><b>神权论坛启动器</b> {{ appVersion ? `v${appVersion}` : "（浏览器预览）" }} —— 献给 MCBBS（我的世界中文论坛）的一封情书。</p>
      <p>本程序是<b>非官方粉丝纪念作品</b>，与 MCBBS、Mojang Studios、Microsoft 均无关联；界面元素全部为原创绘制，未使用任何官方 Logo 或图片素材。</p>
      <p>本程序不分发 Minecraft 游戏文件，所有文件均从 Mojang 官方服务器或 BMCLAPI 镜像下载。感谢 bangbang93 提供 BMCLAPI，感谢 MCIM 提供 Modrinth / CurseForge 镜像。</p>
      <div class="update-row">
        <button class="btn small" :disabled="checking" @click="manualCheck">{{ checking ? "检查中…" : "检查更新" }}</button>
        <label class="check">
          <input type="checkbox" :checked="store.settings?.autoCheckUpdate !== false" @change="setAutoCheck(($event.target as HTMLInputElement).checked)" />
          启动时自动检查更新
        </label>
        <span v-if="updateMsg" :class="updateMsg.ok ? 'ok' : 'error-text'">{{ updateMsg.text }}</span>
      </div>
      <div class="update-row">
        <button class="btn small" :disabled="announcementsLoading" @click="loadAnnouncements(true)">{{ announcementsLoading ? "刷新中…" : "刷新公告" }}</button>
        <label class="check">
          <input type="checkbox" :checked="store.settings?.announcementsEnabled !== false" @change="setAnnouncements(($event.target as HTMLInputElement).checked)" />
          启动时获取公告
        </label>
        <a v-if="store.settings?.dismissedAnnouncements?.length" @click="restoreAnnouncements()">重新显示已关闭的公告</a>
        <span v-if="announcementsNote" :class="announcementsNote.ok ? 'ok' : 'error-text'">{{ announcementsNote.text }}</span>
      </div>
      <p class="muted hint">检查更新和获取公告都只会向 GitHub 读取公开内容（GitHub 能看到你的网络地址），不会上传任何其他信息。公告只在启动时获取一次；更新不会自动安装，点了「立即更新」才会。</p>
    </div>
  </div>
  <GameDirDialog v-if="dirDialog && dirInfo" :info="dirInfo" @close="dirDialog = false" />
</template>

<style scoped>
.row { display: flex; gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--border); }
.row:last-child { border-bottom: none; }
.row label { width: 90px; flex-shrink: 0; text-align: right; padding-top: 6px; color: var(--text-muted); }
.field { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; flex: 1; }
.wide { flex: 1; min-width: 300px; }
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
.field label.check { width: auto; text-align: left; padding-top: 0; color: var(--text); }
</style>
