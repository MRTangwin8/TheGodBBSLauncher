<script setup lang="ts">
import { onMounted } from "vue";
import { useVersionsStore } from "../stores/versions";
import { formatDate, kindLabel } from "../utils";

const versions = useVersionsStore();
onMounted(() => versions.loadInstalled());
</script>

<template>
  <div class="box">
    <div class="box-title">
      版本管理
      <span class="extra"><a @click="versions.loadInstalled()">刷新</a></span>
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
          </td>
          <td class="muted">{{ v.inheritsFrom ?? "-" }}</td>
          <td class="num">{{ formatDate(v.releaseTime) }}</td>
          <td class="num actions">
            <button class="btn small primary" disabled title="启动功能施工中">启动</button>
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
</style>
