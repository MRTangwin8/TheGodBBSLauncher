<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { useVersionsStore } from "../stores/versions";
import { useGameStore } from "../stores/game";
import { useAccountsStore } from "../stores/accounts";
import { formatDate, kindLabel } from "../utils";

const versions = useVersionsStore();
const game = useGameStore();
const accounts = useAccountsStore();
const router = useRouter();
onMounted(() => versions.loadInstalled());

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
      版本管理
      <span class="extra">
        <template v-if="accounts.selected">当前账号：<b>{{ accounts.selected.name }}</b> · </template>
        <a @click="versions.loadInstalled()">刷新</a>
      </span>
    </div>
    <div v-if="game.error" class="box-body">
      <div class="mod-tip">【版主提示】启动失败：{{ game.error }}</div>
    </div>
    <table class="thread-table">
      <thead>
        <tr><th>主题</th><th>继承自</th><th class="num">发布时间</th><th class="num">操作</th></tr>
      </thead>
      <tbody>
        <tr v-for="v in versions.installed" :key="v.id">
          <td>
            <span class="tag" :class="v.kind">{{ kindLabel[v.kind ?? ""] ?? v.kind ?? "自定义" }}</span>
            <b>{{ v.id }}</b>
            <span v-if="!v.hasJar" class="error-text"> （缺少 client.jar）</span>
            <span v-if="game.running.some((r) => r.versionId === v.id)" class="playing">● 游戏中</span>
          </td>
          <td class="muted">{{ v.inheritsFrom ?? "-" }}</td>
          <td class="num">{{ formatDate(v.releaseTime) }}</td>
          <td class="num actions">
            <button
              class="btn small primary"
              :disabled="!!game.launching"
              :title="accounts.selected ? '' : '先添加一个账号'"
              @click="launch(v.id)"
            >
              {{ game.launching === v.id ? "启动中…" : "启动" }}
            </button>
            <button
              class="btn small"
              :disabled="!!v.inheritsFrom || versions.jobs[v.id]?.status === 'running'"
              @click="versions.install(v.id)"
            >
              补全文件
            </button>
          </td>
        </tr>
        <tr v-if="!versions.installed.length">
          <td colspan="4" class="muted">本版块暂无主题，去 <RouterLink to="/download">下载中心</RouterLink> 安装一个吧</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.actions { display: flex; gap: 6px; justify-content: flex-end; }
.playing { color: var(--grass); font-size: 12px; margin-left: 6px; }
</style>
