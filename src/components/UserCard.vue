<script setup lang="ts">
import { computed } from "vue";
import { useVersionsStore } from "../stores/versions";

const versions = useVersionsStore();
// 积分暂时按本地已安装版本数计算，以后换成游戏时长
const points = computed(() => versions.installed.length * 10);
</script>

<template>
  <div class="box">
    <div class="box-title">个人信息</div>
    <div class="box-body user">
      <div class="avatar" aria-hidden="true">
        <!-- 原创像素头像 -->
        <svg viewBox="0 0 8 8" shape-rendering="crispEdges">
          <rect width="8" height="8" fill="#6b4a2b" />
          <rect y="2" width="8" height="6" fill="#c69c6d" />
          <rect x="1" y="4" width="2" height="1" fill="#fff" /><rect x="2" y="4" width="1" height="1" fill="#3b5ba8" />
          <rect x="5" y="4" width="2" height="1" fill="#fff" /><rect x="5" y="4" width="1" height="1" fill="#3b5ba8" />
          <rect x="3" y="6" width="2" height="1" fill="#7a4a2a" />
        </svg>
      </div>
      <div class="name">Steve</div>
      <div class="group" title="在本启动器里，你拥有一切权限">神权</div>
      <dl>
        <dt>积分</dt><dd>{{ points }}</dd>
        <dt>已装版本</dt><dd>{{ versions.installed.length }}</dd>
        <dt>游戏时长</dt><dd>0 小时</dd>
      </dl>
      <RouterLink to="/account">进入个人空间 »</RouterLink>
    </div>
  </div>
</template>

<style scoped>
.user { text-align: center; }
.avatar svg { width: 72px; height: 72px; border: 2px solid var(--border-strong); image-rendering: pixelated; }
.name { font-weight: bold; font-size: 15px; margin-top: 6px; }
.group { display: inline-block; margin: 4px 0 8px; padding: 0 8px; color: #fff; background: linear-gradient(#f2c230, #c8901a); border-radius: 2px; font-weight: bold; cursor: help; }
dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; text-align: left; margin: 0 0 10px; padding: 8px; background: var(--bg-box-alt); border-radius: var(--radius); }
dt { color: var(--text-muted); }
dd { margin: 0; text-align: right; }
</style>
