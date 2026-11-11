// A small file backed cache with expiry, in callback style. Each key is one JSON file in a folder.

import fs from "node:fs";
import path from "node:path";
import { readJson, writeJson } from "./files.mjs";
import { NotFoundError, ValidationError } from "./errors.mjs";

const KEY_PATTERN = /^[\w.-]+$/;

/**
 * Tells whether a cache key is acceptable: a string of letters, digits, underscore, dot or dash.
 * @param key - the value to test
 */
function isValidKey(key) {
  return typeof key === "string" && KEY_PATTERN.test(key);
}

/**
 * Stores a value under a key for ttlMs milliseconds. Calls back with (error). A bad key or a ttlMs
 * that is not a positive number gives a ValidationError through the callback, never a throw.
 * @param dir - the cache folder
 * @param key - the cache key
 * @param value - any JSON value
 * @param ttlMs - how long the value lives
 * @param callback - function (error)
 */
export function cachePut(dir, key, value, ttlMs, callback) {
  if (!isValidKey(key)) {
    process.nextTick(callback, new ValidationError(`bad cache key: ${String(key)}`));
    return;
  }
  if (typeof ttlMs !== "number" || !Number.isFinite(ttlMs) || ttlMs <= 0) {
    process.nextTick(callback, new ValidationError("ttlMs must be a positive number"));
    return;
  }
  writeJson(path.join(dir, `${key}.json`), { value, expiresAt: Date.now() + ttlMs }, callback);
}

/**
 * Reads a value. Calls back with (error, value). A missing or expired entry gives undefined (an
 * expired entry's file is removed first). A bad key gives a ValidationError through the callback.
 * @param dir - the cache folder
 * @param key - the cache key
 * @param callback - function (error, value)
 */
export function cacheGet(dir, key, callback) {
  if (!isValidKey(key)) {
    process.nextTick(callback, new ValidationError(`bad cache key: ${String(key)}`));
    return;
  }
  const file = path.join(dir, `${key}.json`);
  readJson(file, (error, entry) => {
    if (error instanceof NotFoundError) {
      callback(null, undefined);
      return;
    }
    if (error) {
      callback(error);
      return;
    }
    if (entry.expiresAt <= Date.now()) {
      fs.rm(file, { force: true }, () => callback(null, undefined));
      return;
    }
    callback(null, entry.value);
  });
}

/**
 * Removes every entry in the cache folder. Calls back with (error, count) where count is how many
 * entry files were removed. A folder that does not exist gives count 0.
 * @param dir - the cache folder
 * @param callback - function (error, count)
 */
export function cacheClear(dir, callback) {
  fs.readdir(dir, (error, names) => {
    if (error) {
      if (error.code === "ENOENT") callback(null, 0);
      else callback(error);
      return;
    }
    const entries = names.filter((name) => name.endsWith(".json"));
    if (entries.length === 0) {
      callback(null, 0);
      return;
    }
    let remaining = entries.length;
    let failed = false;
    for (const name of entries) {
      fs.rm(path.join(dir, name), { force: true }, (removeError) => {
        if (failed) return;
        if (removeError) {
          failed = true;
          callback(removeError);
          return;
        }
        remaining -= 1;
        if (remaining === 0) callback(null, entries.length);
      });
    }
  });
}
