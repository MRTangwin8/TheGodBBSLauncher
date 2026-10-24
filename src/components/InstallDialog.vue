<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api } from "../api";
import type { LoaderKind, LoaderVersion } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { loaderLabel } from "../utils";

const props = defineProps<{ mc: string }>();
const emit = defineEmits<{ close: [] }>();
const versions = useVersionsStore();

const kinds: (LoaderKind | null)[] = [null, "fabric", "quilt", "forge", "neoforge"];
const blurb: Record<string, string> = {
  none: "纯净原版，不装任何加载器",
  fabric: "轻量、更新快，适合新版本和性能优化模组",
  quilt: "Fabric 的分支，兼容大部分 Fabric 模组",
  forge: "老牌加载器，模组最多，旧版本首选",
  neoforge: "Forge 的社区分支，1.20.2 以后的主流选择",
};

const kind = ref<LoaderKind | null>(null);
const list = ref<LoaderVersion[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selected = ref<string>("");
const showUnstable = ref(false);

// 每种加载器的版本列表缓存，切回来不用重新请求
const cache = new Map<LoaderKind, LoaderVersion[]>();

watch(kind, async (k) => {
  error.value = null;
  list.value = [];
  selected.value = "";
  if (!k) return;
  if (cache.has(k)) {
    list.value = cache.get(k)!;
  } else {
    loading.value = true;
    try {
      list.value = await api.listLoaderVersions(k, props.mc);
      cache.set(k, list.value);
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }
  selected.value = (list.value.find((v) => v.stable) ?? list.value[0])?.id ?? "";
});

const shown = computed(() => {
  const l = showUnstable.value ? list.value : list.value.filter((v) => v.stable);
  // 全是测试版时（比如刚出的 MC 版本）直接全部显示
  return l.length ? l : list.value;
});

const canInstall = computed(() => !kind.value || !!selected.value);

function confirm() {
  if (!kind.value) {
    versions.install(props.mc);
  } else {
    const v = list.value.find((x) => x.id === selected.value);
    if (v) versions.installLoader(props.mc, kind.value, v);
  }
  emit("close");
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        发表新主题 · 安装 Minecraft {{ mc }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body">
        <div class="kinds">
          <label v-for="k in kinds" :key="k ?? 'none'" class="kind" :class="{ active: kind === k }">
            <input v-model="kind" type="radio" :value="k" />
            <b>{{ k ? loaderLabel[k] : "原版" }}</b>
            <span class="muted">{{ blurb[k ?? "none"] }}</span>
          </label>
        </div>

        <div v-if="kind" class="pick">
          <div v-if="loading" class="muted">正在获取 {{ loaderLabel[kind] }} 版本列表…</div>
          <div v-else-if="error" class="error-text">{{ error }}</div>
          <div v-else-if="!list.length" class="mod-tip">【版主提示】{{ loaderLabel[kind] }} 还不支持 Minecraft {{ mc }}</div>
          <template v-else>
            <select v-model="selected" class="input">
              <option v-for="v in shown" :key="v.id" :value="v.id">{{ v.version }}{{ v.stable ? " ★" : "" }}</option>
            </select>
            <label class="check"><input v-model="showUnstable" type="checkbox" /> 显示测试版</label>
            <span class="muted">共 {{ list.length }} 个版本，★ 为稳定/推荐版</span>
          </template>
        </div>

        <div class="actions">
          <button class="btn" @click="emit('close')">取消</button>
          <button class="btn primary" :disabled="!canInstall" @click="confirm">
            {{ kind ? `安装 ${mc} + ${loaderLabel[kind]}` : `安装原版 ${mc}` }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: rgba(40, 28, 14, 0.45); display: flex; align-items: center; justify-content: center; z-index: 20; }
.dialog { width: 560px; max-width: calc(100vw - 32px); margin: 0; }
.close { font-size: 18px; line-height: 1; color: var(--text-muted); }
.kinds { display: grid; gap: 6px; }
.kind { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border: 1px solid var(--border); border-radius: var(--radius); cursor: pointer; background: var(--bg-box-alt); }
.kind.active { border-color: var(--grass); background: rgba(94, 156, 53, 0.1); }
.kind b { width: 72px; }
.kind .muted { font-size: 12px; }
.pick { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-top: 12px; padding-top: 12px; border-top: 1px dashed var(--border); }
.pick select { min-width: 220px; }
.check { display: flex; align-items: center; gap: 4px; cursor: pointer; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
</style>
