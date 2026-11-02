<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { useVersionsStore } from "../stores/versions";
import { api } from "../api";
import { ref } from "vue";
import ModpackExportDialog from "../components/ModpackExportDialog.vue";
import { useGameStore } from "../stores/game";
import { useAccountsStore } from "../stores/accounts";
import { formatDate, kindLabel, loaderLabel } from "../utils";
import { errText, t } from "../i18n";

const versions = useVersionsStore();
const game = useGameStore();
const accounts = useAccountsStore();
const router = useRouter();
const exportFor = ref<string | null>(null);
onMounted(() => versions.loadInstalled());

async function rename(id: string) {
  const next = prompt(t("versions.renamePrompt", { id }), id)?.trim();
  if (!next || next === id) return;
  try {
    await api.renameVersion(id, next);
    await versions.loadInstalled();
  } catch (e) {
    alert(errText(e));
  }
}

async function launch(id: string) {
  if (!accounts.selected) {
    router.push("/account");
    return;
  }
  if (await game.launch(id)) router.push("/log");
}
</script>

<template>
  <div class="box">
    <div class="box-title">
      {{ $t("nav.versions") }}
      <span class="extra">
        <template v-if="accounts.selected"><i18n-t keypath="versions.currentAccount"><b>{{ accounts.selected.name }}</b></i18n-t> · </template>
        <a @click="versions.loadInstalled()">{{ $t("common.refresh") }}</a>
      </span>
    </div>
    <div v-if="game.error" class="box-body">
      <div class="mod-tip">{{ $t("common.modTip") }}{{ $t("common.launchFailed", { err: errText(game.error) }) }}</div>
    </div>
    <table class="thread-table">
      <thead>
        <tr><th>{{ $t("common.thread") }}</th><th>{{ $t("versions.gameVersion") }}</th><th class="num">{{ $t("common.released") }}</th><th class="num">{{ $t("common.actions") }}</th></tr>
      </thead>
      <tbody>
        <tr v-for="v in versions.installed" :key="v.id">
          <td>
            <span v-if="v.loader" class="tag" :class="'ld-' + v.loader">{{ loaderLabel[v.loader] }}</span>
            <span v-else class="tag" :class="v.kind">{{ kindLabel(v.kind) }}</span>
            <RouterLink :to="`/versions/${encodeURIComponent(v.id)}`" class="name" :title="$t('versions.manageTip')"><b>{{ v.id }}</b></RouterLink>
            <span v-if="!v.hasJar" class="error-text"> {{ $t("versions.noJar") }}</span>
            <span v-if="game.running.some((r) => r.versionId === v.id)" class="playing">● {{ $t("versions.playing") }}</span>
          </td>
          <td class="muted">{{ v.mcVersion ?? v.inheritsFrom ?? "-" }}</td>
          <td class="num">{{ formatDate(v.releaseTime) }}</td>
          <td class="num actions">
            <button
              class="btn small primary"
              :disabled="!!game.launching"
              :title="accounts.selected ? '' : $t('versions.addAccountFirst')"
              @click="launch(v.id)"
            >
              {{ game.launching === v.id ? $t("launchHome.launching") : $t("common.launch") }}
            </button>
            <button
              class="btn small"
              :disabled="!!v.inheritsFrom || versions.jobs[v.id]?.status === 'running'"
              @click="versions.install(v.id)"
            >
              {{ $t("common.completeFiles") }}
            </button>
            <RouterLink :to="`/versions/${encodeURIComponent(v.id)}`" class="btn small">{{ $t("versions.manage") }}</RouterLink>
            <button class="btn small" :disabled="game.running.some((r) => r.versionId === v.id)" @click="rename(v.id)">{{ $t("versions.rename") }}</button>
            <button class="btn small" @click="exportFor = v.id">{{ $t("versions.export") }}</button>
          </td>
        </tr>
        <tr v-if="!versions.installed.length">
          <td colspan="4" class="muted"><i18n-t keypath="versions.empty"><RouterLink to="/download">{{ $t("nav.download") }}</RouterLink></i18n-t></td>
        </tr>
      </tbody>
    </table>
  </div>
  <ModpackExportDialog v-if="exportFor" :version-id="exportFor" @close="exportFor = null" />
</template>

<style scoped>
.actions { display: flex; gap: 6px; justify-content: flex-end; }
.playing { color: var(--grass); font-size: 12px; margin-left: 6px; }
.name { color: var(--text); }
.tag.ld-fabric { background: #b39b72; }
.tag.ld-quilt { background: #8e5fb4; }
.tag.ld-forge { background: #3e4b61; }
.tag.ld-neoforge { background: #d2691e; }
</style>
