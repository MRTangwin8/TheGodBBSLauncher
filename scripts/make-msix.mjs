// 把商店版的程序打成微软商店要的 MSIX 包（不签名，上传到合作伙伴中心后由微软签名）。
// 用法：node scripts/make-msix.mjs <godbbs-launcher.exe> <输出目录>
// 先用 `VITE_STORE=1 npx tauri build --features store --no-bundle` 编出商店版的 exe；需要 Windows SDK 里的 makeappx.exe。
//
// 包标识（Name / Publisher / PublisherDisplayName）来自合作伙伴中心「产品标识」页，必须原样一致，否则商店拒收。
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const IDENTITY = {
  name: "UACG.637635993B2C9",
  publisher: "CN=938704B1-0CD5-480A-A76D-B2680294F879",
  publisherDisplayName: "UACG",
};
export const DISPLAY_NAME = "神权论坛启动器";
export const EXECUTABLE = "godbbs-launcher.exe";
/** 包里要带的图标：清单里的文件名 → src-tauri/icons 里的文件 */
export const ASSETS = {
  "StoreLogo.png": "StoreLogo.png",
  "Square44x44Logo.png": "Square44x44Logo.png",
  "Square150x150Logo.png": "Square150x150Logo.png",
};

/** 0.3.7 → 0.3.7.0。商店要求四段数字，最后一段必须是 0（留给商店用） */
export function msixVersion(version) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(version.trim());
  if (!m || m.slice(1).some((n) => Number(n) > 65535)) throw new Error(`版本号不能用于 MSIX：${version}（要求 x.y.z，每段不超过 65535）`);
  return `${Number(m[1])}.${Number(m[2])}.${Number(m[3])}.0`;
}

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/**
 * 生成 AppxManifest.xml。
 * runFullTrust：普通的 Win32 程序（要启动 Java 子进程、读写任意游戏目录），不是沙盒里的 UWP 应用。
 * 最低 Windows 10 1809（17763）：WebView2 和 Tauri 的最低要求。
 */
export function buildManifest(version) {
  return `<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="${esc(IDENTITY.name)}" Publisher="${esc(IDENTITY.publisher)}" Version="${msixVersion(version)}" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>${esc(DISPLAY_NAME)}</DisplayName>
    <PublisherDisplayName>${esc(IDENTITY.publisherDisplayName)}</PublisherDisplayName>
    <Logo>Assets\\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="zh-CN" />
  </Resources>
  <Applications>
    <Application Id="GodBBSLauncher" Executable="${EXECUTABLE}" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="${esc(DISPLAY_NAME)}" Description="${esc("Minecraft Java 版启动器")}" BackgroundColor="transparent"
                          Square150x150Logo="Assets\\Square150x150Logo.png" Square44x44Logo="Assets\\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <Capability Name="internetClient" />
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
`;
}

/** 在 Windows SDK 里找最新版本的 makeappx.exe */
export function findMakeAppx(root = "C:\\Program Files (x86)\\Windows Kits\\10\\bin") {
  if (process.env.MAKEAPPX && fs.existsSync(process.env.MAKEAPPX)) return process.env.MAKEAPPX;
  const versions = fs.existsSync(root) ? fs.readdirSync(root).filter((v) => /^10\.\d+\.\d+\.\d+$/.test(v)) : [];
  versions.sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
  for (const v of versions) {
    const p = path.join(root, v, "x64", "makeappx.exe");
    if (fs.existsSync(p)) return p;
  }
  throw new Error("找不到 makeappx.exe：请安装 Windows SDK，或用环境变量 MAKEAPPX 指定路径");
}

function main() {
  const [exe, outDir] = process.argv.slice(2);
  if (!exe || !outDir) throw new Error("用法：node scripts/make-msix.mjs <godbbs-launcher.exe> <输出目录>");
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const version = JSON.parse(fs.readFileSync(path.join(repo, "package.json"), "utf8")).version;
  const layout = path.join(outDir, "msix-layout");
  fs.rmSync(layout, { recursive: true, force: true });
  fs.mkdirSync(path.join(layout, "Assets"), { recursive: true });
  fs.copyFileSync(exe, path.join(layout, EXECUTABLE));
  for (const [name, src] of Object.entries(ASSETS)) fs.copyFileSync(path.join(repo, "src-tauri", "icons", src), path.join(layout, "Assets", name));
  fs.writeFileSync(path.join(layout, "AppxManifest.xml"), buildManifest(version));
  const out = path.join(outDir, `GodBBSLauncher_${version}_x64.msix`);
  execFileSync(findMakeAppx(), ["pack", "/o", "/d", layout, "/p", out], { stdio: "inherit" });
  console.log(`已生成 ${out}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
