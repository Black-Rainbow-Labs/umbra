import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const src = (name) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, "src", name)).href);
const index = await src("index.mjs");
const files = await src("files.mjs");
const cache = await src("cache.mjs");
const tasks = await src("tasks.mjs");
const errors = await src("errors.mjs");

const unhandled = [];
process.on("unhandledRejection", (reason) => unhandled.push(reason));

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "asynckit-hidden-"));
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const timerCount = () => process.getActiveResourcesInfo().filter((name) => name === "Timeout").length;

/**
 * Asserts that a promise rejects with an instance of the class and returns the error.
 * @param promise - the promise
 * @param Class - the expected class
 */
async function rejectsWith(promise, Class) {
  assert.ok(promise instanceof Promise, "must return a Promise");
  try {
    await promise;
  } catch (error) {
    assert.ok(error instanceof Class, `expected ${Class.name}, got ${error && error.name}: ${error && error.message}`);
    return error;
  }
  assert.fail(`expected a rejection with ${Class.name}`);
}

test("R1 every Async function is exported from its module and from src/index.mjs", () => {
  const map = { files: [files, ["readJson", "writeJson"]], cache: [cache, ["cachePut", "cacheGet", "cacheClear"]], tasks: [tasks, ["withTimeout", "retry", "mapLimit"]] };
  for (const [label, [module, names]] of Object.entries(map)) {
    for (const name of names) {
      assert.equal(typeof module[name], "function", `${label} lost ${name}`);
      assert.equal(typeof module[`${name}Async`], "function", `${label} has no ${name}Async`);
      assert.equal(index[name], module[name], `index ${name}`);
      assert.equal(index[`${name}Async`], module[`${name}Async`], `index ${name}Async`);
    }
  }
  for (const name of ["AppError", "NotFoundError", "ParseError", "ValidationError", "TimeoutError"]) assert.equal(index[name], errors[name]);
});

test("R2 readJsonAsync and writeJsonAsync", async () => {
  const file = path.join(tmp, "x", "y", "data.json");
  assert.equal(await index.writeJsonAsync(file, { n: [1] }), undefined);
  assert.equal(fs.readFileSync(file, "utf8"), '{\n  "n": [\n    1\n  ]\n}\n');
  assert.deepEqual(await index.readJsonAsync(file), { n: [1] });
  const missing = await rejectsWith(index.readJsonAsync(path.join(tmp, "nope.json")), index.NotFoundError);
  assert.equal(missing.cause.code, "ENOENT");
  fs.writeFileSync(path.join(tmp, "bad.json"), "{oops");
  const parse = await rejectsWith(index.readJsonAsync(path.join(tmp, "bad.json")), index.ParseError);
  assert.ok(parse.cause instanceof SyntaxError);
  await rejectsWith(index.writeJsonAsync(path.join(tmp, "u.json"), undefined), index.ValidationError);
  const cycle = {};
  cycle.me = cycle;
  await rejectsWith(index.writeJsonAsync(path.join(tmp, "cycle.json"), cycle), index.ParseError);
  assert.equal(fs.existsSync(path.join(tmp, "cycle.json")), false);
  const directory = await index.readJsonAsync(tmp).catch((error) => error);
  assert.equal(directory.code, "EISDIR");
  assert.ok(!(directory instanceof index.NotFoundError));
});

test("R3 cachePutAsync, cacheGetAsync and cacheClearAsync", async () => {
  const dir = path.join(tmp, "cache");
  assert.equal(await index.cachePutAsync(dir, "a", { v: 1 }, 5000), undefined);
  await index.cachePutAsync(dir, "b.c-d_e", [1, 2], 5000);
  assert.deepEqual(await index.cacheGetAsync(dir, "a"), { v: 1 });
  assert.equal(await index.cacheGetAsync(dir, "missing"), undefined);
  fs.writeFileSync(path.join(dir, "old.json"), JSON.stringify({ value: 1, expiresAt: Date.now() - 1 }));
  assert.equal(await index.cacheGetAsync(dir, "old"), undefined);
  assert.equal(fs.existsSync(path.join(dir, "old.json")), false);
  for (const key of ["a/b", "", 5, "../x", undefined, "sp ace"]) await rejectsWith(index.cacheGetAsync(dir, key), index.ValidationError);
  for (const key of ["a/b", "", null]) await rejectsWith(index.cachePutAsync(dir, key, 1, 100), index.ValidationError);
  for (const ttl of [0, -1, "10", NaN, Infinity, undefined]) await rejectsWith(index.cachePutAsync(dir, "k", 1, ttl), index.ValidationError);
  fs.writeFileSync(path.join(dir, "keep.txt"), "x");
  assert.equal(await index.cacheClearAsync(dir), 2);
  assert.equal(fs.existsSync(path.join(dir, "keep.txt")), true);
  assert.equal(await index.cacheClearAsync(path.join(tmp, "no-such-cache")), 0);
});

test("R4 withTimeoutAsync", async () => {
  assert.equal(await index.withTimeoutAsync(async () => "fast", 500), "fast");
  const slow = await rejectsWith(index.withTimeoutAsync(() => sleep(200), 20), index.TimeoutError);
  assert.equal(slow.code, "E_TIMEOUT");
  assert.equal(slow.message, "timed out after 20 ms");
  const failure = new Error("boom");
  assert.equal(await index.withTimeoutAsync(() => Promise.reject(failure), 200).catch((e) => e), failure);
  assert.equal(await index.withTimeoutAsync(() => { throw failure; }, 200).catch((e) => e), failure);
  const before = timerCount();
  await index.withTimeoutAsync(() => sleep(1), 60000);
  assert.equal(timerCount(), before, "the timeout timer must be cleared");
  await index.withTimeoutAsync(() => sleep(60).then(() => Promise.reject(new Error("late"))), 10).catch(() => {});
  await sleep(120);
  assert.deepEqual(unhandled, []);
  await rejectsWith(index.withTimeoutAsync("nope", 100), index.ValidationError);
  await rejectsWith(index.withTimeoutAsync(() => 1, 0), index.ValidationError);
  await rejectsWith(index.withTimeoutAsync(() => 1, "5"), index.ValidationError);
});

test("R5 retryAsync", async () => {
  const seen = [];
  const result = await index.retryAsync(async (attempt) => {
    seen.push(attempt);
    if (attempt < 3) throw new Error(`fail ${attempt}`);
    return "ok";
  }, { attempts: 5 });
  assert.equal(result, "ok");
  assert.deepEqual(seen, [1, 2, 3]);
  const started = Date.now();
  let calls = 0;
  await index.retryAsync(async () => { calls += 1; if (calls < 3) throw new Error("x"); }, { attempts: 3, delayMs: 30 });
  assert.ok(Date.now() - started >= 50, "delayMs between attempts");
  const last = new Error("last");
  let count = 0;
  const caught = await index.retryAsync(async () => { count += 1; throw count === 2 ? last : new Error("early"); }, { attempts: 2 }).catch((e) => e);
  assert.equal(caught, last);
  let defaults = 0;
  await index.retryAsync(async () => { defaults += 1; throw new Error("always"); }).catch(() => {});
  assert.equal(defaults, 3);
  const bad = new index.ValidationError("nope");
  let validationCalls = 0;
  assert.equal(await index.retryAsync(async () => { validationCalls += 1; throw bad; }, { attempts: 4 }).catch((e) => e), bad);
  assert.equal(validationCalls, 1);
  for (const options of [{ attempts: 0 }, { attempts: 1.5 }, { delayMs: -1 }, { attempts: "3" }]) await rejectsWith(index.retryAsync(async () => 1, options), index.ValidationError);
  await rejectsWith(index.retryAsync("nope", {}), index.ValidationError);
});

test("R6 mapLimitAsync", async () => {
  let active = 0;
  let peak = 0;
  const calls = [];
  const out = await index.mapLimitAsync([30, 10, 20, 5, 15, 1], 3, async (item, position, ...rest) => {
    calls.push([position, rest.length]);
    active += 1;
    peak = Math.max(peak, active);
    await sleep(item);
    active -= 1;
    return item * 2;
  });
  assert.deepEqual(out, [60, 20, 40, 10, 30, 2]);
  assert.equal(peak, 3);
  assert.deepEqual(calls.map((call) => call[0]).sort(), [0, 1, 2, 3, 4, 5]);
  assert.ok(calls.every((call) => call[1] === 0), "the iteratee gets only (item, index)");
  assert.deepEqual(await index.mapLimitAsync([], 2, async () => 1), []);
  assert.deepEqual(await index.mapLimitAsync([1, 2], 10, async (x) => x + 1), [2, 3]);
  const started = [];
  const two = new Error("two");
  const failed = await index.mapLimitAsync([1, 2, 3, 4], 1, async (item) => { started.push(item); if (item === 2) throw two; return item; }).catch((e) => e);
  assert.equal(failed, two);
  await sleep(20);
  assert.deepEqual(started, [1, 2]);
  await index.mapLimitAsync([1, 2], 2, async (item) => { await sleep(item * 15); throw new Error(`e${item}`); }).catch(() => {});
  await sleep(60);
  assert.deepEqual(unhandled, []);
  await rejectsWith(index.mapLimitAsync([1], 0, async () => 1), index.ValidationError);
  await rejectsWith(index.mapLimitAsync("abc", 1, async () => 1), index.ValidationError);
  await rejectsWith(index.mapLimitAsync([1], 1, "nope"), index.ValidationError);
});

test("R7 the Async functions never throw synchronously", async () => {
  const calls = [
    [() => index.readJsonAsync(undefined), TypeError],
    [() => index.writeJsonAsync(undefined, {}), TypeError],
    [() => index.cachePutAsync(undefined, "k", 1, 10), TypeError],
    [() => index.cacheGetAsync(undefined, "k"), TypeError],
    [() => index.cacheClearAsync(undefined), TypeError],
    [() => index.withTimeoutAsync(), index.ValidationError],
    [() => index.retryAsync(), index.ValidationError],
    [() => index.mapLimitAsync(), index.ValidationError],
  ];
  for (const [call, Class] of calls) {
    let promise;
    assert.doesNotThrow(() => { promise = call(); }, "must not throw synchronously");
    await rejectsWith(promise, Class);
  }
});

test("R8 the callback forms still behave as before", async () => {
  const dir = path.join(tmp, "callbacks");
  const once = (fn) => new Promise((resolve) => fn((...args) => resolve(args)));
  assert.deepEqual(await once((cb) => files.writeJson(path.join(dir, "w.json"), 1, cb)), [null]);
  assert.deepEqual(await once((cb) => cache.cachePut(dir, "k", 7, 1000, cb)), [null]);
  assert.deepEqual(await once((cb) => cache.cacheGet(dir, "k", cb)), [null, 7]);
  assert.deepEqual(await once((cb) => cache.cacheGet(dir, "nope", cb)), [null, undefined]);
  assert.deepEqual(await once((cb) => cache.cacheClear(path.join(tmp, "absent"), cb)), [null, 0]);
  const [error, data] = await once((cb) => files.readJson(path.join(tmp, "absent.json"), cb));
  assert.ok(error instanceof errors.NotFoundError);
  assert.equal(data, undefined);
  const attempts = [];
  await once((cb) => tasks.retry((done, attempt) => { attempts.push(attempt); done(attempt < 2 ? new Error("x") : null, 1); }, { attempts: 3 }, cb));
  assert.deepEqual(attempts, [1, 2]);
  const second = await once((cb) => tasks.mapLimit([1, 2], 1, (item, position, done) => done(null, [item, position]), cb));
  assert.deepEqual(second, [null, [[1, 0], [2, 1]]]);
  let doubled = 0;
  const [timeoutError, value] = await once((cb) => tasks.withTimeout((done) => { done(null, "a"); done(null, "b"); }, 50, (...args) => { doubled += 1; cb(...args); }));
  await sleep(30);
  assert.equal(doubled, 1);
  assert.deepEqual([timeoutError, value], [null, "a"]);
  let sync = true;
  const asyncCheck = new Promise((resolve) => tasks.mapLimit([], 1, () => {}, () => resolve(sync)));
  sync = false;
  assert.equal(await asyncCheck, false, "callbacks must not run synchronously");
  const before = timerCount();
  await once((cb) => tasks.withTimeout((done) => done(null, 1), 60000, cb));
  assert.equal(timerCount(), before);
});

test("R9 rejections use the library error classes, codes and causes unchanged", async () => {
  const notFound = await index.readJsonAsync(path.join(tmp, "q.json")).catch((e) => e);
  const parse = await index.writeJsonAsync(path.join(tmp, "p.json"), (() => { const o = {}; o.o = o; return o; })()).catch((e) => e);
  const validation = await index.cacheGetAsync(tmp, "a/b").catch((e) => e);
  const timeout = await index.withTimeoutAsync(() => sleep(100), 10).catch((e) => e);
  for (const [error, Class, code, name] of [[notFound, errors.NotFoundError, "E_NOT_FOUND", "NotFoundError"], [parse, errors.ParseError, "E_PARSE", "ParseError"], [validation, errors.ValidationError, "E_VALIDATION", "ValidationError"], [timeout, errors.TimeoutError, "E_TIMEOUT", "TimeoutError"]]) {
    assert.equal(error.constructor, Class);
    assert.ok(error instanceof errors.AppError);
    assert.ok(error instanceof Error);
    assert.equal(error.code, code);
    assert.equal(error.name, name);
  }
  assert.equal(notFound.cause.code, "ENOENT");
  assert.ok(parse.cause instanceof TypeError);
  assert.equal(new errors.AppError("x").code, "E_APP");
});
