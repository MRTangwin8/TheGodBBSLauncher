<script setup lang="ts">
import { useLauncher } from "../../composables/useLauncher";
import { kindLabel as accountKind } from "../../stores/accounts";
import { formatDuration, loaderLabel } from "../../utils";
import { useButtons } from "../../buttons";
import QuickButtons from "./QuickButtons.vue";
import { errText } from "../../i18n";

// 经典论坛首页的「快速启动」版块
const { launchLabel, launchClass, launchStyle } = useButtons();
const { selected, byRecent, current, runningRun, launchingThis, launchStage, launch, versions, accounts, game, router } = useLauncher();
</script>

<template>
  <div class="box">
    <div class="box-title">
      🎮 {{ $t("quickLaunch.title") }}
      <RouterLink to="/versions" class="extra">{{ $t("quickLaunch.allVersions") }} »</RouterLink>
    </div>
    <div v-if="!versions.installed.length" class="box-body muted">
      <i18n-t keypath="quickLaunch.empty"><RouterLink to="/download">{{ $t("nav.download") }}</RouterLink></i18n-t>
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
        <option v-for="a in accounts.accounts" :key="a.id" :value="a.id">{{ $t("quickLaunch.accountOption", { name: a.name, kind: accountKind(a.kind) }) }}</option>
      </select>
      <RouterLink v-else to="/account" class="no-account">{{ $t("quickLaunch.addAccount") }} »</RouterLink>

      <template v-if="runningRun">
        <button class="btn go running" @click="router.push('/log')">{{ $t("shell.gameRunning") }}</button>
        <a class="stop" @click="game.kill(runningRun.runId)">{{ $t("quickLaunch.stop") }}</a>
      </template>
      <button v-else class="btn primary go" :class="launchClass" :style="launchStyle" :disabled="!!game.launching" @click="launch()">
        {{ launchingThis ? launchStage : accounts.selected ? `▶ ${launchLabel}` : $t("quickLaunch.addAccount") }}
      </button>
    </div>
    <div v-if="versions.installed.length" class="quick-extra"><QuickButtons /></div>
    <div v-if="current && versions.installed.length" class="meta muted">
      {{ current.loader ? loaderLabel[current.loader] : $t("common.vanilla") }}{{ current.mcVersion && current.mcVersion !== current.id ? ` · MC ${current.mcVersion}` : "" }}
      · {{ $t("quickLaunch.played", { time: formatDuration(current.playTimeSecs) }) }}
      <span v-if="game.error && !game.launching" class="error-text"> · {{ $t("common.launchFailed", { err: errText(game.error) }) }}</span>
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
