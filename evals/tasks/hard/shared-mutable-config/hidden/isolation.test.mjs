import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspace = process.env.WORKSPACE_DIR;
const lib = await import(pathToFileURL(path.join(workspace, "src", "index.mjs")).href);
const { DEFAULTS, getDefaults, buildConfig, withAuth, enableFeature, planRetries, createService } = lib;

const ORIGINAL = { timeout: 30, retries: { max: 3, backoff: [100, 200, 400] }, headers: { accept: "application/json" }, features: { beta: false, tags: [] } };

/**
 * Collects every object and array reachable from a value.
 * @param value - any value
 * @param found - the set to fill
 */
function collectRefs(value, found = new Set()) {
  if (value !== null && typeof value === "object" && !found.has(value)) {
    found.add(value);
    Object.values(value).forEach((inner) => collectRefs(inner, found));
  }
  return found;
}

/**
 * Returns the objects and arrays that two values have in common.
 * @param a - one value
 * @param b - another value
 */
function sharedRefs(a, b) {
  const left = collectRefs(a);
  return [...collectRefs(b)].filter((ref) => left.has(ref));
}

/**
 * Walks a value and asserts every object and array in it is frozen.
 * @param value - the value to check
 * @param label - a name for failure messages
 */
function assertDeeplyFrozen(value, label) {
  if (value !== null && typeof value === "object") {
    assert.ok(Object.isFrozen(value), `${label} is not frozen`);
    for (const [key, inner] of Object.entries(value)) assertDeeplyFrozen(inner, `${label}.${key}`);
  }
}

/**
 * Builds the config a request should get, computed independently of the library.
 * @param request - { overrides, token, features }
 */
function expectedConfig(request) {
  const config = structuredClone(ORIGINAL);
  const overrides = request.overrides ?? {};
  if (overrides.retries?.max !== undefined) config.retries.max = overrides.retries.max;
  if (overrides.retries?.backoff !== undefined) config.retries.backoff = structuredClone(overrides.retries.backoff);
  if (overrides.timeout !== undefined) config.timeout = overrides.timeout;
  if (overrides.headers) Object.assign(config.headers, overrides.headers);
  if (request.token) config.headers.authorization = `Bearer ${request.token}`;
  for (const name of request.features ?? []) {
    if (name === "beta") config.features.beta = true;
    if (!config.features.tags.includes(name)) config.features.tags.push(name);
  }
  const last = config.retries.backoff[config.retries.backoff.length - 1] ?? 0;
  config.retries.schedule = Array.from({ length: config.retries.max }, (_, i) => config.retries.backoff[i] ?? last);
  return config;
}

test("DEFAULTS is deeply frozen and mutation throws", () => {
  assert.deepEqual(DEFAULTS, ORIGINAL);
  assertDeeplyFrozen(DEFAULTS, "DEFAULTS");
  assert.throws(() => { DEFAULTS.headers.authorization = "x"; }, TypeError);
  assert.throws(() => { DEFAULTS.features.tags.push("x"); }, TypeError);
  assert.throws(() => { DEFAULTS.retries.backoff.length = 0; }, TypeError);
});

test("getDefaults returns a fresh unfrozen copy each time", () => {
  const first = getDefaults();
  const second = getDefaults();
  assert.deepEqual(first, ORIGINAL);
  assert.notEqual(first, second);
  assert.deepEqual(sharedRefs(first, second), []);
  assert.deepEqual(sharedRefs(first, DEFAULTS), []);
  first.headers.authorization = "x";
  first.features.tags.push("x");
  first.retries.backoff.push(1);
  assert.deepEqual(getDefaults(), ORIGINAL);
  assert.deepEqual(DEFAULTS, ORIGINAL);
});

test("buildConfig without overrides is a copy that shares nothing with DEFAULTS", () => {
  for (const overrides of [undefined, {}, null]) {
    const config = buildConfig(overrides);
    assert.deepEqual(config, ORIGINAL);
    assert.deepEqual(sharedRefs(config, DEFAULTS), []);
    config.headers.authorization = "x";
    config.features.tags.push("x");
    config.retries.backoff.pop();
    assert.deepEqual(buildConfig(overrides), ORIGINAL);
  }
});

test("buildConfig merges plain objects deeply and replaces arrays and primitives", () => {
  assert.deepEqual(buildConfig({ retries: { max: 5 } }).retries, { max: 5, backoff: [100, 200, 400] });
  assert.deepEqual(buildConfig({ headers: { "x-id": "7" } }).headers, { accept: "application/json", "x-id": "7" });
  assert.deepEqual(buildConfig({ retries: { backoff: [1] } }).retries, { max: 3, backoff: [1] });
  assert.deepEqual(buildConfig({ features: { tags: ["a", "b"] } }).features, { beta: false, tags: ["a", "b"] });
  assert.equal(buildConfig({ timeout: 5 }).timeout, 5);
  assert.equal(buildConfig({ timeout: undefined }).timeout, 30);
  assert.equal(buildConfig({ timeout: null }).timeout, null);
  assert.deepEqual(buildConfig({ region: { name: "eu" } }).region, { name: "eu" });
  assert.deepEqual(DEFAULTS, ORIGINAL);
});

test("buildConfig neither changes nor keeps references to the overrides", () => {
  const overrides = { retries: { backoff: [5, 6] }, features: { tags: ["a"] }, headers: { "x-id": "1" }, region: { names: ["eu"] } };
  const snapshot = structuredClone(overrides);
  const config = buildConfig(overrides);
  assert.deepEqual(overrides, snapshot);
  assert.deepEqual(sharedRefs(config, overrides), []);
  config.features.tags.push("b");
  config.region.names.push("us");
  assert.deepEqual(overrides, snapshot);
  overrides.retries.backoff.push(7);
  assert.deepEqual(config.retries.backoff, [5, 6]);
});

test("withAuth returns a new config and leaves its argument alone", () => {
  const input = buildConfig();
  const snapshot = structuredClone(input);
  const output = withAuth(input, "tok");
  assert.deepEqual(input, snapshot);
  assert.notEqual(output, input);
  assert.equal(output.headers.authorization, "Bearer tok");
  assert.deepEqual({ ...output, headers: { accept: output.headers.accept } }, snapshot);
  assert.deepEqual(sharedRefs(input, output), []);
  for (const token of [undefined, "", null]) {
    const plain = withAuth(input, token);
    assert.deepEqual(plain, snapshot);
    assert.deepEqual(sharedRefs(input, plain), []);
  }
});

test("enableFeature returns a new config, adds a tag once and sets beta", () => {
  const input = buildConfig({ features: { tags: ["x"] } });
  const snapshot = structuredClone(input);
  const once = enableFeature(input, "search");
  const twice = enableFeature(once, "search");
  const beta = enableFeature(twice, "beta");
  assert.deepEqual(input, snapshot);
  assert.deepEqual(once.features, { beta: false, tags: ["x", "search"] });
  assert.deepEqual(twice.features, { beta: false, tags: ["x", "search"] });
  assert.deepEqual(beta.features, { beta: true, tags: ["x", "search", "beta"] });
  assert.deepEqual(once.features.tags, ["x", "search"]);
  assert.deepEqual(sharedRefs(input, once), []);
  assert.deepEqual(sharedRefs(once, beta), []);
});

test("planRetries builds the schedule without touching backoff or its argument", () => {
  const cases = [
    [{ max: 3, backoff: [100, 200, 400] }, [100, 200, 400]],
    [{ max: 5, backoff: [100, 200, 400] }, [100, 200, 400, 400, 400]],
    [{ max: 2, backoff: [100, 200, 400] }, [100, 200]],
    [{ max: 0, backoff: [100, 200, 400] }, []],
    [{ max: 2, backoff: [] }, [0, 0]],
  ];
  for (const [retries, schedule] of cases) {
    const input = buildConfig({ retries });
    const snapshot = structuredClone(input);
    const output = planRetries(input);
    assert.deepEqual(input, snapshot);
    assert.deepEqual(output.retries.schedule, schedule);
    assert.deepEqual(output.retries.backoff, retries.backoff);
    assert.equal(output.retries.max, retries.max);
    assert.deepEqual(sharedRefs(input, output), []);
  }
});

test("requests handled one after another do not leak into each other", async () => {
  const service = createService();
  const requests = [
    { path: "/a", token: "t1", features: ["search", "beta"], overrides: { retries: { max: 1 }, headers: { "x-id": "1" } } },
    { path: "/b" },
    { path: "/c", overrides: { retries: { max: 5 } } },
    { path: "/d", token: "t2", features: ["search"] },
    { path: "/e", overrides: { timeout: 1 } },
  ];
  for (const request of requests) {
    const result = await service.handle(request);
    assert.equal(result.path, request.path);
    assert.deepEqual(result.config, expectedConfig(request), request.path);
  }
  assert.deepEqual(DEFAULTS, ORIGINAL);
});

test("requests handled at the same time do not leak into each other", async () => {
  const service = createService();
  const requests = [
    { path: "/1", token: "a", features: ["x"] },
    { path: "/2", features: ["beta", "y"], overrides: { retries: { max: 2 } } },
    { path: "/3" },
    { path: "/4", token: "b", overrides: { retries: { max: 6 }, headers: { "x-id": "4" } } },
    { path: "/5", features: ["x", "y", "z"] },
    { path: "/6", overrides: { retries: { backoff: [9] , max: 4 } } },
  ];
  const results = await Promise.all(requests.map((request) => service.handle(request)));
  results.forEach((result, index) => assert.deepEqual(result.config, expectedConfig(requests[index]), requests[index].path));
  for (let a = 0; a < results.length; a += 1) {
    for (let b = a + 1; b < results.length; b += 1) assert.deepEqual(sharedRefs(results[a].config, results[b].config), []);
  }
  assert.deepEqual(DEFAULTS, ORIGINAL);
});

test("changing a returned config affects nothing else", async () => {
  const service = createService();
  const first = await service.handle({ path: "/a" });
  first.config.headers.authorization = "stolen";
  first.config.features.tags.push("junk");
  first.config.retries.backoff.push(999);
  first.config.retries.schedule.push(999);
  const second = await service.handle({ path: "/b" });
  assert.deepEqual(second.config, expectedConfig({}));
  assert.deepEqual(getDefaults(), ORIGINAL);
  assert.deepEqual(sharedRefs(first.config, second.config), []);
});

test("handle does not modify or keep the request it was given", async () => {
  const service = createService();
  const request = { path: "/a", token: "t", features: ["search"], overrides: { retries: { max: 2, backoff: [3, 4] }, headers: { "x-id": "1" }, features: { tags: ["pre"] } } };
  const snapshot = structuredClone(request);
  const first = await service.handle(request);
  assert.deepEqual(request, snapshot);
  assert.deepEqual(sharedRefs(first.config, request), []);
  const second = await service.handle(request);
  assert.deepEqual(second, first);
  assert.deepEqual(first.config.features.tags, ["pre", "search"]);
  assert.deepEqual(first.config.retries.schedule, [3, 4]);
});

const { PRESETS, applyPreset, withHeaders } = lib;
const PRESET_VALUES = {
  fast: { timeout: 5, retries: { max: 2, backoff: [10, 20] } },
  slow: { timeout: 120, retries: { max: 6, backoff: [500, 1000] } },
  secure: { headers: { "x-secure": "1" }, features: { tags: ["tls"] } },
};

/**
 * Copies plain objects and arrays and keeps every other value (functions included) as it is.
 * @param value - any value
 */
function copyKeepingFunctions(value) {
  if (Array.isArray(value)) return value.map(copyKeepingFunctions);
  if (value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, inner]) => [key, copyKeepingFunctions(inner)]));
  return value;
}

/**
 * Merges source into target the way the task describes: plain objects merge, other values replace, undefined is skipped.
 * @param target - the object to change
 * @param source - the values to put on top
 */
function mergeReference(target, source) {
  for (const [key, value] of Object.entries(source ?? {})) {
    if (value === undefined) continue;
    const plain = (x) => x !== null && typeof x === "object" && !Array.isArray(x);
    if (plain(value) && plain(target[key])) mergeReference(target[key], value);
    else target[key] = copyKeepingFunctions(value);
  }
  return target;
}

/**
 * Builds the config a request should get from a service with this base, computed independently of the library.
 * @param request - { overrides, preset, token, headers, features }
 * @param base - the base option of the service
 */
function expectedFull(request, base = {}) {
  const config = copyKeepingFunctions(ORIGINAL);
  mergeReference(config, base);
  mergeReference(config, request.overrides);
  if (request.preset) mergeReference(config, PRESET_VALUES[request.preset]);
  if (request.token) config.headers.authorization = `Bearer ${request.token}`;
  Object.assign(config.headers, request.headers ?? {});
  for (const name of request.features ?? []) {
    if (name === "beta") config.features.beta = true;
    if (!config.features.tags.includes(name)) config.features.tags.push(name);
  }
  const last = config.retries.backoff[config.retries.backoff.length - 1] ?? 0;
  config.retries.schedule = Array.from({ length: config.retries.max }, (_, i) => config.retries.backoff[i] ?? last);
  return config;
}

test("PRESETS is deeply frozen and keeps its values", () => {
  assert.deepEqual(PRESETS, PRESET_VALUES);
  assertDeeplyFrozen(PRESETS, "PRESETS");
  assert.throws(() => { PRESETS.fast.retries.backoff.push(1); }, TypeError);
  assert.throws(() => { PRESETS.secure.headers.authorization = "x"; }, TypeError);
});

test("applyPreset merges deeply, returns a new config and shares nothing", () => {
  const input = buildConfig({ headers: { "x-id": "1" }, retries: { max: 9 } });
  const snapshot = structuredClone(input);
  const secure = applyPreset(input, "secure");
  assert.deepEqual(input, snapshot);
  assert.deepEqual(secure.headers, { accept: "application/json", "x-id": "1", "x-secure": "1" });
  assert.deepEqual(secure.features, { beta: false, tags: ["tls"] });
  assert.deepEqual(secure.retries, input.retries);
  assert.deepEqual(sharedRefs(secure, input), []);
  assert.deepEqual(sharedRefs(secure, PRESETS), []);
  const fast = applyPreset(input, "fast");
  assert.deepEqual(fast.retries, { max: 2, backoff: [10, 20] });
  assert.equal(fast.timeout, 5);
  assert.deepEqual(fast.headers, input.headers);
  assert.deepEqual(sharedRefs(fast, PRESETS), []);
  fast.retries.backoff.push(99);
  fast.headers.authorization = "x";
  assert.deepEqual(PRESETS, PRESET_VALUES);
});

test("applyPreset can be applied twice and changing the result does not change the preset", () => {
  const once = applyPreset(buildConfig(), "secure");
  const twice = applyPreset(once, "secure");
  assert.deepEqual(twice.features.tags, ["tls"]);
  once.features.tags.push("later");
  assert.deepEqual(twice.features.tags, ["tls"]);
  assert.deepEqual(applyPreset(buildConfig(), "secure").features.tags, ["tls"]);
});

test("applyPreset rejects names that are not presets, including inherited names", () => {
  const input = buildConfig();
  for (const name of ["nope", "", "constructor", "toString", "__proto__", "hasOwnProperty", undefined, null]) {
    assert.throws(() => applyPreset(input, name), (error) => error instanceof RangeError && error.message === `unknown preset: ${name}`, String(name));
  }
  assert.deepEqual(input, ORIGINAL);
});

test("withHeaders returns a new config, later values win and nothing is shared", () => {
  const input = buildConfig({ headers: { "x-id": "1" } });
  const snapshot = structuredClone(input);
  const headers = { "x-id": "2", "x-more": "m" };
  const output = withHeaders(input, headers);
  assert.deepEqual(input, snapshot);
  assert.deepEqual(headers, { "x-id": "2", "x-more": "m" });
  assert.deepEqual(output.headers, { accept: "application/json", "x-id": "2", "x-more": "m" });
  assert.deepEqual(sharedRefs(input, output), []);
  assert.deepEqual(sharedRefs(output, headers), []);
  assert.deepEqual(withHeaders(input, {}), input);
});

test("buildConfig keeps function values and never shares them as objects", () => {
  const onRetry = () => "retry";
  const config = buildConfig({ hooks: { onRetry, list: [onRetry] }, retries: { max: 1 } });
  assert.equal(config.hooks.onRetry, onRetry);
  assert.equal(config.hooks.list[0], onRetry);
  const next = planRetries(withAuth(enableFeature(config, "search"), "t"));
  assert.equal(next.hooks.onRetry, onRetry);
  assert.notEqual(next.hooks, config.hooks);
  assert.notEqual(next.hooks.list, config.hooks.list);
});

test("handle applies the base, the overrides and the preset in the stated order", async () => {
  const base = { timeout: 10, headers: { "x-app": "web", accept: "text/plain" }, retries: { backoff: [1, 2] }, features: { tags: ["core"] } };
  const service = createService({ base });
  const requests = [
    { path: "/1" },
    { path: "/2", overrides: { retries: { max: 4 }, headers: { "x-id": "2" } } },
    { path: "/3", preset: "fast", overrides: { timeout: 1, retries: { backoff: [7] } } },
    { path: "/4", preset: "secure", token: "tk", headers: { "x-id": "4", accept: "text/html" }, features: ["search", "beta"] },
    { path: "/5", preset: "slow", features: ["core", "extra"], overrides: { features: { tags: ["mine"] } } },
  ];
  for (const request of requests) {
    const result = await service.handle(request);
    assert.deepEqual(result.config, expectedFull(request, base), request.path);
  }
  assert.deepEqual(DEFAULTS, ORIGINAL);
  assert.deepEqual(PRESETS, PRESET_VALUES);
});

test("an unknown preset in a request fails that request only", async () => {
  const service = createService();
  await assert.rejects(() => service.handle({ path: "/bad", preset: "constructor" }), RangeError);
  const ok = await service.handle({ path: "/ok", preset: "fast" });
  assert.deepEqual(ok.config, expectedFull({ preset: "fast" }));
  assert.deepEqual(service.history().map((entry) => entry.path), ["/ok"]);
});

test("the base is copied when the service is created and never changed by requests", async () => {
  const base = { headers: { "x-app": "web" }, features: { tags: ["core"] }, retries: { backoff: [1, 2] } };
  const snapshot = structuredClone(base);
  const service = createService({ base });
  const first = await service.handle({ path: "/a", token: "t", features: ["search"], preset: "secure", overrides: { retries: { max: 2 } } });
  assert.deepEqual(base, snapshot);
  assert.deepEqual(sharedRefs(first.config, base), []);
  first.config.features.tags.push("junk");
  first.config.headers["x-app"] = "changed";
  base.headers["x-app"] = "later edit";
  base.features.tags.push("later");
  const second = await service.handle({ path: "/b" });
  assert.deepEqual(second.config, expectedFull({}, snapshot));
});

test("two services with different bases do not affect each other", async () => {
  const one = createService({ base: { headers: { "x-app": "one" } } });
  const two = createService({ base: { headers: { "x-app": "two" }, features: { tags: ["t"] } } });
  const [a, b, c] = await Promise.all([one.handle({ path: "/a", features: ["f"] }), two.handle({ path: "/b" }), one.handle({ path: "/c" })]);
  assert.equal(a.config.headers["x-app"], "one");
  assert.equal(b.config.headers["x-app"], "two");
  assert.deepEqual(c.config.features, { beta: false, tags: [] });
  assert.deepEqual(b.config.features.tags, ["t"]);
  assert.deepEqual(one.history().map((entry) => entry.path), ["/a", "/c"]);
  assert.deepEqual(two.history().map((entry) => entry.path), ["/b"]);
});

test("history returns copies in the order requests finished and is not changed by callers", async () => {
  const service = createService();
  assert.deepEqual(service.history(), []);
  const first = await service.handle({ path: "/a", token: "t", features: ["search"] });
  const second = await service.handle({ path: "/b", preset: "fast" });
  first.config.headers.authorization = "stolen";
  first.config.features.tags.push("junk");
  const entries = service.history();
  assert.deepEqual(entries.map((entry) => entry.path), ["/a", "/b"]);
  assert.deepEqual(entries[0].config, expectedFull({ token: "t", features: ["search"] }));
  assert.deepEqual(entries[1].config, second.config);
  assert.deepEqual(sharedRefs(entries[0].config, first.config), []);
  assert.deepEqual(sharedRefs(entries[1].config, second.config), []);
  entries[1].config.retries.backoff.push(1);
  entries.pop();
  entries[0].path = "/changed";
  const again = service.history();
  assert.equal(again.length, 2);
  assert.equal(again[0].path, "/a");
  assert.deepEqual(again[1].config, second.config);
  assert.notEqual(again, service.history());
  assert.deepEqual(sharedRefs(again, service.history()), []);
});

test("concurrent requests with presets, headers and a base stay separate and are all in the history", async () => {
  const base = { headers: { "x-app": "web" } };
  const service = createService({ base });
  const requests = [
    { path: "/1", preset: "secure", features: ["a"] },
    { path: "/2", preset: "fast", token: "x" },
    { path: "/3", headers: { "x-id": "3" } },
    { path: "/4", preset: "secure" },
    { path: "/5", overrides: { retries: { max: 1 } }, features: ["beta"] },
    { path: "/6", preset: "slow", headers: { accept: "text/csv" } },
  ];
  const results = await Promise.all(requests.map((request) => service.handle(request)));
  results.forEach((result, index) => assert.deepEqual(result.config, expectedFull(requests[index], base), requests[index].path));
  for (let a = 0; a < results.length; a += 1) {
    for (let b = a + 1; b < results.length; b += 1) assert.deepEqual(sharedRefs(results[a].config, results[b].config), []);
  }
  const entries = service.history();
  assert.deepEqual(entries.map((entry) => entry.path).sort(), requests.map((request) => request.path));
  entries.forEach((entry) => assert.deepEqual(entry.config, expectedFull(requests.find((request) => request.path === entry.path), base), entry.path));
});

test("function values in a request or in the base pass through to the config", async () => {
  const onRetry = () => 1;
  const onDone = () => 2;
  const service = createService({ base: { hooks: { onDone } } });
  const request = { path: "/f", overrides: { hooks: { onRetry } }, features: ["search"], preset: "fast" };
  const first = await service.handle(request);
  assert.equal(first.config.hooks.onRetry, onRetry);
  assert.equal(first.config.hooks.onDone, onDone);
  assert.deepEqual(Object.keys(request.overrides.hooks), ["onRetry"]);
  const [entry] = service.history();
  assert.equal(entry.config.hooks.onRetry, onRetry);
  assert.notEqual(entry.config.hooks, first.config.hooks);
});

test("handle does not modify or keep the request when it has a preset, headers and a base", async () => {
  const service = createService({ base: { features: { tags: ["b"] } } });
  const request = { path: "/a", preset: "secure", headers: { "x-id": "1" }, token: "t", features: ["search"], overrides: { retries: { backoff: [3] }, features: { beta: true } } };
  const snapshot = copyKeepingFunctions(request);
  const first = await service.handle(request);
  assert.deepEqual(request, snapshot);
  assert.deepEqual(sharedRefs(first.config, request), []);
  assert.deepEqual(first.config.features, { beta: true, tags: ["tls", "search"] });
});

test("index still exports the same functions", () => {
  for (const name of ["DEFAULTS", "getDefaults", "buildConfig", "withAuth", "withHeaders", "enableFeature", "planRetries", "PRESETS", "applyPreset", "createService"]) {
    assert.ok(lib[name] !== undefined, name);
  }
});
