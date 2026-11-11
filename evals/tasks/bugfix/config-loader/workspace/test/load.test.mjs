import test from "node:test";
import assert from "node:assert/strict";
import { loadConfig } from "../src/index.mjs";

test("empty text gives the defaults", () => {
  const config = loadConfig("");
  assert.equal(config.server.port, 8080);
  assert.equal(config.cache.enabled, true);
});

test("values are converted to numbers and booleans", () => {
  const config = loadConfig("name = api\n[server]\nport = 9000\ntls = true\n");
  assert.equal(config.name, "api");
  assert.equal(config.server.port, 9000);
  assert.equal(config.server.tls, true);
  assert.equal(config.server.host, "localhost");
});

test("false in the file turns a setting off", () => {
  const config = loadConfig("[cache]\nenabled = false\n");
  assert.equal(config.cache.enabled, false);
});

test("loading twice does not carry settings from the first load", () => {
  loadConfig("[server]\nport = 9000\n");
  assert.equal(loadConfig("").server.port, 8080);
});
