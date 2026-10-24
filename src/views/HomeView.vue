<script setup lang="ts">
import { computed } from "vue";
import { useSettingsStore } from "../stores/settings";
import ForumHome from "../components/home/ForumHome.vue";
import LaunchHome from "../components/home/LaunchHome.vue";

// 经典论坛布局用第一版的论坛首页，其它布局用启动卡片首页
const settings = useSettingsStore();
const classic = computed(() => (settings.settings?.layout ?? "classic") === "classic");
</script>

<template>
  <ForumHome v-if="classic" />
  <LaunchHome v-else />
</template>
