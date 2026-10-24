<script setup lang="ts">
import { computed, watchEffect } from "vue";
import type { Account } from "../api/types";
import { useAccountsStore } from "../stores/accounts";
import SkinView from "./SkinView.vue";
import PixelHead from "./PixelHead.vue";

// 有真实皮肤（正版 / 外置）就画皮肤，否则用按 UUID 生成的像素头像
const props = withDefaults(defineProps<{ account: Account | null; mode?: "head" | "body"; size?: number }>(), { mode: "head", size: 72 });
const store = useAccountsStore();

watchEffect(() => {
  if (props.account) store.loadSkin(props.account);
});
const skin = computed(() => (props.account ? store.skins[props.account.id] : null));
</script>

<template>
  <SkinView v-if="skin" :src="skin.dataUrl" :slim="skin.slim" :mode="mode" :size="size" />
  <PixelHead v-else :seed="account?.uuid ?? 'guest'" :size="mode === 'body' ? size / 2 : size" />
</template>
