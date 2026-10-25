import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const moduleUrl = (source) => `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
const source = await readFile(new URL("../src/community.ts", import.meta.url), "utf8");

async function communityModule(native) {
  const aliases = new Map([
    ["./api", moduleUrl(`export const inTauri = ${native};`)],
    ["@tauri-apps/plugin-opener", moduleUrl(`
      export const openUrl = async (url) => { globalThis.__communityTestCalls.push(url); };
    `)],
  ]);
  const compiled = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext },
    transformers: {
      before: [(context) => (root) => ts.visitNode(root, function visit(node) {
        if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
          const alias = aliases.get(node.moduleSpecifier.text);
          if (alias) {
            return context.factory.updateImportDeclaration(
              node, node.modifiers, node.importClause, context.factory.createStringLiteral(alias), node.attributes,
            );
          }
        }
        return ts.visitEachChild(node, visit, context);
      })],
    },
  });
  return import(moduleUrl(compiled.outputText));
}

test("community resources contain only the two requested websites", async () => {
  const { communitySites } = await communityModule(true);
  assert.deepEqual(communitySites.map((site) => site.id), ["memorialResources", "redstone"]);
  assert.equal(communitySites[0].url, "https://www.mcbbs.co/portal.php?v3page=resources&type=modpack");
  assert.equal(communitySites[1].url, "https://www.mczwlt.net/");
});

test("desktop resource links always use the system-browser opener", async () => {
  globalThis.__communityTestCalls = [];
  const { browseCommunity } = await communityModule(true);
  await browseCommunity("memorialResources");
  await browseCommunity("redstone");
  assert.deepEqual(globalThis.__communityTestCalls, [
    "https://www.mcbbs.co/portal.php?v3page=resources&type=modpack",
    "https://www.mczwlt.net/",
  ]);
});

test("browser previews use safe external links without desktop IPC", async () => {
  const original = globalThis.window;
  const calls = [];
  globalThis.window = { open: (...args) => { calls.push(args); } };
  globalThis.__communityTestCalls = [];
  try {
    const { browseCommunity } = await communityModule(false);
    await browseCommunity("redstone");
    assert.deepEqual(calls, [["https://www.mczwlt.net/", "_blank", "noopener,noreferrer"]]);
    assert.deepEqual(globalThis.__communityTestCalls, []);
  } finally {
    if (original === undefined) delete globalThis.window;
    else globalThis.window = original;
  }
});

test("removed or unknown destinations cannot open another website", async () => {
  globalThis.__communityTestCalls = [];
  const { browseCommunity } = await communityModule(true);
  for (const site of ["forum", "klpbbs", "file:///private"]) {
    await assert.rejects(browseCommunity(site), /未知社区/);
  }
  assert.deepEqual(globalThis.__communityTestCalls, []);
});
