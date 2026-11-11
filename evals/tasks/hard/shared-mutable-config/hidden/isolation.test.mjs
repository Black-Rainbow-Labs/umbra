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

test("index still exports the same functions", () => {
  for (const name of ["DEFAULTS", "getDefaults", "buildConfig", "withAuth", "enableFeature", "planRetries", "createService"]) {
    assert.ok(lib[name] !== undefined, name);
  }
});
