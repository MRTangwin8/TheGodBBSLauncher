<script setup lang="ts">
import { computed } from "vue";
import { useVersionsStore } from "../stores/versions";
import { kindLabel, useAccountsStore } from "../stores/accounts";
import PixelHead from "./PixelHead.vue";

const versions = useVersionsStore();
const accounts = useAccountsStore();
// 积分暂时按本地已安装版本数计算，以后换成游戏时长
const points = computed(() => versions.installed.length * 10);
const acc = computed(() => accounts.selected);
</script>

<template>
  <div class="box">
    <div class="box-title">个人信息</div>
    <div class="box-body user">
      <div class="avatar"><PixelHead :seed="acc?.uuid ?? 'guest'" /></div>
      <div class="name">{{ acc?.name ?? "游客" }}</div>
      <div class="group" title="在本启动器里，你拥有一切权限">神权</div>
      <dl>
        <dt>账号类型</dt><dd>{{ acc ? kindLabel[acc.kind] : "未登录" }}</dd>
        <dt>积分</dt><dd>{{ points }}</dd>
        <dt>已装版本</dt><dd>{{ versions.installed.length }}</dd>
        <dt>游戏时长</dt><dd>0 小时</dd>
      </dl>
      <RouterLink to="/account">{{ acc ? "管理账号 »" : "登录 / 添加账号 »" }}</RouterLink>
    </div>
  </div>
</template>

<style scoped>
.user { text-align: center; }
.avatar { display: flex; justify-content: center; }
.name { font-weight: bold; font-size: 15px; margin-top: 6px; }
.group { display: inline-block; margin: 4px 0 8px; padding: 0 8px; color: #fff; background: linear-gradient(#f2c230, #c8901a); border-radius: 2px; font-weight: bold; cursor: help; }
dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; text-align: left; margin: 0 0 10px; padding: 8px; background: var(--bg-box-alt); border-radius: var(--radius); }
dt { color: var(--text-muted); }
dd { margin: 0; text-align: right; }
</style>
