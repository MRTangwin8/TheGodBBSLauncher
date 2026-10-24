import { defineStore } from "pinia";
import { ref } from "vue";

/** 等待导入的整合包文件（按钮选择或拖进窗口） */
export const useModpackStore = defineStore("modpack", () => {
  const importPath = ref<string | null>(null);
  return { importPath };
});
