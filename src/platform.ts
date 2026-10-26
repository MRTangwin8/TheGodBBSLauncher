// 平台判断。macOS 用原生红绿灯（覆盖式标题栏），Windows / Linux 用自绘的最小化 / 最大化 / 关闭
//
// 浏览器预览里在地址后面加 ?mockMac 可以模拟 macOS 的布局，并画出三个假红绿灯，方便在没有 Mac 的机器上检查留白
export const isMac = /Mac/i.test(navigator.userAgent) || new URLSearchParams(location.search).has("mockMac");

/** 把平台写到 <html data-os>，样式里用 html[data-os="mac"] 区分 */
export function setupPlatform() {
  document.documentElement.dataset.os = isMac ? "mac" : "other";
  if (isMac && new URLSearchParams(location.search).has("mockMac")) {
    const lights = document.createElement("div");
    lights.setAttribute("aria-hidden", "true");
    lights.style.cssText = "position:fixed;top:12px;left:14px;z-index:99999;display:flex;gap:8px;pointer-events:none";
    for (const color of ["#ff5f57", "#febc2e", "#28c840"]) {
      const dot = document.createElement("i");
      dot.style.cssText = `width:12px;height:12px;border-radius:50%;background:${color}`;
      lights.appendChild(dot);
    }
    document.body.appendChild(lights);
  }
}
