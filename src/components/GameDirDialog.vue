<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, inTauri, onMigrateProgress } from "../api";
import type { DirStats, GameDirInfo, MigrateProgress, MigrateReport } from "../api/types";
import { useGameStore } from "../stores/game";
import { useSettingsStore } from "../stores/settings";
import { formatBytes } from "../utils";

// 更改游戏目录：默认把现有内容搬过去；也可以只改位置（旧数据留在原处）。
// 后端保证：新位置确认没问题之前，旧数据一个字节都不动。
const props = defineProps<{ info: GameDirInfo; initial?: string }>();
const emit = defineEmits<{ close: [] }>();
const game = useGameStore();
const settings = useSettingsStore();

const target = ref(props.initial ?? props.info.recommended);
const moveFiles = ref(true);
const mine = ref<DirStats | null>(null);
const dest = ref<DirStats | null>(null);
const busy = ref(false);
const progress = ref<MigrateProgress | null>(null);
const report = ref<MigrateReport | null>(null);
const error = ref("");

let unlisten: (() => void) | undefined;
onUnmounted(() => unlisten?.());

const stageText: Record<string, string> = {
  scanning: "正在统计文件…",
  moving: "正在移动…",
  copying: "正在复制…",
  verifying: "正在核对复制结果…",
  cleaning: "正在清理旧目录…",
};
const percent = computed(() => {
  const p = progress.value;
  return p?.totalBytes ? Math.min(100, Math.round((p.bytes / p.totalBytes) * 100)) : 0;
});
const sameAsCurrent = computed(() => norm(target.value) === norm(props.info.current));
const hasOld = computed(() => !!mine.value?.exists && !mine.value.empty);
const blocked = computed(() => {
  if (game.running.length) return "游戏还在运行，先退出游戏再更改目录。";
  if (!target.value.trim()) return "请选择新位置。";
  if (sameAsCurrent.value) return "新位置和现在的一样。";
  if (moveFiles.value && hasOld.value && dest.value?.exists && !dest.value.empty) return "新位置不是空文件夹。请选一个空文件夹（或新建一个），或者改选「只改位置」。";
  return "";
});

function norm(p: string) {
  return p.replace(/[\\/]+$/, "").replace(/\\/g, "/").toLowerCase();
}

async function refresh() {
  mine.value = await api.dirStats(props.info.current).catch(() => null);
  dest.value = target.value.trim() ? await api.dirStats(target.value.trim()).catch(() => null) : null;
}

onMounted(refresh);

async function pick() {
  if (!inTauri) {
    error.value = "浏览器预览模式不能选择文件夹";
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
    report.value = await api.gameDirMigrate(target.value.trim(), moveFiles.value && hasOld.value);
    await settings.load();
  } catch (e) {
    error.value = `${e}`;
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
    <div class="dialog box" role="dialog" aria-modal="true" aria-label="更改游戏目录">
      <div class="box-title">
        更改游戏目录
        <a v-if="!busy" class="extra close" @click="dismiss">×</a>
      </div>
      <div class="box-body">
        <template v-if="report">
          <p class="ok-text">
            <template v-if="report.moved">已迁移完成。<template v-if="report.renamed">同一个磁盘，直接移动，没有复制。</template><template v-else>共 {{ report.files }} 个文件，{{ formatBytes(report.bytes) }}。</template></template>
            <template v-else>已改到新位置，旧文件原样留在原处。</template>
          </p>
          <p class="muted small break">现在的游戏目录：{{ target }}</p>
          <div v-if="report.leftover.length" class="warn-box">
            <p class="small">下面这些东西已经复制成功，但旧目录里没能删掉（可能被占用或杀毒软件在扫描），请自己删除：</p>
            <ul class="small"><li v-for="p in report.leftover" :key="p" class="break">{{ p }}</li></ul>
          </div>
        </template>
        <template v-else>
          <div class="path-row">
            <span class="label muted">现在</span>
            <span class="break">{{ info.current }}</span>
            <span v-if="mine?.exists" class="muted small">（{{ mine.files }} 个文件，{{ formatBytes(mine.bytes) }}）</span>
          </div>
          <div class="path-row">
            <span class="label muted">改到</span>
            <input v-model="target" class="input grow" :disabled="busy" @change="refresh" />
            <button class="btn small" :disabled="busy" @click="pick">选择…</button>
          </div>
          <p v-if="target !== info.recommended" class="small">
            <a @click="target = info.recommended; refresh()">用推荐位置（启动器旁边）</a>
          </p>
          <p v-if="dest?.exists && !dest.empty && !moveFiles" class="muted small">这个文件夹里已经有 {{ dest.files }} 个文件，启动器会直接使用它。</p>

          <div v-if="hasOld" class="choice">
            <label><input v-model="moveFiles" type="radio" :value="true" :disabled="busy" /> 把现有内容搬过去（推荐）</label>
            <label><input v-model="moveFiles" type="radio" :value="false" :disabled="busy" /> 只改位置，不搬文件（旧文件留在原处，新位置从头开始）</label>
          </div>
          <p v-if="hasOld && moveFiles" class="muted small">
            同一个磁盘上会直接移动，几 GB 也是瞬间；跨磁盘要复制，可能要几分钟，期间不能启动游戏和下载。复制完核对无误才会删旧文件，中途取消或出错，旧文件不会受影响。
          </p>

          <div v-if="busy" class="progress">
            <div class="xp-bar"><div :style="{ width: percent + '%' }" /></div>
            <p class="muted small">
              {{ stageText[progress?.stage ?? "scanning"] }}
              <template v-if="progress?.totalBytes">{{ formatBytes(progress.bytes) }} / {{ formatBytes(progress.totalBytes) }}（{{ percent }}%）</template>
            </p>
          </div>
          <p v-if="blocked && !busy" class="muted small">{{ blocked }}</p>
          <p v-if="error" class="error-text small">{{ error }}</p>
        </template>
      </div>
      <div class="actions">
        <template v-if="report">
          <span class="grow" />
          <button class="btn primary" @click="emit('close')">好的</button>
        </template>
        <template v-else>
          <span class="grow" />
          <button v-if="busy" class="btn" @click="api.gameDirMigrateCancel()">取消迁移</button>
          <button v-else class="btn" @click="dismiss">关闭</button>
          <button class="btn primary" :disabled="busy || !!blocked" @click="run">{{ busy ? "迁移中…" : moveFiles && hasOld ? "开始迁移" : "确定" }}</button>
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
