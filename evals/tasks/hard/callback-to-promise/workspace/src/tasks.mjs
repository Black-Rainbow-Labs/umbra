// Control flow helpers in callback style: timeouts, retries and limited concurrency.

import { TimeoutError, ValidationError } from "./errors.mjs";

/**
 * Runs fn(done) and calls back with its result, or with a TimeoutError when it takes longer than ms.
 * Only the first outcome counts; a later result is ignored. The timer is cleared when fn finishes.
 * @param fn - function (done) that calls done(error, result)
 * @param ms - the time limit in milliseconds, a positive number
 * @param callback - function (error, result)
 */
export function withTimeout(fn, ms, callback) {
  if (typeof fn !== "function" || typeof ms !== "number" || !(ms > 0)) {
    process.nextTick(callback, new ValidationError("withTimeout needs a function and a positive ms"));
    return;
  }
  let finished = false;
  const finish = (error, result) => {
    if (finished) return;
    finished = true;
    clearTimeout(timer);
    if (error) callback(error);
    else callback(null, result);
  };
  const timer = setTimeout(() => finish(new TimeoutError(`timed out after ${ms} ms`)), ms);
  try {
    fn(finish);
  } catch (error) {
    finish(error);
  }
}

/**
 * Calls fn(done, attempt) until it succeeds or `attempts` tries were used. Waits delayMs between
 * tries. A ValidationError is never retried. When the tries run out the last error is passed on.
 * @param fn - function (done, attempt) with attempt counting from 1
 * @param options - { attempts (default 3, an integer of at least 1), delayMs (default 0) }
 * @param callback - function (error, result)
 */
export function retry(fn, options, callback) {
  const { attempts = 3, delayMs = 0 } = options ?? {};
  if (typeof fn !== "function" || !Number.isInteger(attempts) || attempts < 1 || typeof delayMs !== "number" || !(delayMs >= 0)) {
    process.nextTick(callback, new ValidationError("retry needs a function, attempts of at least 1 and delayMs of 0 or more"));
    return;
  }
  let attempt = 0;
  const run = () => {
    attempt += 1;
    fn((error, result) => {
      if (!error) {
        callback(null, result);
      } else if (error instanceof ValidationError || attempt >= attempts) {
        callback(error);
      } else {
        setTimeout(run, delayMs);
      }
    }, attempt);
  };
  run();
}

/**
 * Maps items through iteratee(item, index, done) with at most `limit` running at once. Calls back
 * with (error, results) where results are in the order of the items. After the first error no more
 * iterations are started and the callback is not called again. An empty list gives [] (asynchronously).
 * @param items - an array
 * @param limit - the concurrency, an integer of at least 1
 * @param iteratee - function (item, index, done)
 * @param callback - function (error, results)
 */
export function mapLimit(items, limit, iteratee, callback) {
  if (!Array.isArray(items) || !Number.isInteger(limit) || limit < 1 || typeof iteratee !== "function") {
    process.nextTick(callback, new ValidationError("mapLimit needs an array, a limit of at least 1 and a function"));
    return;
  }
  if (items.length === 0) {
    process.nextTick(callback, null, []);
    return;
  }
  const results = new Array(items.length);
  let next = 0;
  let active = 0;
  let completed = 0;
  let failed = false;
  const launch = () => {
    while (!failed && active < limit && next < items.length) {
      const index = next;
      next += 1;
      active += 1;
      iteratee(items[index], index, (error, value) => {
        active -= 1;
        if (failed) return;
        if (error) {
          failed = true;
          callback(error);
          return;
        }
        results[index] = value;
        completed += 1;
        if (completed === items.length) callback(null, results);
        else launch();
      });
    }
  };
  launch();
}
