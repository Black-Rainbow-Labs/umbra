import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { cachePut, cacheGet, cacheClear, ValidationError } from "../src/index.mjs";

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "asynckit-cache-"));

test("cachePut and cacheGet store a value", (t, done) => {
  cachePut(dir, "user.1", { name: "Ada" }, 10000, (error) => {
    assert.equal(error, null);
    cacheGet(dir, "user.1", (getError, value) => {
      assert.equal(getError, null);
      assert.deepEqual(value, { name: "Ada" });
      done();
    });
  });
});

test("cacheGet gives undefined for a missing key", (t, done) => {
  cacheGet(dir, "nothing", (error, value) => {
    assert.equal(error, null);
    assert.equal(value, undefined);
    done();
  });
});

test("an expired entry gives undefined and its file is removed", (t, done) => {
  fs.writeFileSync(path.join(dir, "old.json"), JSON.stringify({ value: 1, expiresAt: Date.now() - 5 }));
  cacheGet(dir, "old", (error, value) => {
    assert.equal(error, null);
    assert.equal(value, undefined);
    assert.equal(fs.existsSync(path.join(dir, "old.json")), false);
    done();
  });
});

test("bad keys and bad ttl values come back through the callback", (t, done) => {
  cacheGet(dir, "a/b", (error) => {
    assert.ok(error instanceof ValidationError);
    cachePut(dir, "ok", 1, 0, (putError) => {
      assert.ok(putError instanceof ValidationError);
      cachePut(dir, "../x", 1, 100, (keyError) => {
        assert.ok(keyError instanceof ValidationError);
        done();
      });
    });
  });
});

test("cacheClear removes entry files and counts them", (t, done) => {
  const clearDir = path.join(dir, "clear");
  cachePut(clearDir, "a", 1, 1000, () => {
    cachePut(clearDir, "b", 2, 1000, () => {
      cacheClear(clearDir, (error, count) => {
        assert.equal(error, null);
        assert.equal(count, 2);
        cacheClear(path.join(dir, "no-such-folder"), (missingError, zero) => {
          assert.equal(missingError, null);
          assert.equal(zero, 0);
          done();
        });
      });
    });
  });
});
