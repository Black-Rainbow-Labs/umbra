import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { readJson, writeJson, NotFoundError, ParseError, ValidationError } from "../src/index.mjs";

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "asynckit-files-"));

test("writeJson then readJson round trips and creates folders", (t, done) => {
  const file = path.join(dir, "a", "b", "data.json");
  writeJson(file, { x: [1, 2] }, (error) => {
    assert.equal(error, null);
    assert.equal(fs.readFileSync(file, "utf8"), '{\n  "x": [\n    1,\n    2\n  ]\n}\n');
    readJson(file, (readError, data) => {
      assert.equal(readError, null);
      assert.deepEqual(data, { x: [1, 2] });
      done();
    });
  });
});

test("readJson reports a missing file as NotFoundError", (t, done) => {
  readJson(path.join(dir, "missing.json"), (error, data) => {
    assert.ok(error instanceof NotFoundError);
    assert.equal(error.code, "E_NOT_FOUND");
    assert.equal(error.cause.code, "ENOENT");
    assert.equal(data, undefined);
    done();
  });
});

test("readJson reports bad JSON as ParseError", (t, done) => {
  const file = path.join(dir, "bad.json");
  fs.writeFileSync(file, "{nope");
  readJson(file, (error) => {
    assert.ok(error instanceof ParseError);
    assert.ok(error.cause instanceof SyntaxError);
    done();
  });
});

test("writeJson rejects undefined and cycles through the callback", (t, done) => {
  writeJson(path.join(dir, "u.json"), undefined, (error) => {
    assert.ok(error instanceof ValidationError);
    const cycle = {};
    cycle.self = cycle;
    writeJson(path.join(dir, "c.json"), cycle, (cycleError) => {
      assert.ok(cycleError instanceof ParseError);
      assert.equal(fs.existsSync(path.join(dir, "c.json")), false);
      done();
    });
  });
});

test("callbacks of the validation paths run asynchronously", (t, done) => {
  let sync = true;
  writeJson(path.join(dir, "z.json"), undefined, () => {
    assert.equal(sync, false);
    done();
  });
  sync = false;
});
