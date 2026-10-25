import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createPinia, setActivePinia } from "pinia";
import ts from "typescript";

const moduleUrl = (source) => `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
const aliases = new Map([
  ["pinia", import.meta.resolve("pinia")],
  ["vue", import.meta.resolve("vue")],
  ["../api", moduleUrl(`
    export const api = {
      launchGame: (...args) => globalThis.__gameTestBridge.launchGame(...args),
      killGame: (...args) => globalThis.__gameTestBridge.killGame(...args),
    };
    export const onGameLog = (callback) => { globalThis.__gameTestBridge.log = callback; };
    export const onGameExit = (callback) => { globalThis.__gameTestBridge.exit = callback; };
  `)],
  ["./versions", moduleUrl(`
    export const useVersionsStore = () => globalThis.__gameTestBridge.versions;
  `)],
]);

// Exercise the production store with real Pinia and mocked IPC, without a browser or extra test packages.
const source = await readFile(new URL("../src/stores/game.ts", import.meta.url), "utf8");
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
const { useGameStore } = await import(moduleUrl(compiled.outputText));

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function setup() {
  const pending = deferred();
  const jobs = [];
  const dismissed = [];
  const killed = [];
  const bridge = {
    launchGame: () => pending.promise,
    killGame: async (runId) => { killed.push(runId); },
    versions: {
      trackLaunch(versionId) {
        const job = { versionId, status: "running" };
        jobs.push(job);
        return job;
      },
      dismiss: (versionId) => { dismissed.push(versionId); },
    },
  };
  globalThis.__gameTestBridge = bridge;
  setActivePinia(createPinia());
  return { store: useGameStore(), bridge, pending, jobs, dismissed, killed };
}

const result = (runId = "run") => ({ runId, versionId: "test-version", player: "Steve" });
const exit = (runId = "run", patch = {}) => ({
  runId, versionId: "test-version", code: 0, killed: false, analysis: null, ...patch,
});

test("preserves an exit and its diagnostics arriving before launch returns", async () => {
  const { store, bridge, pending, dismissed } = setup();
  const launching = store.launch("test-version");
  const analysis = { exitCode: 1, findings: [], files: ["crash.txt"] };
  bridge.log({ runId: "run", line: "first line" });
  bridge.exit(exit("run", { code: 1, analysis }));
  pending.resolve(result());
  assert.equal(await launching, true);
  assert.equal(store.runs.run.status, "exited");
  assert.equal(store.runs.run.code, 1);
  assert.deepEqual(store.runs.run.analysis, analysis);
  assert.deepEqual(store.runs.run.lines, ["first line"]);
  assert.equal(store.running.length, 0);
  assert.deepEqual(dismissed, ["test-version"]);
});

test("bounds logs both before and after the launch response", async () => {
  const { store, bridge, pending } = setup();
  const launching = store.launch("test-version");
  for (let i = 0; i < 5010; i++) bridge.log({ runId: "run", line: `early-${i}` });
  pending.resolve(result());
  await launching;
  assert.equal(store.runs.run.lines.length, 5000);
  assert.equal(store.runs.run.lines[0], "early-10");
  bridge.log({ runId: "run", line: "late" });
  assert.equal(store.runs.run.lines.length, 5000);
  assert.equal(store.runs.run.lines[0], "early-11");
  assert.equal(store.runs.run.lines[4999], "late");
});

test("handles normal and manually killed exits after registration", async () => {
  const { store, bridge, pending, killed } = setup();
  const launching = store.launch("test-version");
  pending.resolve(result());
  await launching;
  assert.equal(store.running.length, 1);
  await store.kill("run");
  assert.deepEqual(killed, ["run"]);
  bridge.exit(exit("run", { code: null, killed: true }));
  assert.equal(store.runs.run.status, "exited");
  assert.equal(store.runs.run.killed, true);
  assert.equal(store.running.length, 0);
});

test("failed launches discard buffered events and allow another launch", async () => {
  const { store, bridge, pending, jobs } = setup();
  const launching = store.launch("test-version");
  bridge.log({ runId: "run", line: "stale" });
  bridge.exit(exit("run", { code: 1 }));
  pending.reject(new Error("launch failed"));
  assert.equal(await launching, false);
  assert.equal(store.launching, null);
  assert.equal(jobs[0].status, "failed");
  assert.match(store.error, /launch failed/);

  bridge.launchGame = async () => result();
  assert.equal(await store.launch("test-version"), true);
  assert.equal(store.runs.run.status, "running");
  assert.deepEqual(store.runs.run.lines, []);
});

test("does not accept a second concurrent launch", async () => {
  const { store, pending, jobs } = setup();
  const first = store.launch("test-version");
  assert.equal(await store.launch("another-version"), false);
  assert.equal(jobs.length, 1);
  pending.resolve(result());
  assert.equal(await first, true);
});

test("late events from a cleared run do not contaminate subsequent launches", async () => {
  const { store, bridge, pending } = setup();
  const launching = store.launch("test-version");
  pending.resolve(result());
  await launching;
  bridge.exit(exit());
  store.clear("run");
  bridge.log({ runId: "run", line: "stale" });
  bridge.exit(exit("run", { code: 1 }));
  bridge.launchGame = async () => result();
  await store.launch("test-version");
  assert.equal(store.runs.run.status, "running");
  assert.deepEqual(store.runs.run.lines, []);
});
