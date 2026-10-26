// 生成 Tauri 应用内更新用的清单 latest.json。
// 用法：node scripts/make-latest-json.mjs <发布文件目录> <标签，如 v0.3.0> [仓库，默认 MRTangwin8/TheGodBBSLauncher]
// 目录里要有各平台的更新包和对应的 .sig 签名文件（文件名约定见下面的 PLATFORMS）。
// 一个 .sig 都没有说明这次构建没有签名密钥，什么也不生成，Release 照常发布但不支持应用内更新。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** 平台 → 更新包的文件名后缀（发布附件名已经换成英文） */
export const PLATFORMS = {
  "windows-x86_64": "_x64-setup.exe",
  "darwin-aarch64": "_aarch64.app.tar.gz",
  "darwin-x86_64": "_x64.app.tar.gz",
  "linux-x86_64": "_amd64.AppImage",
};

export function buildManifest(dir, tag, repo = "MRTangwin8/TheGodBBSLauncher", now = new Date()) {
  if (!/^v\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(tag)) throw new Error(`标签不合法：${tag}`);
  const files = fs.readdirSync(dir);
  const platforms = {};
  const missing = [];
  for (const [platform, suffix] of Object.entries(PLATFORMS)) {
    const bundle = files.find((f) => f.endsWith(suffix));
    const sig = bundle && files.includes(`${bundle}.sig`) ? `${bundle}.sig` : null;
    if (!bundle || !sig) {
      missing.push(platform);
      continue;
    }
    platforms[platform] = {
      signature: fs.readFileSync(path.join(dir, sig), "utf8").trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(bundle)}`,
    };
  }
  if (!Object.keys(platforms).length) return { manifest: null, missing };
  return { manifest: { version: tag.slice(1), notes: "", pub_date: now.toISOString(), platforms }, missing };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dir, tag, repo] = process.argv.slice(2);
  if (!dir || !tag) {
    console.error("用法：node scripts/make-latest-json.mjs <发布文件目录> <标签> [仓库]");
    process.exit(2);
  }
  const { manifest, missing } = buildManifest(dir, tag, repo);
  if (!manifest) {
    console.log("没有找到任何 .sig 签名文件：这次构建没有签名密钥，不生成 latest.json，应用内更新不可用");
    process.exit(0);
  }
  if (missing.length) console.warn(`警告：这些平台缺少更新包或签名，应用内更新对它们不可用：${missing.join("、")}`);
  fs.writeFileSync(path.join(dir, "latest.json"), JSON.stringify(manifest, null, 2) + "\n");
  console.log(`已生成 latest.json（${Object.keys(manifest.platforms).join("、")}）`);
}
