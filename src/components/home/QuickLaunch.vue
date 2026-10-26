<script setup lang="ts">
import { useLauncher } from "../../composables/useLauncher";
import { kindLabel as accountKind } from "../../stores/accounts";
import { formatDuration, loaderLabel } from "../../utils";
import { useButtons } from "../../buttons";
import QuickButtons from "./QuickButtons.vue";

// 经典论坛首页的「快速启动」版块
const { launchLabel, launchClass, launchStyle } = useButtons();
const { selected, byRecent, current, runningRun, launchingThis, launchStage, launch, versions, accounts, game, router } = useLauncher();
</script>

<template>
  <div class="box">
    <div class="box-title">
      🎮 快速启动
      <RouterLink to="/versions" class="extra">全部版本 »</RouterLink>
    </div>
    <div v-if="!versions.installed.length" class="box-body muted">
      还没有安装任何版本，去 <RouterLink to="/download">下载中心</RouterLink> 装一个，或者把整合包拖进窗口。
    </div>
    <div v-else class="box-body quick">
      <select v-model="selected" class="input pick">
        <option v-for="v in byRecent" :key="v.id" :value="v.id">{{ v.id }}</option>
      </select>
      <select
        v-if="accounts.accounts.length"
        :value="accounts.selectedId ?? ''"
        class="input pick account"
        @change="accounts.select(($event.target as HTMLSelectElement).value)"
      >
        <option v-for="a in accounts.accounts" :key="a.id" :value="a.id">{{ a.name }}（{{ accountKind[a.kind] }}）</option>
      </select>
      <RouterLink v-else to="/account" class="no-account">添加账号 »</RouterLink>

      <template v-if="runningRun">
        <button class="btn go running" @click="router.push('/log')">游戏运行中</button>
        <a class="stop" @click="game.kill(runningRun.runId)">结束</a>
      </template>
      <button v-else class="btn primary go" :class="launchClass" :style="launchStyle" :disabled="!!game.launching" @click="launch()">
        {{ launchingThis ? launchStage : accounts.selected ? `▶ ${launchLabel}` : "添加账号" }}
      </button>
    </div>
    <div v-if="versions.installed.length" class="quick-extra"><QuickButtons /></div>
    <div v-if="current && versions.installed.length" class="meta muted">
      {{ current.loader ? loaderLabel[current.loader] : "原版" }}{{ current.mcVersion && current.mcVersion !== current.id ? ` · MC ${current.mcVersion}` : "" }}
      · 已玩 {{ formatDuration(current.playTimeSecs) }}
      <span v-if="game.error && !game.launching" class="error-text"> · 启动失败：{{ game.error }}</span>
    </div>
  </div>
</template>

<style scoped>
.quick { display: flex; align-items: center; gap: 8px; padding-bottom: 6px; }
.pick { flex: 1; min-width: 0; font-weight: bold; }
.pick.account { flex: 0.7; }
.no-account { white-space: nowrap; }
.go { padding: 7px 22px; font-size: 14px; font-weight: bold; min-width: 130px; }
.go.running { color: #fff; background: linear-gradient(#5d9fd8, #2f6aa8); border-color: #1d4570; }
.stop { color: var(--red); font-size: 12px; white-space: nowrap; }
.meta { padding: 0 14px 10px; font-size: 12px; }
.quick-extra { padding: 0 14px 8px; }
.quick-extra :deep(.quick-buttons) { margin-top: 2px; }
</style>
