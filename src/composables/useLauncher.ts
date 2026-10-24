// 首页启动用的公共逻辑：默认选中的版本、启动状态、启动动作。两种首页共用。
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useVersionsStore } from "../stores/versions";
import { useAccountsStore } from "../stores/accounts";
import { useGameStore } from "../stores/game";
import { useSettingsStore } from "../stores/settings";

export function useLauncher() {
  const versions = useVersionsStore();
  const accounts = useAccountsStore();
  const game = useGameStore();
  const settings = useSettingsStore();
  const router = useRouter();

  const selected = ref("");

  /** 最近玩过的在前，没玩过的按发布时间 */
  const byRecent = computed(() =>
    [...versions.installed].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || (b.releaseTime ?? "").localeCompare(a.releaseTime ?? "")),
  );
  const current = computed(() => versions.installed.find((v) => v.id === selected.value) ?? null);

  // 默认选上次启动的版本
  watch(
    () => [versions.installed.length, settings.settings?.lastVersion] as const,
    () => {
      if (selected.value && versions.installed.some((v) => v.id === selected.value)) return;
      const last = settings.settings?.lastVersion;
      selected.value = (last && versions.installed.some((v) => v.id === last) ? last : byRecent.value[0]?.id) ?? "";
    },
    { immediate: true },
  );

  const runningRun = computed(() => game.running.find((r) => r.versionId === selected.value));
  const launchingThis = computed(() => !!selected.value && game.launching === selected.value);
  const launchStage = computed(() => {
    const j = selected.value ? versions.jobs[selected.value] : undefined;
    if (!j || j.kind !== "launch" || j.status !== "running") return "正在准备…";
    if (j.stage === "java") return "正在下载 Java…";
    const p = j.progress;
    return p && p.totalFiles ? `补全文件 ${p.finishedFiles}/${p.totalFiles}` : "检查游戏文件…";
  });

  async function launch(id = selected.value) {
    if (!id) return;
    selected.value = id;
    if (!accounts.selected) {
      router.push("/account");
      return;
    }
    if (await game.launch(id)) router.push("/log");
  }

  return { selected, byRecent, current, runningRun, launchingThis, launchStage, launch, versions, accounts, game, router };
}
