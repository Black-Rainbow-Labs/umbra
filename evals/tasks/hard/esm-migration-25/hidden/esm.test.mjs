// Hidden tests for esm-migration-25. WORKSPACE_DIR is the migrated project.

import test from "node:test";
import assert from "node:assert";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

const WS = process.env.WORKSPACE_DIR;

/** Imports a workspace module by its path relative to the workspace. */
async function load(relative) {
  return import(pathToFileURL(path.join(WS, relative)).href);
}

/** Returns the sorted export names of a loaded module. */
function names(ns) {
  return Object.keys(ns).sort();
}

/** Lists every file under a folder, relative with forward slashes, skipping node_modules. */
function walk(dir, base = dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(full, base));
    else out.push(path.relative(base, full).split(path.sep).join("/"));
  }
  return out;
}

const SRC = [
  "constants", "strings", "numbers", "arrays", "objects", "dates", "validators", "logger", "ids", "cache", "config", "paths",
  "fileStore", "events", "tree", "walker", "plugins/upper", "plugins/reverse", "pluginLoader", "slug", "csv", "retry", "memo", "stats", "index",
  "semver", "limits", "registry", "legacy", "format", "ring/a", "ring/b", "ring/c", "plugins/title",
];

test("every module exists with the .mjs extension and no .js source is left", () => {
  for (const name of SRC) assert.ok(fs.existsSync(path.join(WS, "src", `${name}.mjs`)), `src/${name}.mjs is missing`);
  for (const file of ["bin/cli.mjs", "run-tests.mjs", "test/harness.mjs", "test/extra.test.mjs", "test/plugins.test.mjs"]) assert.ok(fs.existsSync(path.join(WS, file)), `${file} is missing`);
  const leftovers = walk(WS).filter((file) => file.endsWith(".js"));
  assert.deepStrictEqual(leftovers, []);
});

test("no CommonJS syntax is left in any .mjs file", () => {
  const bad = [];
  for (const file of walk(WS).filter((f) => f.endsWith(".mjs"))) {
    const text = fs.readFileSync(path.join(WS, file), "utf8");
    if (/\brequire\s*\(|\bmodule\.exports\b|\b__dirname\b|\b__filename\b|(^|[^.\w])exports\.\w+\s*=/m.test(text)) bad.push(file);
  }
  assert.deepStrictEqual(bad, []);
});

test("package.json keeps CommonJS as the package type and runs the .mjs test runner", () => {
  const pkg = JSON.parse(fs.readFileSync(path.join(WS, "package.json"), "utf8"));
  assert.notStrictEqual(pkg.type, "module");
  assert.strictEqual(pkg.scripts.test, "node run-tests.mjs");
});

test("constants, strings, numbers, arrays and objects export the same names and behave the same", async () => {
  const c = await load("src/constants.mjs");
  assert.deepStrictEqual(names(c), ["DEFAULT_LOCALE", "MAX_ITEMS", "VERSION"]);
  assert.strictEqual(c.VERSION, "2.4.0");
  const s = await load("src/strings.mjs");
  assert.deepStrictEqual(names(s), ["capitalize", "padLeft", "truncate"]);
  assert.strictEqual(s.padLeft(5, 4, "0"), "0005");
  assert.strictEqual(s.truncate("abcdefghij", 8), "abcde...");
  const n = await load("src/numbers.mjs");
  assert.deepStrictEqual(names(n), ["average", "clamp", "sum"]);
  assert.strictEqual(n.clamp(15, 0, 10), 10);
  assert.strictEqual(n.average([]), 0);
  const a = await load("src/arrays.mjs");
  assert.deepStrictEqual(names(a), ["chunk", "flatten", "unique"]);
  assert.deepStrictEqual(a.chunk([1, 2, 3], 0), [[1], [2], [3]]);
  assert.deepStrictEqual(a.chunk([1, 2, 3], 99), [[1, 2, 3]]);
  const o = await load("src/objects.mjs");
  assert.deepStrictEqual(names(o), ["deepMerge", "omit", "pick"]);
  assert.deepStrictEqual(o.deepMerge({ a: { b: 1, c: 2 }, d: 1 }, { a: { c: 3 } }), { a: { b: 1, c: 3 }, d: 1 });
  assert.deepStrictEqual(o.omit({ a: 1, b: 2 }, ["a"]), { b: 2 });
});

test("dates and validators", async () => {
  const d = await load("src/dates.mjs");
  assert.deepStrictEqual(names(d), ["addDays", "formatDate"]);
  assert.strictEqual(d.formatDate(d.addDays(new Date(Date.UTC(2024, 1, 27)), 3)), "2024-03-01");
  const v = await load("src/validators.mjs");
  assert.deepStrictEqual(names(v), ["isEmail", "isNonEmptyString", "isPositiveInt"]);
  assert.strictEqual(v.isEmail("a@b.co"), true);
  assert.strictEqual(v.isEmail("a@b"), false);
  assert.strictEqual(v.isPositiveInt(0), false);
});

test("logger is a default export that writes to the sink", async () => {
  const l = await load("src/logger.mjs");
  assert.deepStrictEqual(names(l), ["default"]);
  const lines = [];
  const log = l.default("app", (line) => lines.push(line));
  log.info("up");
  log.warn("slow");
  assert.deepStrictEqual(lines, ["app INFO up", "app WARN slow"]);
});

test("ids is a default export with a counter shared by the module", async () => {
  const i = await load("src/ids.mjs");
  assert.deepStrictEqual(names(i), ["default"]);
  const first = i.default("x");
  const second = i.default("x");
  assert.match(first, /^x-\d+$/);
  assert.strictEqual(Number(second.split("-")[1]), Number(first.split("-")[1]) + 1);
});

test("cache is a default export class with the same eviction behavior", async () => {
  const k = await load("src/cache.mjs");
  assert.deepStrictEqual(names(k), ["default"]);
  const cache = new k.default(2);
  assert.match(cache.id, /^cache-\d+$/);
  cache.set("a", 1).set("b", 2);
  cache.get("a");
  cache.set("c", 3);
  assert.strictEqual(cache.get("b"), undefined);
  assert.strictEqual(cache.get("a"), 1);
  assert.strictEqual(cache.size, 2);
});

test("config still reads data/defaults.json", async () => {
  const c = await load("src/config.mjs");
  assert.deepStrictEqual(names(c), ["getConfig", "getDefaults"]);
  const merged = c.getConfig({ features: { beta: true } });
  assert.deepStrictEqual(merged, { server: { host: "localhost", port: 8080 }, features: { beta: true, retries: 3 } });
  const defaults = c.getDefaults();
  defaults.server.port = 1;
  assert.strictEqual(c.getDefaults().server.port, 8080);
});

test("paths resolves the src and data folders and reports its own file name", async () => {
  const p = await load("src/paths.mjs");
  assert.deepStrictEqual(names(p), ["dataDir", "dataPath", "srcDir", "thisFile"]);
  assert.strictEqual(fs.realpathSync(p.srcDir()), fs.realpathSync(path.join(WS, "src")));
  assert.strictEqual(fs.realpathSync(p.dataDir()), fs.realpathSync(path.join(WS, "data")));
  assert.strictEqual(fs.realpathSync(p.dataPath("defaults.json")), fs.realpathSync(path.join(WS, "data", "defaults.json")));
  assert.strictEqual(p.thisFile(), "paths.mjs");
});

test("fileStore writes JSON with 2 space indent and a trailing newline", async () => {
  const f = await load("src/fileStore.mjs");
  assert.deepStrictEqual(names(f), ["readJsonFile", "writeJsonFile"]);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "esm-hidden-"));
  try {
    const file = path.join(dir, "nested", "out.json");
    f.writeJsonFile(file, { a: [1, 2] });
    assert.strictEqual(fs.readFileSync(file, "utf8"), '{\n  "a": [\n    1,\n    2\n  ]\n}\n');
    assert.deepStrictEqual(f.readJsonFile(file), { a: [1, 2] });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("events bus", async () => {
  const e = await load("src/events.mjs");
  assert.deepStrictEqual(names(e), ["createBus"]);
  const bus = e.createBus();
  const seen = [];
  const handler = (x) => seen.push(x);
  bus.on("hit", handler);
  assert.strictEqual(bus.emit("hit", 1), 1);
  bus.off("hit", handler);
  assert.strictEqual(bus.emit("hit", 2), 0);
  assert.deepStrictEqual(seen, [1]);
});

test("tree and walker import each other and both work whichever loads first", async () => {
  const code = `
    const order = process.argv[1] === "walker" ? ["walker", "tree"] : ["tree", "walker"];
    const root = new URL(process.argv[2]);
    const first = await import(new URL("src/" + order[0] + ".mjs", root).href);
    const second = await import(new URL("src/" + order[1] + ".mjs", root).href);
    const tree = order[0] === "tree" ? first : second;
    const walker = order[0] === "tree" ? second : first;
    const t = tree.node(1, [tree.node(2, [tree.node(4)]), tree.node(3)]);
    console.log(JSON.stringify([tree.depth(t), tree.values(t), walker.walk(t), walker.maxDepth(t), Object.keys(tree).sort(), Object.keys(walker).sort()]));
  `;
  for (const first of ["tree", "walker"]) {
    const run = spawnSync("node", ["--input-type=module", "-e", code, first, pathToFileURL(WS + path.sep).href], { encoding: "utf8", timeout: 30000 });
    assert.strictEqual(run.status, 0, run.stderr);
    assert.strictEqual(run.stdout.trim(), JSON.stringify([3, [1, 2, 4, 3], [1, 2, 4, 3], 3, ["depth", "node", "values"], ["maxDepth", "walk"]]));
  }
});

test("plugins are default exports and pluginLoader loads them asynchronously", async () => {
  const up = await load("src/plugins/upper.mjs");
  const rev = await load("src/plugins/reverse.mjs");
  assert.deepStrictEqual(names(up), ["default"]);
  assert.deepStrictEqual(names(rev), ["default"]);
  assert.strictEqual(up.default("abc"), "ABC");
  assert.strictEqual(rev.default("abc"), "cba");
  const loader = await load("src/pluginLoader.mjs");
  assert.deepStrictEqual(names(loader), ["listPlugins", "loadPlugin"]);
  assert.deepStrictEqual(loader.listPlugins(), ["reverse", "title", "upper"]);
  const pending = loader.loadPlugin("upper");
  assert.ok(pending instanceof Promise);
  assert.strictEqual((await pending)("xyz"), "XYZ");
  const title = await load("src/plugins/title.mjs");
  assert.deepStrictEqual(names(title), ["default"]);
  assert.strictEqual(title.default("hello big world"), "Hello Big World");
  assert.strictEqual((await loader.loadPlugin("title"))("a b"), "A B");
  await assert.rejects(loader.loadPlugin("missing"), { name: "Error", message: "unknown plugin: missing" });
  await assert.rejects(loader.loadPlugin("../paths"), { message: "unknown plugin: ../paths" });
  await assert.rejects(loader.loadPlugin("upper/../reverse"), { message: "unknown plugin: upper/../reverse" });
});

test("slug, csv, retry, memo and stats", async () => {
  const slug = await load("src/slug.mjs");
  assert.deepStrictEqual(names(slug), ["default"]);
  assert.strictEqual(slug.default("  Many   Words -- Here!! "), "many-words-here");
  const csv = await load("src/csv.mjs");
  assert.deepStrictEqual(names(csv), ["parseCsv", "toCsv"]);
  assert.deepStrictEqual(csv.parseCsv("a,b\n1,2\n3\n"), [{ a: "1", b: "2" }, { a: "3", b: "" }]);
  assert.strictEqual(csv.toCsv([{ a: 1, b: 2 }, { a: 3 }], ["a", "b"]), "a,b\n1,2\n3,\n");
  const r = await load("src/retry.mjs");
  assert.deepStrictEqual(names(r), ["retry"]);
  let calls = 0;
  const value = await r.retry(async (attempt) => { calls += 1; if (attempt < 3) throw new Error("no"); return attempt; }, 5);
  assert.deepStrictEqual([value, calls], [3, 3]);
  await assert.rejects(r.retry(async () => { throw new Error("always"); }, 2), /always/);
  const m = await load("src/memo.mjs");
  assert.deepStrictEqual(names(m), ["default"]);
  let runs = 0;
  const double = m.default((x) => { runs += 1; return x * 2; });
  assert.deepStrictEqual([double(2), double(2), double(3), runs, double.size()], [4, 4, 6, 2, 2]);
  const st = await load("src/stats.mjs");
  assert.deepStrictEqual(names(st), ["summarize"]);
  assert.deepStrictEqual(st.summarize([1, 1, 4]), { count: 3, sum: 6, average: 2, distinct: 2 });
});

test("index re-exports the namespaces and VERSION", async () => {
  const idx = await load("src/index.mjs");
  assert.deepStrictEqual(names(idx), ["VERSION", "arrays", "numbers", "objects", "stats", "strings"]);
  assert.strictEqual(idx.VERSION, "2.4.0");
  assert.deepStrictEqual(names(idx.strings), ["capitalize", "padLeft", "truncate"]);
  assert.strictEqual(idx.stats.summarize([2, 4]).average, 3);
});

test("bin/cli.mjs runs when started directly and stays quiet when imported", async () => {
  const cli = path.join(WS, "bin", "cli.mjs");
  const sum = spawnSync("node", [cli, "sum", "1", "2", "3.5"], { cwd: WS, encoding: "utf8", timeout: 30000 });
  assert.deepStrictEqual([sum.status, sum.stdout], [0, "6.5\n"]);
  const slug = spawnSync("node", [cli, "slug", "Hello", "Big World"], { cwd: WS, encoding: "utf8", timeout: 30000 });
  assert.strictEqual(slug.stdout, "hello-big-world\n");
  const version = spawnSync("node", [cli, "version"], { cwd: WS, encoding: "utf8", timeout: 30000 });
  assert.strictEqual(version.stdout, "2.4.0\n");
  const bad = spawnSync("node", [cli, "nope"], { cwd: WS, encoding: "utf8", timeout: 30000 });
  assert.deepStrictEqual([bad.status, bad.stdout, bad.stderr], [2, "", "unknown command: nope\n"]);
  const quiet = spawnSync("node", ["--input-type=module", "-e", `const m = await import(${JSON.stringify(pathToFileURL(cli).href)}); console.log(Object.keys(m).join(","));`], { cwd: WS, encoding: "utf8", timeout: 30000 });
  assert.strictEqual(quiet.stdout, "main,mainAsync\n");
  const mod = await load("bin/cli.mjs");
  assert.deepStrictEqual(mod.main(["sum", "2", "2"]), { code: 0, out: "4" });
  assert.deepStrictEqual(mod.main(["plugins"]), { code: 2, out: "unknown command: plugins" });
});

test("node run-tests.mjs runs the converted tests and they pass", () => {
  const run = spawnSync("node", ["run-tests.mjs"], { cwd: WS, encoding: "utf8", timeout: 45000 });
  assert.strictEqual(run.status, 0, `${run.stdout}${run.stderr}`);
  const summary = /^passed (\d+), failed 0$/m.exec(run.stdout);
  assert.ok(summary, "summary line is missing");
  assert.ok(Number(summary[1]) >= 24, `only ${summary[1]} tests ran`);
  assert.strictEqual((run.stdout.match(/^ok - /gm) || []).length, Number(summary[1]));
  assert.strictEqual(/^not ok/m.test(run.stdout), false);
});

test("the converted test files exist, use the harness and keep all 21 tests", () => {
  const files = ["strings.test.mjs", "arrays.test.mjs", "misc.test.mjs", "extra.test.mjs"];
  let total = 0;
  for (const file of files) {
    const text = fs.readFileSync(path.join(WS, "test", file), "utf8");
    assert.match(text, /harness\.mjs/);
    total += (text.match(/^test\(/gm) || []).length;
  }
  assert.strictEqual(total, 21);
});

test("the new test/plugins.test.mjs has at least 3 tests, uses the harness and imports the new code", () => {
  const text = fs.readFileSync(path.join(WS, "test", "plugins.test.mjs"), "utf8");
  assert.match(text, /harness\.mjs/);
  assert.ok((text.match(/^test\(/gm) || []).length >= 3);
  assert.match(text, /pluginLoader\.mjs/);
  assert.match(text, /mainAsync/);
});

test("semver is a default export with parse and satisfies as named exports", async () => {
  const v = await load("src/semver.mjs");
  assert.deepStrictEqual(names(v), ["default", "parse", "satisfies"]);
  assert.strictEqual(typeof v.default, "function");
  assert.deepStrictEqual([v.default("1.2.3", "1.10.0"), v.default("2.0.0", "2.0.0"), v.default("3.0.1", "3.0.0")], [-1, 0, 1]);
  assert.deepStrictEqual(v.parse("2.0.11"), { major: 2, minor: 0, patch: 11 });
  assert.strictEqual(v.satisfies("1.4.0", "1.3.9"), true);
  assert.strictEqual(v.satisfies("1.3.0", "1.3.1"), false);
  assert.throws(() => v.parse("1.2"), /bad version: 1\.2/);
});

test("limits reads data/limits.json without changing it", async () => {
  const before = fs.readFileSync(path.join(WS, "data", "limits.json"), "utf8");
  assert.strictEqual(before, '{\n  "maxRows": 100,\n  "maxCols": 20,\n  "timeoutMs": 3000\n}\n');
  const l = await load("src/limits.mjs");
  assert.deepStrictEqual(names(l), ["getLimit", "limitNames"]);
  assert.deepStrictEqual([l.getLimit("maxCols"), l.getLimit("timeoutMs")], [20, 3000]);
  assert.deepStrictEqual(l.limitNames(), ["maxCols", "maxRows", "timeoutMs"]);
  assert.throws(() => l.getLimit("nope"), (error) => error instanceof RangeError && error.message === "no limit: nope");
});

test("registry creates its bus lazily and reset starts a new one", async () => {
  const r = await load("src/registry.mjs");
  assert.deepStrictEqual(names(r), ["fire", "register", "reset"]);
  assert.strictEqual(r.fire("none"), 0);
  const seen = [];
  r.register("e", (x) => seen.push(x));
  r.register("e", (x) => seen.push(x * 10));
  assert.strictEqual(r.fire("e", 2), 2);
  r.reset();
  assert.strictEqual(r.fire("e", 3), 0);
  assert.deepStrictEqual(seen, [2, 20]);
});

test("legacy keeps its octal constant and its counter now that modules are strict", async () => {
  const l = await load("src/legacy.mjs");
  assert.deepStrictEqual(names(l), ["FILE_MODE", "nextTicket"]);
  assert.strictEqual(l.FILE_MODE, 493);
  assert.deepStrictEqual([l.nextTicket(), l.nextTicket(), l.nextTicket()], [1, 2, 3]);
  assert.strictEqual("ticketCount" in globalThis, false);
});

test("format has named exports only", async () => {
  const f = await load("src/format.mjs");
  assert.deepStrictEqual(names(f), ["money", "percent"]);
  assert.deepStrictEqual([f.money(5), f.money(1205), f.money(100000)], ["$0.05", "$12.05", "$1000.00"]);
  assert.deepStrictEqual([f.percent(1, 3), f.percent(2, 3), f.percent(5, 0)], ["33%", "67%", "0%"]);
});

test("the three ring modules import each other and work whichever one loads first", () => {
  const code = `
    const root = new URL(process.argv[2]);
    const order = process.argv[1].split("");
    const loaded = {};
    for (const name of order) loaded[name] = await import(new URL("src/ring/" + name + ".mjs", root).href);
    console.log(JSON.stringify([loaded.a.a(2), loaded.b.b(0), loaded.c.c(1), Object.keys(loaded.a), Object.keys(loaded.b), Object.keys(loaded.c)]));
  `;
  for (const order of ["abc", "bca", "cab", "acb"]) {
    const run = spawnSync("node", ["--input-type=module", "-e", code, order, pathToFileURL(WS + path.sep).href], { encoding: "utf8", timeout: 30000 });
    assert.strictEqual(run.status, 0, run.stderr);
    assert.strictEqual(run.stdout.trim(), JSON.stringify(["abcabcabc", "bc", "cabc", ["a"], ["b"], ["c"]]), order);
  }
});

test("bin/cli.mjs mainAsync handles the plugin commands", async () => {
  const mod = await load("bin/cli.mjs");
  assert.deepStrictEqual(await mod.mainAsync(["plugins"]), { code: 0, out: "reverse\ntitle\nupper" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "upper", "hello", "world"]), { code: 0, out: "HELLO WORLD" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "title", "hello", "big", "world"]), { code: 0, out: "Hello Big World" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "reverse", "abc"]), { code: 0, out: "cba" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "nope", "x"]), { code: 2, out: "unknown plugin: nope" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "../paths", "x"]), { code: 2, out: "unknown plugin: ../paths" });
  assert.deepStrictEqual(await mod.mainAsync(["apply", "upper"]), { code: 2, out: "usage: apply <plugin> <text>" });
  assert.deepStrictEqual(await mod.mainAsync(["apply"]), { code: 2, out: "usage: apply <plugin> <text>" });
  assert.deepStrictEqual(await mod.mainAsync(["sum", "1", "2"]), { code: 0, out: "3" });
  assert.deepStrictEqual(await mod.mainAsync(["bogus"]), { code: 2, out: "unknown command: bogus" });
});

test("bin/cli.mjs runs the plugin commands from the command line", () => {
  const cli = path.join(WS, "bin", "cli.mjs");
  const run = (...args) => spawnSync("node", [cli, ...args], { cwd: WS, encoding: "utf8", timeout: 30000 });
  const list = run("plugins");
  assert.deepStrictEqual([list.status, list.stdout], [0, "reverse\ntitle\nupper\n"]);
  const apply = run("apply", "title", "hello", "big", "world");
  assert.deepStrictEqual([apply.status, apply.stdout], [0, "Hello Big World\n"]);
  const missing = run("apply", "nope", "x");
  assert.deepStrictEqual([missing.status, missing.stdout, missing.stderr], [2, "", "unknown plugin: nope\n"]);
  const usage = run("apply", "upper");
  assert.deepStrictEqual([usage.status, usage.stderr], [2, "usage: apply <plugin> <text>\n"]);
  const sum = run("sum", "4", "5");
  assert.deepStrictEqual([sum.status, sum.stdout], [0, "9\n"]);
});
