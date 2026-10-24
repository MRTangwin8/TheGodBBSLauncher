<script setup lang="ts">
import { computed, onMounted } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { inTauri } from "../../api";
import { kindLabel as accountKind } from "../../stores/accounts";
import { useSettingsStore } from "../../stores/settings";
import { useLauncher } from "../../composables/useLauncher";
import { formatDuration, loaderLabel, timeAgo } from "../../utils";
import PixelIcon from "../PixelIcon.vue";

const { selected, byRecent, current, runningRun, launchingThis, launchStage, launch, versions, accounts, game, router } = useLauncher();
const settings = useSettingsStore();

const recent = computed(() => byRecent.value.filter((v) => v.lastPlayed).slice(0, 5));
const totalPlay = computed(() => versions.installed.reduce((s, v) => s + v.playTimeSecs, 0));
// 积分：每小时 10 分 + 每个版本 5 分，纯属娱乐
const points = computed(() => Math.floor(totalPlay.value / 360) + versions.installed.length * 5);

onMounted(() => versions.loadInstalled());

function describe(v: { loader: string | null; mcVersion: string | null; id: string }) {
  const mc = v.mcVersion && v.mcVersion !== v.id ? `MC ${v.mcVersion}` : "";
  return [mc, v.loader ? loaderLabel[v.loader] : "原版"].filter(Boolean).join(" · ");
}

/** 在资源管理器里打开这个版本的文件夹 */
function openFolder() {
  const root = settings.settings?.gameDir;
  if (!root || !selected.value || !inTauri) return;
  const sep = root.includes("\\") ? "\\" : "/";
  revealItemInDir(`${root}${sep}versions${sep}${selected.value}`);
}
</script>

<template>
  <div class="home">
    <!-- 启动卡片 -->
    <section class="hero box">
      <template v-if="versions.installed.length">
        <div class="hero-main">
          <div class="pickers">
            <label class="picker">
              <span class="picker-label">版本</span>
              <select v-model="selected" class="picker-select">
                <option v-for="v in byRecent" :key="v.id" :value="v.id">{{ v.id }}</option>
              </select>
            </label>
            <label class="picker">
              <span class="picker-label">账号</span>
              <select
                v-if="accounts.accounts.length"
                :value="accounts.selectedId ?? ''"
                class="picker-select"
                @change="accounts.select(($event.target as HTMLSelectElement).value)"
              >
                <option v-for="a in accounts.accounts" :key="a.id" :value="a.id">{{ a.name }}（{{ accountKind[a.kind] }}）</option>
              </select>
              <RouterLink v-else to="/account" class="picker-empty">还没有账号，点这里添加 »</RouterLink>
            </label>
          </div>
          <p v-if="current" class="hero-meta">
            <span v-if="current.modpack" class="tag modpack">整合包</span>
            {{ describe(current) }} · 已玩 {{ formatDuration(current.playTimeSecs) }} · {{ timeAgo(current.lastPlayed) }}
          </p>
          <div v-if="current" class="quick-actions">
            <a @click="openFolder"><PixelIcon name="folder" :size="12" /> 打开文件夹</a>
            <a v-if="current.loader" @click="router.push({ path: '/resources', query: { version: current.id } })"><PixelIcon name="resources" :size="12" /> 管理模组</a>
            <a @click="router.push('/versions')"><PixelIcon name="versions" :size="12" /> 版本管理</a>
          </div>
          <p v-if="game.error && !game.launching" class="error-text small">启动失败：{{ game.error }}</p>
        </div>

        <div class="hero-action">
          <template v-if="runningRun">
            <button class="launch running" @click="router.push('/log')">
              <span class="launch-title">游戏运行中</span>
              <span class="launch-sub">点击查看日志</span>
            </button>
            <a class="stop" @click="game.kill(runningRun.runId)">结束进程</a>
          </template>
          <button v-else class="launch" :disabled="!!game.launching" @click="launch()">
            <PixelIcon v-if="!launchingThis" name="play" :size="22" />
            <span class="launch-text">
              <span class="launch-title">{{ launchingThis ? "启动中…" : accounts.selected ? "开始游戏" : "添加账号" }}</span>
              <span class="launch-sub">{{ launchingThis ? launchStage : current?.id }}</span>
            </span>
          </button>
        </div>
      </template>

      <div v-else class="hero-empty">
        <h2>欢迎来到神权论坛！</h2>
        <p class="muted">还没有安装任何版本。先去下载中心装一个，或者把整合包文件直接拖进窗口。</p>
        <RouterLink to="/download" class="btn primary">去下载中心</RouterLink>
      </div>
    </section>

    <div class="cols">
      <div>
        <section class="box">
          <div class="box-title">📢 公告栏</div>
          <div class="box-body notice">
            <p><span class="tag">置顶</span><b>神权论坛启动器</b>：原版 / 加载器 / 整合包一键安装，资源中心下载时自动检查兼容性，崩溃了版主帮你看。</p>
            <p class="muted">小贴士：把 .mrpack 或整合包 .zip 直接拖进窗口就能导入。</p>
          </div>
        </section>

        <section class="box">
          <div class="box-title">最近玩过 <RouterLink to="/versions" class="extra">全部版本 »</RouterLink></div>
          <div v-if="!recent.length" class="box-body muted">沙发还空着～ 启动一次游戏就会出现在这里。</div>
          <div v-for="(v, i) in recent" :key="v.id" class="floor">
            <span class="floor-no">{{ i + 1 }}#</span>
            <div class="floor-main">
              <b>{{ v.id }}</b>
              <span class="muted small">{{ describe(v) }} · {{ timeAgo(v.lastPlayed) }} · 共 {{ formatDuration(v.playTimeSecs) }}</span>
            </div>
            <button class="btn small" :disabled="!!game.launching || game.running.some((r) => r.versionId === v.id)" @click="launch(v.id)">启动</button>
          </div>
        </section>
      </div>

      <section class="box stats">
        <div class="box-title">我的数据</div>
        <div class="box-body">
          <div class="stat"><span class="stat-num">{{ formatDuration(totalPlay) }}</span><span class="muted">游戏时长</span></div>
          <div class="stat"><span class="stat-num">{{ versions.installed.length }}</span><span class="muted">已装版本</span></div>
          <div class="stat"><span class="stat-num">{{ points }}</span><span class="muted">积分</span></div>
          <div class="group">用户组：<b>神权</b></div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.home { max-width: 1000px; margin: 0 auto; }

/* ---------- 启动卡片 ---------- */
.hero { display: flex; align-items: center; gap: 24px; padding: 22px 24px; }
.hero-main { flex: 1; min-width: 0; }
.pickers { display: flex; flex-direction: column; gap: 10px; }
.picker { display: flex; align-items: center; gap: 12px; }
.picker-label { width: 32px; color: var(--text-muted); }
.picker-select {
  flex: 1;
  max-width: 420px;
  padding: 8px 12px;
  font: inherit;
  font-size: 15px;
  font-weight: bold;
  color: var(--text);
  background: var(--bg-input);
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  cursor: pointer;
}
.picker-select:focus { outline: none; border-color: var(--gold); box-shadow: 0 0 0 2px rgba(224, 164, 25, 0.3); }
.picker-empty { font-weight: bold; }
.hero-meta { margin: 12px 0 0 44px; color: var(--text-muted); }
.quick-actions { display: flex; gap: 16px; margin: 8px 0 0 44px; font-size: 12px; }
.quick-actions a { display: inline-flex; align-items: center; gap: 5px; color: var(--text-muted); }
.quick-actions a:hover { color: var(--link-hover); text-decoration: none; }
.tag.modpack { background: #b5651d; }
.small { font-size: 12px; }

.hero-action { display: flex; flex-direction: column; align-items: center; gap: 6px; }
/* Minecraft 风格的大按钮 */
.launch {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 220px;
  padding: 16px 26px;
  font: inherit;
  color: #fff;
  text-align: left;
  background: linear-gradient(#7cc44e, #4f8a2b);
  border: 3px solid #2f5418;
  border-radius: 6px;
  box-shadow: inset 0 3px 0 rgba(255, 255, 255, 0.35), inset 0 -4px 0 rgba(0, 0, 0, 0.25), 0 4px 12px rgba(47, 84, 24, 0.4);
  cursor: pointer;
  text-shadow: 2px 2px 0 rgba(0, 0, 0, 0.35);
  transition: filter 0.15s, transform 0.08s;
}
.launch:hover:not(:disabled) { filter: brightness(1.08); }
.launch:active:not(:disabled) { transform: translateY(2px); box-shadow: inset 0 2px 0 rgba(255, 255, 255, 0.3), inset 0 -2px 0 rgba(0, 0, 0, 0.25); }
.launch:disabled { filter: saturate(0.6); cursor: progress; }
.launch.running { flex-direction: column; align-items: center; gap: 2px; background: linear-gradient(#5d9fd8, #2f6aa8); border-color: #1d4570; }
.launch-text { display: flex; flex-direction: column; min-width: 0; }
.launch-title { font-size: 22px; font-weight: 900; letter-spacing: 2px; }
.launch-sub { font-size: 12px; opacity: 0.9; max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.stop { font-size: 12px; color: var(--red); }

.hero-empty { flex: 1; text-align: center; padding: 10px 0; }
.hero-empty h2 { color: var(--brown-dark); margin-bottom: 6px; }
.hero-empty .btn { margin-top: 8px; text-decoration: none; }

/* ---------- 下方两栏 ---------- */
.cols { display: grid; grid-template-columns: 1fr 230px; gap: 14px; align-items: start; }
.notice p { margin: 4px 0; line-height: 1.7; }
.floor { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px dashed var(--border); }
.floor:last-child { border-bottom: none; }
.floor-no { color: var(--text-muted); font-family: var(--font-pixel); }
.floor-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.floor-main b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.stats .box-body { display: flex; flex-direction: column; gap: 12px; }
.stat { display: flex; flex-direction: column; }
.stat-num { font-size: 22px; font-weight: 900; color: var(--brown-dark); }
.group b { color: var(--gold); }
</style>
