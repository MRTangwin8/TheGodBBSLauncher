<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { api } from "../api";
import type { LoaderKind, LoaderVersion, OptiFineVersion } from "../api/types";
import { useVersionsStore } from "../stores/versions";
import { defaultVersionName, loaderLabel } from "../utils";
import { errText, t } from "../i18n";

const props = defineProps<{ mc: string }>();
const emit = defineEmits<{ close: [] }>();
const versions = useVersionsStore();

// OptiFine 不是模组加载器：单独装成「原版 + OptiFine」，或者在选 Forge 时顺带作为模组装上
type Kind = LoaderKind | "optifine" | null;
const kinds: Kind[] = [null, "fabric", "quilt", "forge", "neoforge", "optifine"];

const kind = ref<Kind>(null);
const list = ref<LoaderVersion[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selected = ref<string>("");
const showUnstable = ref(false);

// 每种加载器的版本列表缓存，切回来不用重新请求
const cache = new Map<LoaderKind, LoaderVersion[]>();

// OptiFine 版本列表（单独安装和装进 Forge 共用）
const ofList = ref<OptiFineVersion[]>([]);
const ofLoaded = ref(false);
const ofLoading = ref(false);
const ofError = ref<string | null>(null);
const ofSelected = ref("");
const withOptifine = ref(false);
const ofKey = (v: OptiFineVersion) => `${v.type}_${v.patch}`;
async function loadOptifine() {
  if (ofLoaded.value || ofLoading.value) return;
  ofLoading.value = true;
  ofError.value = null;
  try {
    ofList.value = await api.listOptifine(props.mc);
    ofLoaded.value = true;
  } catch (e) {
    ofError.value = errText(e);
  } finally {
    ofLoading.value = false;
  }
}
/** 当前场景能选的 OptiFine：装进 Forge 时只要标了 Forge 兼容的 */
const ofShown = computed(() => {
  const base = kind.value === "forge" ? ofList.value.filter((v) => v.forge) : ofList.value;
  const stable = base.filter((v) => !v.preview);
  return showUnstable.value || !stable.length ? base : stable;
});
const ofPicked = computed(() => ofList.value.find((v) => ofKey(v) === ofSelected.value) ?? null);
watch([ofShown, withOptifine, kind], () => {
  if (!ofShown.value.some((v) => ofKey(v) === ofSelected.value)) ofSelected.value = ofShown.value[0] ? ofKey(ofShown.value[0]) : "";
});
watch(withOptifine, (on) => on && loadOptifine());

watch(kind, async (k) => {
  error.value = null;
  list.value = [];
  selected.value = "";
  if (k === "optifine") {
    await loadOptifine();
    return;
  }
  if (!k) return;
  if (cache.has(k)) {
    list.value = cache.get(k)!;
  } else {
    loading.value = true;
    try {
      list.value = await api.listLoaderVersions(k, props.mc);
      cache.set(k, list.value);
    } catch (e) {
      error.value = errText(e);
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

// 版本名：默认带 MC 版本，用户改过就不再跟着变
const name = ref("");
const nameTouched = ref(false);
watch([kind, selected, ofSelected, withOptifine], () => {
  if (nameTouched.value) return;
  if (kind.value === "optifine") name.value = ofSelected.value ? `${props.mc}-OptiFine_${ofSelected.value}` : "";
  else if (kind.value && selected.value) name.value = defaultVersionName(props.mc, kind.value, selected.value) + (kind.value === "forge" && withOptifine.value ? "-OptiFine" : "");
  else name.value = "";
});
const nameError = computed(() => {
  const n = name.value.trim();
  if (!kind.value) return null;
  if (!n) return t("install.nameEmpty");
  if (/[\\/:*?"<>|]/.test(n)) return t("install.nameChars", { chars: '\\ / : * ? " < > |' });
  if (versions.installed.some((v) => v.id === n)) return t("install.nameTaken");
  return null;
});

const canInstall = computed(() => {
  if (!kind.value) return true;
  if (nameError.value) return false;
  if (kind.value === "optifine") return !!ofPicked.value;
  return !!selected.value && (!withOptifine.value || kind.value !== "forge" || !!ofPicked.value);
});
const hasName = computed(() => (kind.value === "optifine" ? !!ofSelected.value : !!(kind.value && selected.value)));

function confirm() {
  if (!kind.value) {
    versions.install(props.mc);
  } else if (kind.value === "optifine") {
    if (ofPicked.value) versions.installOptifine(props.mc, ofPicked.value, null, name.value.trim());
  } else if (kind.value === "forge" && withOptifine.value && ofPicked.value) {
    const v = list.value.find((x) => x.id === selected.value);
    if (v) versions.installOptifine(props.mc, ofPicked.value, v, name.value.trim());
  } else {
    const v = list.value.find((x) => x.id === selected.value);
    if (v) versions.installLoader(props.mc, kind.value, v, name.value.trim());
  }
  emit("close");
}
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog box">
      <div class="box-title">
        {{ $t("install.title", { mc }) }}
        <a class="extra close" @click="emit('close')">×</a>
      </div>
      <div class="box-body">
        <div class="kinds">
          <label v-for="k in kinds" :key="k ?? 'none'" class="kind" :class="{ active: kind === k }">
            <input v-model="kind" type="radio" :value="k" />
            <b>{{ k ? loaderLabel[k] : $t("common.vanilla") }}</b>
            <span class="muted">{{ $t(`install.blurb.${k ?? "none"}`) }}</span>
          </label>
        </div>

        <div v-if="kind === 'optifine'" class="pick">
          <div v-if="ofLoading" class="muted">{{ $t("install.loading", { loader: "OptiFine" }) }}</div>
          <div v-else-if="ofError" class="error-text">{{ ofError }}</div>
          <div v-else-if="!ofList.length" class="mod-tip">{{ $t("common.modTip") }}{{ $t("install.unsupported", { loader: "OptiFine", mc }) }}</div>
          <template v-else>
            <select v-model="ofSelected" class="input">
              <option v-for="v in ofShown" :key="ofKey(v)" :value="ofKey(v)">{{ v.type }} {{ v.patch }}{{ v.preview ? "" : " ★" }}</option>
            </select>
            <label class="check"><input v-model="showUnstable" type="checkbox" /> {{ $t("install.showUnstable") }}</label>
          </template>
          <p class="muted small full">{{ $t("install.optifineNote") }}</p>
        </div>

        <div v-else-if="kind" class="pick">
          <div v-if="loading" class="muted">{{ $t("install.loading", { loader: loaderLabel[kind] }) }}</div>
          <div v-else-if="error" class="error-text">{{ error }}</div>
          <div v-else-if="!list.length" class="mod-tip">{{ $t("common.modTip") }}{{ $t("install.unsupported", { loader: loaderLabel[kind], mc }) }}</div>
          <template v-else>
            <select v-model="selected" class="input">
              <option v-for="v in shown" :key="v.id" :value="v.id">{{ v.version }}{{ v.stable ? " ★" : "" }}</option>
            </select>
            <label class="check"><input v-model="showUnstable" type="checkbox" /> {{ $t("install.showUnstable") }}</label>
            <span class="muted">{{ $t("install.count", { n: list.length }) }}</span>
          </template>
        </div>

        <div v-if="kind === 'forge' && selected" class="pick">
          <label class="check"><input v-model="withOptifine" type="checkbox" /> {{ $t("install.withOptifine") }}</label>
          <template v-if="withOptifine">
            <span v-if="ofLoading" class="muted">{{ $t("install.loading", { loader: "OptiFine" }) }}</span>
            <span v-else-if="ofError" class="error-text">{{ ofError }}</span>
            <span v-else-if="!ofShown.length" class="muted">{{ $t("install.optifineNoForge", { mc }) }}</span>
            <select v-else v-model="ofSelected" class="input">
              <option v-for="v in ofShown" :key="ofKey(v)" :value="ofKey(v)">{{ v.type }} {{ v.patch }} · {{ $t("install.optifineForge", { forge: v.forge }) }}</option>
            </select>
          </template>
        </div>

        <div v-if="hasName" class="name-row">
          <label>{{ $t("install.name") }}</label>
          <input v-model="name" class="input" @input="nameTouched = true" />
          <span v-if="nameError" class="error-text">{{ nameError }}</span>
          <span v-else class="muted">{{ $t("install.nameHint") }}</span>
        </div>

        <div class="actions">
          <button class="btn" @click="emit('close')">{{ $t("common.cancel") }}</button>
          <button class="btn primary" :disabled="!canInstall" @click="confirm">
            {{ kind ? $t("install.installLoader", { mc, loader: loaderLabel[kind] + (kind === "forge" && withOptifine ? " + OptiFine" : "") }) : $t("install.installVanilla", { mc }) }}
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
.pick .full { width: 100%; margin: 0; }
.small { font-size: 12px; }
.check { display: flex; align-items: center; gap: 4px; cursor: pointer; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.name-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 12px; }
.name-row label { color: var(--text-muted); }
.name-row .input { flex: 1; min-width: 240px; font-family: var(--font-pixel); }
.name-row span { width: 100%; padding-left: 64px; font-size: 12px; }
</style>
