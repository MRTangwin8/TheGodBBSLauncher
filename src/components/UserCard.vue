<script setup lang="ts">
import { computed } from "vue";
import { useVersionsStore } from "../stores/versions";
import { kindLabel, useAccountsStore } from "../stores/accounts";
import AccountAvatar from "./AccountAvatar.vue";
import { formatDuration } from "../utils";

const versions = useVersionsStore();
const accounts = useAccountsStore();
const totalPlay = computed(() => versions.installed.reduce((s, v) => s + v.playTimeSecs, 0));
// 积分：每小时 10 分 + 每个版本 5 分（和首页一致）
const points = computed(() => Math.floor(totalPlay.value / 360) + versions.installed.length * 5);
const acc = computed(() => accounts.selected);
</script>

<template>
  <div class="box">
    <div class="box-title">{{ $t("userCard.title") }}</div>
    <div class="box-body user">
      <div class="avatar"><AccountAvatar :account="acc" mode="body" :size="128" /></div>
      <div class="name">{{ acc?.name ?? $t("shell.guestName") }}</div>
      <div class="group" :title="$t('userCard.godTip')">{{ $t("shell.god") }}</div>
      <dl>
        <dt>{{ $t("userCard.accountType") }}</dt><dd>{{ acc ? kindLabel(acc.kind) : $t("userCard.notLoggedIn") }}</dd>
        <dt>{{ $t("userCard.points") }}</dt><dd>{{ points }}</dd>
        <dt>{{ $t("userCard.installed") }}</dt><dd>{{ versions.installed.length }}</dd>
        <dt>{{ $t("userCard.playTime") }}</dt><dd>{{ formatDuration(totalPlay) }}</dd>
      </dl>
      <RouterLink to="/account">{{ acc ? $t("userCard.manage") : $t("userCard.loginOrAdd") }} »</RouterLink>
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
