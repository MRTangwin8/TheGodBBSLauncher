import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { buildManifest } from "../scripts/make-latest-json.mjs";

function release(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "latest-json-"));
  for (const [name, text] of Object.entries(files)) fs.writeFileSync(path.join(dir, name), text);
  return dir;
}
const ALL = {
  "GodBBSLauncher_0.3.0_x64-setup.exe": "x",
  "GodBBSLauncher_0.3.0_x64-setup.exe.sig": "SIG-WIN\n",
  "GodBBSLauncher_0.3.0_aarch64.app.tar.gz": "x",
  "GodBBSLauncher_0.3.0_aarch64.app.tar.gz.sig": "SIG-ARM",
  "GodBBSLauncher_0.3.0_x64.app.tar.gz": "x",
  "GodBBSLauncher_0.3.0_x64.app.tar.gz.sig": "SIG-INTEL",
  "GodBBSLauncher_0.3.0_amd64.AppImage": "x",
  "GodBBSLauncher_0.3.0_amd64.AppImage.sig": "SIG-LINUX",
  "GodBBSLauncher_0.3.0_x64_zh-CN.msi": "x", // 普通安装包不参与更新清单
};

test("四个平台的更新包和签名都对得上", () => {
  const { manifest, missing } = buildManifest(release(ALL), "v0.3.0", "o/r", new Date("2026-10-26T00:00:00Z"));
  assert.deepEqual(missing, []);
  assert.equal(manifest.version, "0.3.0");
  assert.equal(manifest.pub_date, "2026-10-26T00:00:00.000Z");
  assert.deepEqual(Object.keys(manifest.platforms).sort(), ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"]);
  assert.equal(manifest.platforms["windows-x86_64"].signature, "SIG-WIN", "签名要去掉末尾换行");
  assert.equal(manifest.platforms["darwin-aarch64"].signature, "SIG-ARM");
  assert.equal(manifest.platforms["darwin-x86_64"].signature, "SIG-INTEL");
  assert.equal(manifest.platforms["darwin-x86_64"].url, "https://github.com/o/r/releases/download/v0.3.0/GodBBSLauncher_0.3.0_x64.app.tar.gz");
});

test("x64 的 mac 包不会被当成 aarch64，也不会和 Windows 的 x64-setup 混", () => {
  const { manifest } = buildManifest(release(ALL), "v0.3.0");
  assert.match(manifest.platforms["darwin-aarch64"].url, /aarch64\.app\.tar\.gz$/);
  assert.match(manifest.platforms["windows-x86_64"].url, /x64-setup\.exe$/);
});

test("没有任何签名文件就不生成清单", () => {
  const { manifest, missing } = buildManifest(release({ "GodBBSLauncher_0.3.0_x64-setup.exe": "x" }), "v0.3.0");
  assert.equal(manifest, null);
  assert.equal(missing.length, 4);
});

test("缺平台时只写有签名的，并报告缺了谁", () => {
  const { "GodBBSLauncher_0.3.0_amd64.AppImage.sig": _drop, ...partial } = ALL;
  const { manifest, missing } = buildManifest(release(partial), "v0.3.0");
  assert.deepEqual(missing, ["linux-x86_64"]);
  assert.ok(!manifest.platforms["linux-x86_64"]);
});

test("标签不合法就报错，不往网址里拼", () => {
  for (const bad of ["0.3.0", "v0.3", "v0.3.0/../x", "v0.3.0?x=1", "latest"]) {
    assert.throws(() => buildManifest(release(ALL), bad), /标签不合法/, bad);
  }
});
