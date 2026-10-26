import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/buttons-logic.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 } }).outputText;
const logic = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
const { orderNav, fullOrder, moveItem, validateServer, validateQuick, describeQuick, newQuickId, ALWAYS_VISIBLE } = logic;

const NAV = ["/", "/download", "/versions", "/resources", "/log", "/account", "/theme", "/settings"].map((path) => ({ path }));
const paths = (list) => list.map((x) => x.path);

test("没有自定义时保持默认顺序", () => {
  assert.deepEqual(paths(orderNav(NAV, [], [])), paths(NAV));
});

test("设置里列出的在前，没列出的按默认顺序排在后面", () => {
  const got = paths(orderNav(NAV, ["/log", "/download"], []));
  assert.deepEqual(got.slice(0, 2), ["/log", "/download"]);
  assert.deepEqual(got.slice(2), ["/", "/versions", "/resources", "/account", "/theme", "/settings"]);
});

test("设置里残留的已不存在的页面会被忽略", () => {
  assert.deepEqual(paths(orderNav(NAV, ["/gone", "/log"], [])).slice(0, 1), ["/log"]);
  assert.equal(orderNav(NAV, ["/gone"], []).length, NAV.length);
});

test("隐藏的页面去掉，但首页、主题中心、设置永远留着", () => {
  const got = paths(orderNav(NAV, [], ["/log", "/download", ...ALWAYS_VISIBLE]));
  assert.ok(!got.includes("/log") && !got.includes("/download"));
  for (const p of ALWAYS_VISIBLE) assert.ok(got.includes(p), `${p} 不能被隐藏`);
});

test("完整顺序包含新增页面，并和导航排序一致", () => {
  const all = paths(NAV);
  assert.deepEqual(fullOrder(all, ["/settings", "/"]), ["/settings", "/", ...all.filter((p) => !["/settings", "/"].includes(p))]);
  assert.deepEqual(fullOrder(all, []), all);
});

test("上移下移，越界不动", () => {
  assert.deepEqual(moveItem(["a", "b", "c"], 1, -1), ["b", "a", "c"]);
  assert.deepEqual(moveItem(["a", "b", "c"], 1, 1), ["a", "c", "b"]);
  const same = ["a", "b"];
  assert.equal(moveItem(same, 0, -1), same);
  assert.equal(moveItem(same, 1, 1), same);
  assert.equal(moveItem(same, 5, 1), same);
});

test("服务器地址：合格的", () => {
  for (const ok of ["", "  ", "mc.example.com", "mc.example.com:25565", "127.0.0.1:25566", "[::1]:25565", "[2001:db8::1]", "my_server-1.net"]) {
    assert.equal(validateServer(ok), null, JSON.stringify(ok));
  }
});

test("服务器地址：能往命令行里夹带东西的一律拒绝", () => {
  for (const bad of ["--server x", "a b", "a;b", "a&b", "$(x)", "`x`", "host:0", "host:70000", "host:", ":25565", "a..b", ".a", "a.", "::1", "[::1", "[]", "[abc]", "http://x", "x/y", "-evil"]) {
    assert.notEqual(validateServer(bad), null, JSON.stringify(bad));
  }
  assert.notEqual(validateServer("a".repeat(300)), null);
});

test("快捷按钮校验", () => {
  const q = (action, label = "按钮") => ({ id: "q1", label, icon: "", action });
  assert.equal(validateQuick(q({ type: "openFolder", path: "C:/mc" })), null);
  assert.equal(validateQuick(q({ type: "runProgram", path: "C:/tools/a.bat" })), null);
  assert.equal(validateQuick(q({ type: "openUrl", url: "https://example.com/a?b=1" })), null);
  assert.equal(validateQuick(q({ type: "launch", versionId: "1.21.1", server: "mc.example.com" })), null);
  assert.equal(validateQuick(q({ type: "launch", versionId: "1.21.1" })), null);

  assert.match(validateQuick(q({ type: "openFolder", path: "C:/mc" }, "  ")), /名字/);
  assert.match(validateQuick(q({ type: "openFolder", path: " " })), /文件夹/);
  assert.match(validateQuick(q({ type: "runProgram", path: "" })), /程序/);
  assert.match(validateQuick(q({ type: "launch", versionId: "" })), /版本/);
  assert.notEqual(validateQuick(q({ type: "launch", versionId: "1.21.1", server: "--bad" })), null);
  for (const url of ["javascript:alert(1)", "file:///C:/Windows/System32/cmd.exe", "ftp://x.com", "example.com", "https://user:pw@example.com", ""]) {
    assert.notEqual(validateQuick(q({ type: "openUrl", url })), null, url);
  }
});

test("动作说明，以及 id 不重复", () => {
  assert.equal(describeQuick({ type: "launch", versionId: "1.21.1", server: " mc.x.com " }), "启动 1.21.1 并进入 mc.x.com");
  assert.equal(describeQuick({ type: "launch", versionId: "1.21.1" }), "启动 1.21.1");
  assert.match(describeQuick({ type: "runProgram", path: "C:/a.bat" }), /运行 C:\/a\.bat/);
  const ids = new Set();
  const existing = [];
  for (let i = 0; i < 200; i++) {
    const id = newQuickId(existing);
    assert.ok(!ids.has(id));
    assert.match(id, /^[a-z0-9]+$/);
    ids.add(id);
    existing.push(id);
  }
});
