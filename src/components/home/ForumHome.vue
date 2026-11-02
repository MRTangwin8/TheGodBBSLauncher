<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useVersionsStore } from "../../stores/versions";
import { useAccountsStore } from "../../stores/accounts";
import UserCard from "../UserCard.vue";
import QuickLaunch from "./QuickLaunch.vue";
import NoticeBoard from "./NoticeBoard.vue";
import { formatDate, kindLabel } from "../../utils";
import { t } from "../../i18n";

// 第一版的论坛首页：公告栏 + 版块列表 + 最新帖子 + 个人信息
const versions = useVersionsStore();
const accounts = useAccountsStore();
onMounted(() => versions.manifest || versions.loadManifest());

const boards = computed(() => [
  {
    to: "/download",
    icon: "⬇",
    name: t("nav.download"),
    desc: t("forumHome.downloadDesc"),
    stat: versions.manifest ? t("forumHome.versionCount", { n: versions.manifest.versions.length }) : "…",
    last: versions.manifest ? t("forumHome.latestRelease", { v: versions.manifest.latest.release }) : "",
  },
  {
    to: "/versions",
    icon: "▤",
    name: t("nav.versions"),
    desc: t("forumHome.versionsDesc"),
    stat: t("forumHome.installedCount", { n: versions.installed.length }),
    last: versions.installed[0] ? t("forumHome.recent", { v: versions.installed[0].id }) : t("forumHome.noVersions"),
  },
  { to: "/resources", icon: "▣", name: t("nav.resources"), desc: t("forumHome.resourcesDesc"), stat: t("forumHome.resourcesStat"), last: t("forumHome.resourcesLast") },
  {
    to: "/account",
    icon: "☺",
    name: t("nav.account"),
    desc: t("forumHome.accountDesc"),
    stat: t("forumHome.accountCount", { n: accounts.accounts.length }),
    last: accounts.selected ? t("forumHome.current", { name: accounts.selected.name }) : t("forumHome.noAccount"),
  },
  { to: "/theme", icon: "✿", name: t("nav.theme"), desc: t("forumHome.themeDesc"), stat: t("forumHome.themeStat"), last: t("forumHome.themeLast") },
  { to: "/settings", icon: "⚙", name: t("nav.settings"), desc: t("forumHome.settingsDesc"), stat: t("forumHome.settingsStat"), last: t("forumHome.settingsLast") },
]);

const latest = computed(() =>
  [...versions.installed].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || (b.releaseTime ?? "").localeCompare(a.releaseTime ?? "")).slice(0, 6),
);
</script>

<template>
  <div class="home">
    <div class="left">
      <QuickLaunch />
      <NoticeBoard>
        <p><span class="tag">{{ $t("notice.pinned") }}</span><b>{{ $t("app.name") }}</b>{{ $t("home.intro") }}</p>
        <p class="muted">{{ $t("home.tipTheme") }}</p>
      </NoticeBoard>

      <div class="box">
        <div class="box-title">{{ $t("forumHome.boards") }}</div>
        <table class="thread-table boards">
          <tbody>
            <tr v-for="b in boards" :key="b.to">
              <td class="board-icon">{{ b.icon }}</td>
              <td>
                <RouterLink :to="b.to" class="board-name">{{ b.name }}</RouterLink>
                <div class="muted">{{ b.desc }}</div>
              </td>
              <td class="num">{{ b.stat }}</td>
              <td class="board-last muted">{{ b.last }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="box">
        <div class="box-title">{{ $t("forumHome.latest") }} <span class="extra muted">{{ $t("forumHome.latestHint") }}</span></div>
        <table class="thread-table">
          <tbody>
            <tr v-for="v in latest" :key="v.id">
              <td>
                <span class="tag" :class="v.kind">{{ kindLabel(v.kind) }}</span>
                <RouterLink to="/versions">{{ v.id }}</RouterLink>
              </td>
              <td class="num">{{ formatDate(v.releaseTime) }}</td>
            </tr>
            <tr v-if="!latest.length">
              <td class="muted"><i18n-t keypath="forumHome.empty"><RouterLink to="/download">{{ $t("nav.download") }}</RouterLink></i18n-t></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="right">
      <UserCard />
    </div>
  </div>
</template>

<style scoped>
.home { display: grid; grid-template-columns: 1fr 230px; gap: 12px; }
.notice p { margin: 4px 0; }
.boards td { padding: 10px; }
.board-icon { width: 44px; font-size: 22px; text-align: center; color: var(--brown); }
.board-name { font-weight: bold; font-size: 14px; }
.board-last { width: 190px; font-size: 12px; }
</style>
