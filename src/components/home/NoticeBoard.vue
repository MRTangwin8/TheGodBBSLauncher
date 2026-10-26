<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "../../api";
import { announcements, dismissAnnouncement } from "../../announcements";

// 两种首页共用的公告栏：GitHub 上的公告（纯文本）+ 各首页自己的内置文字（插槽）
const error = ref("");

async function open(url: string) {
  error.value = "";
  try {
    if (inTauri) await openUrl(url);
    else window.open(url, "_blank", "noopener,noreferrer");
  } catch (e) {
    error.value = `打不开浏览器：${e}`;
  }
}
</script>

<template>
  <div class="box">
    <div class="box-title">📢 公告栏</div>
    <div class="box-body notice">
      <article v-for="a in announcements" :key="a.id" class="item" :class="'lv-' + a.level">
        <div class="head">
          <span v-if="a.pinned" class="tag pin">置顶</span>
          <span v-else-if="a.level === 'warn'" class="tag warn">注意</span>
          <b>{{ a.title }}</b>
          <span v-if="a.date" class="muted date">{{ a.date }}</span>
          <a class="close" title="关闭这条公告" @click="dismissAnnouncement(a.id)">×</a>
        </div>
        <p v-if="a.body" class="body">{{ a.body }}</p>
        <a v-if="a.link" class="more" @click="open(a.link)">{{ a.linkText || "查看详情" }} »</a>
      </article>
      <p v-if="error" class="error-text small">{{ error }}</p>
      <!-- 内置的简介和小贴士由各个首页自己放 -->
      <slot />
    </div>
  </div>
</template>

<style scoped>
.notice p { margin: 4px 0; line-height: 1.7; }
.item { padding: 6px 0 8px; border-bottom: 1px dashed var(--border); }
.head { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.head b { font-size: 14px; }
.date { font-size: 12px; }
.close { margin-left: auto; font-size: 16px; line-height: 1; color: var(--text-muted); cursor: pointer; }
.close:hover { color: var(--red); text-decoration: none; }
/* 公告正文是纯文本：保留换行，长单词也能折行 */
.body { white-space: pre-wrap; word-break: break-word; }
.more { font-size: 12px; cursor: pointer; }
.tag.pin { background: var(--red); }
.tag.warn { background: #c47d1a; }
.lv-warn .head b { color: #a85d00; }
.small { font-size: 12px; }
</style>
