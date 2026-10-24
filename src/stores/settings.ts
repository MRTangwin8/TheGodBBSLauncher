import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "../api";
import type { Settings } from "../api/types";

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings | null>(null);

  async function load() {
    settings.value = await api.getSettings();
  }

  async function save(next: Settings) {
    await api.saveSettings(next);
    settings.value = { ...next };
  }

  return { settings, load, save };
});
