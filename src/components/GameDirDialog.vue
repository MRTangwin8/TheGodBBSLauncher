<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, inTauri, onMigrateProgress } from "../api";
import type { DirStats, GameDirInfo, MigrateProgress, MigrateReport } from "../api/types";
import { useGameStore } from "../stores/game";
import { useSettingsStore } from "../stores/settings";
import { formatBytes } from "../utils";
import { errText, t } from "../i18n";

// 更改游戏目录：默认把现有内容搬过去；也可以只改位置（旧数据留在原处）。
// 后端保证：新位置确认没问题之前，旧数据一个字节都不动。
// fromLegacy：把老版本默认位置（AppData）里留下的数据搬进现在的游戏目录，目标固定
const props = defineProps<{ info: GameDirInfo; initial?: string; fromLegacy?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const game = useGameStore();
const settings = useSettingsStore();

const source = computed(() => (props.fromLegacy ? props.info.legacy : props.info.current));
const target = ref(props.fromLegacy ? props.info.current : (props.initial ?? props.info.recommended));
const moveFiles = ref(true);
const mine = ref<DirStats | null>(null);
const dest = ref<DirStats | null>(null);
const busy = ref(false);
const progress = ref<MigrateProgress | null>(null);
const report = ref<MigrateReport | null>(null);
const error = ref("");

let unlisten: (() => void) | undefined;
onUnmounted(() => unlisten?.());

const percent = computed(() => {
  const p = progress.value;
  return p?.totalBytes ? Math.min(100, Math.round((p.bytes / p.totalBytes) * 100)) : 0;
});
const sameAsCurrent = computed(() => norm(target.value) === norm(source.value));
const hasOld = computed(() => !!mine.value?.exists && !mine.value.empty);
const blocked = computed(() => {
  if (game.running.length) return t("gameDir.gameRunning");
  if (!target.value.trim()) return t("gameDir.pickTarget");
  if (sameAsCurrent.value) return t("gameDir.same");
  return "";
});

function norm(p: string) {
  return p.replace(/[\\/]+$/, "").replace(/\\/g, "/").toLowerCase();
}

async function refresh() {
  mine.value = await api.dirStats(source.value).catch(() => null);
  dest.value = target.value.trim() ? await api.dirStats(target.value.trim()).catch(() => null) : null;
}

onMounted(refresh);

async function pick() {
  if (!inTauri) {
    error.value = t("gameDir.previewOnly");
    return;
  }
  const chosen = await open({ multiple: false, directory: true });
  if (typeof chosen === "string") {
    target.value = chosen;
    await refresh();
  }
}

async function run() {
  error.value = "";
  busy.value = true;
  progress.value = null;
  unlisten = await onMigrateProgress((p) => (progress.value = p));
  try {
    report.value = await api.gameDirMigrate(target.value.trim(), props.fromLegacy || (moveFiles.value && hasOld.value), props.fromLegacy);
    await settings.load();
  } catch (e) {
    error.value = errText(e);
  } finally {
    busy.value = false;
    unlisten?.();
    unlisten = undefined;
  }
}

function dismiss() {
  if (!busy.value) emit("close");
}
</script>

<template>
  <div class="mask" @click.self="dismiss">
    <div class="dialog box" role="dialog" aria-modal="true" :aria-label="$t('gameDir.title')">
      <div class="box-title">
        {{ fromLegacy ? $t("gameDir.legacyTitle") : $t("gameDir.title") }}
        <a v-if="!busy" class="extra close" @click="dismiss">×</a>
      </div>
      <div class="box-body">
        <template v-if="report">
          <p class="ok-text">
            <template v-if="report.moved">{{ $t("gameDir.migrated") }}<template v-if="report.renamed">{{ $t("gameDir.renamed") }}</template><template v-else>{{ $t("gameDir.copied", { n: report.files, size: formatBytes(report.bytes) }) }}</template></template>
            <template v-else>{{ $t("gameDir.changedOnly") }}</template>
          </p>
          <p class="muted small break">{{ $t("gameDir.now", { path: target }) }}</p>
          <div v-if="report.leftover.length" class="warn-box">
            <p class="small">{{ $t("gameDir.leftover") }}</p>
            <ul class="small"><li v-for="p in report.leftover" :key="p" class="break">{{ p }}</li></ul>
          </div>
        </template>
        <template v-else>
          <div class="path-row">
            <span class="label muted">{{ fromLegacy ? $t("gameDir.oldLabel") : $t("gameDir.nowLabel") }}</span>
            <span class="break">{{ source }}</span>
            <span v-if="mine?.exists" class="muted small">{{ $t("gameDir.stats", { n: mine.files, size: formatBytes(mine.bytes) }) }}</span>
          </div>
          <div class="path-row">
            <span class="label muted">{{ $t("gameDir.toLabel") }}</span>
            <span v-if="fromLegacy" class="break">{{ target }}<span class="muted small">{{ $t("gameDir.currentDir") }}</span></span>
            <template v-else>
              <input v-model="target" class="input grow" :disabled="busy" @change="refresh" />
              <button class="btn small" :disabled="busy" @click="pick">{{ $t("common.browse") }}</button>
            </template>
          </div>
          <p v-if="!fromLegacy && target !== info.recommended" class="small">
            <a @click="target = info.recommended; refresh()">{{ $t("gameDir.useRecommended") }}</a>
          </p>
          <p v-if="dest?.exists && !dest.empty && !moveFiles" class="muted small">{{ $t("gameDir.destHasFiles", { n: dest.files }) }}</p>

          <p v-if="fromLegacy" class="muted small">{{ $t("gameDir.legacyHint") }}</p>
          <div v-if="hasOld && !fromLegacy" class="choice">
            <label><input v-model="moveFiles" type="radio" :value="true" :disabled="busy" /> {{ $t("gameDir.move") }}</label>
            <label><input v-model="moveFiles" type="radio" :value="false" :disabled="busy" /> {{ $t("gameDir.noMove") }}</label>
          </div>
          <p v-if="hasOld && (moveFiles || fromLegacy)" class="muted small">
            {{ $t("gameDir.moveHint") }}
          </p>

          <div v-if="busy" class="progress">
            <div class="xp-bar"><div :style="{ width: percent + '%' }" /></div>
            <p class="muted small">
              {{ $t(`gameDir.stage.${progress?.stage ?? "scanning"}`) }}
              <template v-if="progress?.totalBytes">{{ formatBytes(progress.bytes) }} / {{ formatBytes(progress.totalBytes) }} ({{ percent }}%)</template>
            </p>
          </div>
          <p v-if="blocked && !busy" class="muted small">{{ blocked }}</p>
          <p v-if="error" class="error-text small">{{ error }}</p>
        </template>
      </div>
      <div class="actions">
        <template v-if="report">
          <span class="grow" />
          <button class="btn primary" @click="emit('close')">{{ $t("common.ok") }}</button>
        </template>
        <template v-else>
          <span class="grow" />
          <button v-if="busy" class="btn" @click="api.gameDirMigrateCancel()">{{ $t("gameDir.cancel") }}</button>
          <button v-else class="btn" @click="dismiss">{{ $t("common.close") }}</button>
          <button class="btn primary" :disabled="busy || !!blocked" @click="run">{{ busy ? $t("gameDir.migrating") : fromLegacy || (moveFiles && hasOld) ? $t("gameDir.start") : $t("common.confirm") }}</button>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 30; }
.dialog { width: 560px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.path-row { display: flex; align-items: center; gap: 8px; margin: 6px 0; }
.label { flex: none; width: 32px; }
.grow { flex: 1; min-width: 0; }
.break { word-break: break-all; }
.choice { display: flex; flex-direction: column; gap: 6px; margin: 10px 0 4px; }
.choice label { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.progress { margin: 8px 0; }
.small { font-size: 12px; margin: 4px 0; }
.ok-text { color: var(--grass); }
.warn-box { margin-top: 8px; padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--bg-box-alt); }
.warn-box ul { margin: 4px 0 0; padding-left: 18px; }
.actions { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--border); }
</style>
