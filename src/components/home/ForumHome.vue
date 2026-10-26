<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useVersionsStore } from "../../stores/versions";
import { useAccountsStore } from "../../stores/accounts";
import UserCard from "../UserCard.vue";
import QuickLaunch from "./QuickLaunch.vue";
import NoticeBoard from "./NoticeBoard.vue";
import { formatDate, kindLabel } from "../../utils";

// 第一版的论坛首页：公告栏 + 版块列表 + 最新帖子 + 个人信息
const versions = useVersionsStore();
const accounts = useAccountsStore();
onMounted(() => versions.manifest || versions.loadManifest());

const boards = computed(() => [
  {
    to: "/download",
    icon: "⬇",
    name: "下载中心",
    desc: "原版 / 加载器一键安装，导入整合包",
    stat: versions.manifest ? `${versions.manifest.versions.length} 个版本` : "…",
    last: versions.manifest ? `最新正式版 ${versions.manifest.latest.release}` : "",
  },
  {
    to: "/versions",
    icon: "▤",
    name: "版本管理",
    desc: "启动、补全文件、重命名、导出整合包",
    stat: `${versions.installed.length} 个已安装`,
    last: versions.installed[0] ? `最近：${versions.installed[0].id}` : "还没有安装任何版本",
  },
  { to: "/resources", icon: "▣", name: "资源中心", desc: "Modrinth / CurseForge 模组、资源包、光影", stat: "自动装前置", last: "下载时检查兼容性" },
  {
    to: "/account",
    icon: "☺",
    name: "个人空间",
    desc: "微软正版 / 离线 / 外置登录",
    stat: `${accounts.accounts.length} 个账号`,
    last: accounts.selected ? `当前：${accounts.selected.name}` : "还没有账号",
  },
  { to: "/theme", icon: "✿", name: "主题中心", desc: "布局、配色、深浅色、背景图", stat: "三种布局", last: "七套配色" },
  { to: "/settings", icon: "⚙", name: "设置", desc: "游戏目录、下载源、Java、内存、参数", stat: "完全神权", last: "所有参数你说了算" },
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
        <p><span class="tag">置顶</span><b>神权论坛启动器</b>：原版 / 加载器 / 整合包一键安装，资源中心下载时自动检查兼容性，崩溃了版主帮你看。</p>
        <p class="muted">小贴士：把 .mrpack 或整合包 .zip 直接拖进窗口就能导入；去「主题中心」可以换布局和配色。</p>
      </NoticeBoard>

      <div class="box">
        <div class="box-title">版块</div>
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
        <div class="box-title">最新帖子 <span class="extra muted">最近玩过的版本</span></div>
        <table class="thread-table">
          <tbody>
            <tr v-for="v in latest" :key="v.id">
              <td>
                <span class="tag" :class="v.kind">{{ kindLabel[v.kind ?? ""] ?? v.kind ?? "自定义" }}</span>
                <RouterLink to="/versions">{{ v.id }}</RouterLink>
              </td>
              <td class="num">{{ formatDate(v.releaseTime) }}</td>
            </tr>
            <tr v-if="!latest.length">
              <td class="muted">沙发还空着～ 去 <RouterLink to="/download">下载中心</RouterLink> 装一个版本抢沙发吧</td>
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
