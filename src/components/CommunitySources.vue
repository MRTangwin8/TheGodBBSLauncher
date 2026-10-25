<script setup lang="ts">
import { ref } from "vue";
import { browseCommunity, communitySites, type CommunitySite } from "../community";
import ModernIcon from "./ModernIcon.vue";

const sites = communitySites.map((site) => ({ ...site, domain: new URL(site.url).hostname }));
const busy = ref<CommunitySite | null>(null);
const error = ref("");

async function open(site: CommunitySite) {
  busy.value = site;
  error.value = "";
  try {
    await browseCommunity(site);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = null;
  }
}
</script>

<template>
  <div class="community-list">
    <p v-if="error" class="error-text" role="alert">{{ error }}</p>
    <section v-for="site in sites" :key="site.id" class="community-row">
      <ModernIcon name="resources" :size="28" />
      <div class="community-name">
        <h2>{{ site.name }}</h2>
        <span class="muted">{{ site.category }}</span>
        <span class="domain muted">{{ site.domain }}</span>
      </div>
      <div class="community-actions">
        <button class="btn primary" :disabled="busy !== null" @click="open(site.id)">
          <ModernIcon name="external" :size="16" />
          {{ busy === site.id ? "打开中..." : "系统浏览器" }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.community-list { padding: 0 16px; }
.community-row { display: flex; align-items: center; gap: 16px; padding: 22px 0; border-bottom: 1px solid var(--border); }
.community-row:last-child { border-bottom: none; }
.community-row > svg { flex-shrink: 0; color: var(--grass); }
.community-name { display: flex; flex-direction: column; gap: 5px; min-width: 0; flex: 1; overflow-wrap: anywhere; }
.community-name h2 { margin: 0; font-size: 16px; color: var(--text); }
.community-name > span { font-size: 12px; }
.domain { font-family: monospace; }
.community-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.community-actions .btn { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 32px; }
@media (max-width: 680px) {
  .community-row { flex-wrap: wrap; gap: 12px; }
  .community-actions { flex-basis: 100%; padding-left: 40px; }
}
</style>
