import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/game-lang.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 } }).outputText;
const { minecraftLang } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);

test("跟随系统语言，能分出繁体和地区", () => {
  assert.equal(minecraftLang("zh-CN", ["zh-CN"]), "zh_cn");
  assert.equal(minecraftLang("zh-CN", ["zh-TW"]), "zh_tw");
  assert.equal(minecraftLang("zh-CN", ["zh-Hant-HK"]), "zh_hk");
  assert.equal(minecraftLang("zh-CN", ["zh-Hant"]), "zh_tw");
  assert.equal(minecraftLang("zh-CN", ["zh-HK"]), "zh_hk");
  assert.equal(minecraftLang("ja", ["ja-JP"]), "ja_jp");
  assert.equal(minecraftLang("ja", ["ja"]), "ja_jp");
  assert.equal(minecraftLang("en", ["en-GB"]), "en_gb");
  assert.equal(minecraftLang("en", ["fr-FR", "en"]), "fr_fr", "法语系统界面是英文，游戏用法语");
  assert.equal(minecraftLang("en", ["ko"]), "ko_kr");
});

test("启动器里手动选了别的界面语言时跟启动器走", () => {
  assert.equal(minecraftLang("zh-CN", ["en-US"]), "zh_cn");
  assert.equal(minecraftLang("ja", ["zh-CN"]), "ja_jp");
  assert.equal(minecraftLang("en", ["zh-TW"]), "en_us");
});

test("认不出来时退回界面语言", () => {
  assert.equal(minecraftLang("en", []), "en_us");
  assert.equal(minecraftLang("en", ["es-419"]), "es_es");
  assert.equal(minecraftLang("en", ["xx"]), "en_us");
  assert.equal(minecraftLang("en", ["en-US-u-ca-gregory"]), "en_us");
});
