// 更新检测：启动时静默查一次，设置页可以手动查。发现新版本就弹窗问一声
import { ref } from "vue";
import { api } from "./api";
import type { UpdateInfo } from "./api/types";
import { useSettingsStore } from "./stores/settings";

/** 正在弹窗询问的新版本 */
export const pendingUpdate = ref<UpdateInfo | null>(null);

/**
 * 查一次更新。静默模式（启动时）：出错不打扰、用户跳过过的版本不再弹；
 * 手动模式：出错会抛出去让界面显示，跳过过的版本也照样弹。
 */
export async function checkForUpdate(manual: boolean): Promise<UpdateInfo | null> {
  const store = useSettingsStore();
  try {
    const info = await api.checkUpdate();
    if (info.hasUpdate && (manual || info.latest !== store.settings?.skippedUpdate)) pendingUpdate.value = info;
    return info;
  } catch (e) {
    if (manual) throw e;
    return null;
  }
}

/** 记住"跳过此版本"，只对这个版本号有效，更新的版本照常提示 */
export async function skipUpdate(version: string) {
  const store = useSettingsStore();
  pendingUpdate.value = null;
  if (store.settings && version) await store.save({ ...store.settings, skippedUpdate: version });
}
