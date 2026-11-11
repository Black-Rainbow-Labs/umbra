import test from "node:test";
import assert from "node:assert/strict";
import { withTimeout, retry, mapLimit, TimeoutError, ValidationError } from "../src/index.mjs";

test("withTimeout passes a fast result through", (t, done) => {
  withTimeout((cb) => setTimeout(cb, 5, null, "fast"), 500, (error, result) => {
    assert.equal(error, null);
    assert.equal(result, "fast");
    done();
  });
});

test("withTimeout gives a TimeoutError and ignores a late result", (t, done) => {
  let calls = 0;
  withTimeout((cb) => setTimeout(cb, 80, null, "late"), 20, (error) => {
    calls += 1;
    assert.ok(error instanceof TimeoutError);
    assert.equal(error.code, "E_TIMEOUT");
    setTimeout(() => {
      assert.equal(calls, 1);
      done();
    }, 120);
  });
});

test("withTimeout passes errors from fn and catches a throw", (t, done) => {
  const failure = new Error("boom");
  withTimeout((cb) => cb(failure), 100, (error) => {
    assert.equal(error, failure);
    withTimeout(() => {
      throw failure;
    }, 100, (thrown) => {
      assert.equal(thrown, failure);
      done();
    });
  });
});

test("retry tries again until it works and counts attempts from 1", (t, done) => {
  const seen = [];
  retry((cb, attempt) => {
    seen.push(attempt);
    if (attempt < 3) cb(new Error(`fail ${attempt}`));
    else cb(null, "worked");
  }, { attempts: 5 }, (error, result) => {
    assert.equal(error, null);
    assert.equal(result, "worked");
    assert.deepEqual(seen, [1, 2, 3]);
    done();
  });
});

test("retry gives the last error when the attempts run out and skips ValidationError", (t, done) => {
  let calls = 0;
  retry((cb) => {
    calls += 1;
    cb(new Error(`fail ${calls}`));
  }, { attempts: 2 }, (error) => {
    assert.equal(error.message, "fail 2");
    let validationCalls = 0;
    retry((cb) => {
      validationCalls += 1;
      cb(new ValidationError("no"));
    }, { attempts: 4 }, (validationError) => {
      assert.ok(validationError instanceof ValidationError);
      assert.equal(validationCalls, 1);
      done();
    });
  });
});

test("mapLimit keeps the order and the limit", (t, done) => {
  let active = 0;
  let peak = 0;
  mapLimit([30, 10, 20, 5, 15], 2, (item, index, cb) => {
    active += 1;
    peak = Math.max(peak, active);
    setTimeout(() => {
      active -= 1;
      cb(null, item * 2);
    }, item);
  }, (error, results) => {
    assert.equal(error, null);
    assert.deepEqual(results, [60, 20, 40, 10, 30]);
    assert.equal(peak, 2);
    done();
  });
});

test("mapLimit stops at the first error and calls back once", (t, done) => {
  const started = [];
  let calls = 0;
  mapLimit([1, 2, 3, 4, 5], 1, (item, index, cb) => {
    started.push(item);
    cb(item === 2 ? new Error("two") : null, item);
  }, (error) => {
    calls += 1;
    assert.equal(error.message, "two");
    setTimeout(() => {
      assert.deepEqual(started, [1, 2]);
      assert.equal(calls, 1);
      done();
    }, 20);
  });
});

test("mapLimit handles an empty list and bad arguments", (t, done) => {
  mapLimit([], 3, () => {}, (error, results) => {
    assert.equal(error, null);
    assert.deepEqual(results, []);
    mapLimit([1], 0, () => {}, (limitError) => {
      assert.ok(limitError instanceof ValidationError);
      done();
    });
  });
});
