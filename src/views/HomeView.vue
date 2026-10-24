<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useVersionsStore } from "../stores/versions";
import UserCard from "../components/UserCard.vue";
import { formatDate, kindLabel } from "../utils";

const versions = useVersionsStore();
onMounted(() => versions.manifest || versions.loadManifest());

const boards = computed(() => [
  {
    to: "/download",
    icon: "⬇",
    name: "下载中心",
    desc: "原版全版本一键安装，官方源 / BMCLAPI 自动测速",
    stat: versions.manifest ? `${versions.manifest.versions.length} 个版本` : "…",
    last: versions.manifest ? `最新正式版 ${versions.manifest.latest.release}` : "",
  },
  {
    to: "/versions",
    icon: "▤",
    name: "版本管理",
    desc: "已安装的版本，补全文件、版本隔离（施工中）",
    stat: `${versions.installed.length} 个已安装`,
    last: versions.installed[0] ? `最近：${versions.installed[0].id}` : "还没有安装任何版本",
  },
  { to: "/account", icon: "☺", name: "个人空间", desc: "账号、勋章、用户组", stat: "离线", last: "正版登录施工中" },
  { to: "/settings", icon: "⚙", name: "设置", desc: "游戏目录、下载源、并发数", stat: "完全神权", last: "所有参数你说了算" },
]);
</script>

<template>
  <div class="home">
    <div class="left">
      <div class="box">
        <div class="box-title">📢 公告栏</div>
        <div class="box-body notice">
          <p><span class="tag">置顶</span><b>神权论坛启动器</b> 第一版开张！目前能浏览并下载全部原版版本。</p>
          <p class="muted">启动游戏、正版登录、模组加载器正在施工，请各位坛友耐心等待版主更新。</p>
        </div>
      </div>

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
        <div class="box-title">最新帖子 <span class="extra muted">最近安装的版本</span></div>
        <table class="thread-table">
          <tbody>
            <tr v-for="v in versions.installed.slice(0, 6)" :key="v.id">
              <td>
                <span class="tag" :class="v.kind">{{ kindLabel[v.kind ?? ""] ?? v.kind ?? "自定义" }}</span>
                <RouterLink to="/versions">{{ v.id }}</RouterLink>
              </td>
              <td class="num">{{ formatDate(v.releaseTime) }}</td>
            </tr>
            <tr v-if="!versions.installed.length">
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
