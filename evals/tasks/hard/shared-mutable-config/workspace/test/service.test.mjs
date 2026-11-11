import test from "node:test";
import assert from "node:assert/strict";
import { createService } from "../src/index.mjs";

test("a plain request gets the defaults and a retry schedule", async () => {
  const { config } = await createService().handle({ path: "/a" });
  assert.equal(config.timeout, 30);
  assert.deepEqual(config.headers, { accept: "application/json" });
  assert.deepEqual(config.retries.schedule, [100, 200, 400]);
});

test("a token only applies to its own request", async () => {
  const service = createService();
  const first = await service.handle({ path: "/a", token: "t1" });
  const second = await service.handle({ path: "/b" });
  assert.equal(first.config.headers.authorization, "Bearer t1");
  assert.equal(second.config.headers.authorization, undefined);
});

test("a feature only applies to its own request", async () => {
  const service = createService();
  await service.handle({ path: "/a", features: ["search"] });
  const second = await service.handle({ path: "/b" });
  assert.deepEqual(second.config.features, { beta: false, tags: [] });
});
