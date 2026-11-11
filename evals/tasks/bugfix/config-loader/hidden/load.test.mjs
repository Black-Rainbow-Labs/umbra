import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const lib = await import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, "src", "index.mjs")).href);
const { loadConfig, coerceValue, deepMerge, DEFAULTS } = lib;

const ORIGINAL_DEFAULTS = {
  debug: false,
  name: "service",
  server: { host: "localhost", port: 8080, tls: false },
  cache: { enabled: true, ttlSeconds: 300 },
};

test("coerceValue handles false, true, numbers and strings", () => {
  assert.equal(coerceValue("false"), false);
  assert.equal(coerceValue("true"), true);
  assert.equal(coerceValue("0"), 0);
  assert.equal(coerceValue("-4"), -4);
  assert.equal(coerceValue("2.5"), 2.5);
  assert.equal(coerceValue("hello world"), "hello world");
  assert.equal(coerceValue(""), "");
});

test("false overrides a true default", () => {
  const config = loadConfig("[cache]\nenabled = false\nttlSeconds = 0\n");
  assert.equal(config.cache.enabled, false);
  assert.equal(config.cache.ttlSeconds, 0);
});

test("top level false and true values", () => {
  assert.equal(loadConfig("debug = true").debug, true);
  assert.equal(loadConfig("debug = false").debug, false);
});

test("loading does not change the defaults", () => {
  loadConfig("name = other\ndebug = true\n[server]\nport = 1\ntls = true\n[cache]\nenabled = false\n");
  assert.deepEqual(DEFAULTS, ORIGINAL_DEFAULTS);
});

test("a second load starts from the defaults again", () => {
  const first = loadConfig("name = first\n[server]\nport = 9000\n[cache]\nenabled = false\n");
  assert.equal(first.server.port, 9000);
  const second = loadConfig("[server]\nhost = example.org\n");
  assert.deepEqual(second, { ...ORIGINAL_DEFAULTS, server: { host: "example.org", port: 8080, tls: false } });
});

test("changing a returned config does not leak into the next load", () => {
  const first = loadConfig("");
  first.server.port = 1234;
  first.cache.enabled = false;
  assert.equal(loadConfig("").server.port, 8080);
  assert.equal(loadConfig("").cache.enabled, true);
});

test("deepMerge leaves both inputs unchanged", () => {
  const base = { a: { b: 1, c: 2 }, d: 3 };
  const override = { a: { b: 9 }, e: { f: 1 } };
  const merged = deepMerge(base, override);
  assert.deepEqual(merged, { a: { b: 9, c: 2 }, d: 3, e: { f: 1 } });
  assert.deepEqual(base, { a: { b: 1, c: 2 }, d: 3 });
  assert.deepEqual(override, { a: { b: 9 }, e: { f: 1 } });
});

test("a scalar override replaces an object and sections parse", () => {
  const config = loadConfig("# comment\nname = api\n\n[server]\nport = 3000\n");
  assert.equal(config.name, "api");
  assert.equal(config.server.port, 3000);
  assert.equal(config.server.host, "localhost");
});
