import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "../api";
import type { Settings } from "../api/types";

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings | null>(null);

  async function load() {
    settings.value = await api.getSettings();
  }

  /** 先改本地再写盘：界面立刻生效，连续保存也不会被慢回来的旧结果覆盖；写盘失败就撤回 */
  async function save(next: Settings) {
    const prev = settings.value;
    settings.value = { ...next };
    try {
      await api.saveSettings(next);
    } catch (e) {
      settings.value = prev;
      throw e;
    }
  }

  return { settings, load, save };
});
