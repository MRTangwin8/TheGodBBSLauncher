// 公告栏：启动时从 GitHub 仓库的 announcements.json 拉一次（运行期间不再拉），断网时用上次缓存的。
// 内容一律当纯文本显示；格式和发布方法见 docs/ANNOUNCEMENTS.md。
import { computed, ref } from "vue";
import { api } from "./api";
import type { AnnouncementFeed } from "./api/types";
import { useSettingsStore } from "./stores/settings";

const feed = ref<AnnouncementFeed | null>(null);
export const announcementsLoading = ref(false);
/** 最近一次手动刷新的结果说明（成功或失败），设置页显示 */
export const announcementsNote = ref<{ ok: boolean; text: string } | null>(null);

/** 拉公告。启动时静默（出错不打扰）；设置页的"立即刷新"传 manual，会给出结果说明 */
export async function loadAnnouncements(manual = false) {
  announcementsLoading.value = true;
  if (manual) announcementsNote.value = null;
  try {
    feed.value = await api.getAnnouncements(manual);
    if (manual) {
      const n = feed.value.items.length;
      announcementsNote.value = { ok: true, text: feed.value.stale ? "连不上 GitHub，显示的是上次缓存的公告" : n ? `已刷新，共 ${n} 条公告` : "已刷新，现在没有公告" };
    }
  } catch (e) {
    if (manual) announcementsNote.value = { ok: false, text: String(e) };
  } finally {
    announcementsLoading.value = false;
  }
}

/** 要显示的公告：去掉用户已经关掉的 */
export const announcements = computed(() => {
  const dismissed = new Set(useSettingsStore().settings?.dismissedAnnouncements ?? []);
  return (feed.value?.items ?? []).filter((a) => !dismissed.has(a.id));
});

export async function dismissAnnouncement(id: string) {
  const store = useSettingsStore();
  if (!store.settings) return;
  const list = store.settings.dismissedAnnouncements ?? [];
  if (!list.includes(id)) await store.save({ ...store.settings, dismissedAnnouncements: [...list, id] });
}

/** 把关掉过的公告重新显示出来 */
export async function restoreAnnouncements() {
  const store = useSettingsStore();
  if (store.settings) await store.save({ ...store.settings, dismissedAnnouncements: [] });
}
