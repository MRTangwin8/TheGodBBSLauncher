import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api } from "../api";
import type { Account, AccountKind, Skin } from "../api/types";

export const kindLabel: Record<AccountKind, string> = {
  microsoft: "正版",
  offline: "离线",
  yggdrasil: "外置",
};

export const useAccountsStore = defineStore("accounts", () => {
  const accounts = ref<Account[]>([]);
  const selectedId = ref<string | null>(null);
  const selected = computed(() => accounts.value.find((a) => a.id === selectedId.value) ?? null);
  /** 账号 id → 皮肤（null 表示没有皮肤或读取失败） */
  const skins = ref<Record<string, Skin | null>>({});
  const loading = new Set<string>();

  async function loadSkin(a: Account) {
    if (a.kind === "offline" || a.id in skins.value || loading.has(a.id)) return;
    loading.add(a.id);
    try {
      skins.value[a.id] = await api.accountSkin(a.id);
    } catch {
      skins.value[a.id] = null;
    } finally {
      loading.delete(a.id);
    }
  }

  async function load() {
    const list = await api.listAccounts();
    accounts.value = list.accounts;
    selectedId.value = list.selected;
  }

  async function select(id: string) {
    await api.selectAccount(id);
    selectedId.value = id;
  }

  async function remove(id: string) {
    await api.removeAccount(id);
    await load();
  }

  /** 新增账号的命令都会把它设为当前账号，完成后统一刷新 */
  async function added(_: Account) {
    await load();
  }

  return { accounts, selectedId, selected, skins, load, loadSkin, select, remove, added };
});
