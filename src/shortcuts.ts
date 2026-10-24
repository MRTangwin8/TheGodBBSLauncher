// 启动器跑在浏览器内核（WebView2）里，浏览器自带的快捷键和右键菜单对桌面应用没意义，
// 有的还有害：Ctrl+R / F5 会刷新页面，把正在运行的游戏日志和下载进度全清掉。
// 开发者工具另外在 tauri.conf.json 里关掉（F12 由浏览器内核直接处理，网页拦不住）。

function isEditable(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  return el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
}

/** 按下时要拦住的组合键；返回 true 表示拦截 */
export function shouldBlock(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">, editable: boolean): boolean {
  const key = e.key.toLowerCase();
  const mod = e.ctrlKey || e.metaKey;
  // 刷新、开发者工具、浏览器功能键
  if (["f1", "f3", "f5", "f6", "f7", "f12", "browserrefresh", "browserback", "browserforward", "browsersearch"].includes(key)) return true;
  if (mod && e.shiftKey && ["i", "j", "c", "r", "delete", "n", "b", "o"].includes(key)) return true;
  // 前进 / 后退（会打乱页面状态）
  if (e.altKey && (key === "arrowleft" || key === "arrowright" || key === "home")) return true;
  if (mod) {
    // 刷新、打印、保存网页、查看源码、查找、历史、下载、新窗口、书签、关闭标签页
    if (["r", "p", "s", "u", "f", "g", "h", "j", "n", "o", "d", "w", "t", "e", "l", "k"].includes(key)) return true;
    // 缩放：页面布局是按固定比例设计的
    if (["+", "=", "-", "0"].includes(key)) return true;
    // 输入框外的全选没意义，会把整个页面选蓝
    if (key === "a" && !editable) return true;
  }
  return false;
}

export function installShortcutGuard() {
  window.addEventListener(
    "keydown",
    (e) => {
      if (shouldBlock(e, isEditable(e.target))) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    { capture: true },
  );
  // Ctrl+滚轮缩放
  window.addEventListener("wheel", (e) => e.ctrlKey && e.preventDefault(), { passive: false });
  // 浏览器右键菜单（返回、刷新、打印、检查……）；输入框里保留，方便粘贴
  window.addEventListener("contextmenu", (e) => {
    if (!isEditable(e.target)) e.preventDefault();
  });
  // 鼠标侧键的前进后退
  window.addEventListener("mouseup", (e) => {
    if (e.button === 3 || e.button === 4) e.preventDefault();
  });
}
