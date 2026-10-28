// 发行版本。微软商店版构建时设 VITE_STORE=1（后端同时用 --features store 编译，真正的限制在后端 src-tauri/src/edition.rs）：
// 只能用微软正版账号、不在应用内更新。浏览器预览里在地址后面加 ?mockStore 可以看商店版的界面
export const isStore = import.meta.env.VITE_STORE === "1" || new URLSearchParams(location.search).has("mockStore");

/** 商店版不能用的账号类型 */
export const storeBlocks = (kind: string) => isStore && kind !== "microsoft";
