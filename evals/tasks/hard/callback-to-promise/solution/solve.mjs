// Reference solution: adds the promise returning versions next to the callback forms.

import fs from "node:fs";
import path from "node:path";

const PROMISIFY = `// Helper that turns a callback style call into a promise.

/**
 * Runs start(callback) and returns a promise that settles with the callback's outcome. A throw
 * from start becomes a rejection.
 * @param start - function (callback) that begins the operation
 */
export function callbackToPromise(start) {
  return new Promise((resolve, reject) => {
    start((error, value) => (error ? reject(error) : resolve(value)));
  });
}
`;

const FILES_ADD = `
/**
 * Promise version of readJson.
 * @param file - the file path
 */
export function readJsonAsync(file) {
  return callbackToPromise((callback) => readJson(file, callback));
}

/**
 * Promise version of writeJson. Resolves with undefined.
 * @param file - the file path
 * @param data - the value to write
 */
export function writeJsonAsync(file, data) {
  return callbackToPromise((callback) => writeJson(file, data, callback));
}
`;

const CACHE_ADD = `
/**
 * Promise version of cachePut.
 * @param dir - the cache folder
 * @param key - the cache key
 * @param value - any JSON value
 * @param ttlMs - how long the value lives
 */
export function cachePutAsync(dir, key, value, ttlMs) {
  return callbackToPromise((callback) => cachePut(dir, key, value, ttlMs, callback));
}

/**
 * Promise version of cacheGet. Resolves with undefined for a missing or expired entry.
 * @param dir - the cache folder
 * @param key - the cache key
 */
export function cacheGetAsync(dir, key) {
  return callbackToPromise((callback) => cacheGet(dir, key, callback));
}

/**
 * Promise version of cacheClear. Resolves with the number of removed entries.
 * @param dir - the cache folder
 */
export function cacheClearAsync(dir) {
  return callbackToPromise((callback) => cacheClear(dir, callback));
}
`;

const TASKS_ADD = `
/**
 * Calls a promise returning function and passes its outcome to a callback style done function.
 * @param start - function () returning a value or a promise
 * @param done - function (error, result)
 */
function settleInto(start, done) {
  Promise.resolve()
    .then(start)
    .then((value) => done(null, value), (error) => done(error || new Error("rejected without a reason")));
}

/**
 * Promise version of withTimeout. fn is () => Promise.
 * @param fn - function () returning a promise
 * @param ms - the time limit in milliseconds
 */
export function withTimeoutAsync(fn, ms) {
  if (typeof fn !== "function") return callbackToPromise((callback) => withTimeout(fn, ms, callback));
  return callbackToPromise((callback) => withTimeout((done) => settleInto(() => fn(), done), ms, callback));
}

/**
 * Promise version of retry. fn is (attempt) => Promise with attempt counting from 1.
 * @param fn - function (attempt) returning a promise
 * @param options - { attempts, delayMs }
 */
export function retryAsync(fn, options) {
  if (typeof fn !== "function") return callbackToPromise((callback) => retry(fn, options, callback));
  return callbackToPromise((callback) => retry((done, attempt) => settleInto(() => fn(attempt), done), options, callback));
}

/**
 * Promise version of mapLimit. iteratee is (item, index) => Promise.
 * @param items - an array
 * @param limit - the concurrency
 * @param iteratee - function (item, index) returning a promise
 */
export function mapLimitAsync(items, limit, iteratee) {
  if (typeof iteratee !== "function") return callbackToPromise((callback) => mapLimit(items, limit, iteratee, callback));
  return callbackToPromise((callback) => mapLimit(items, limit, (item, index, done) => settleInto(() => iteratee(item, index), done), callback));
}
`;

/**
 * Appends a block to a source file and adds an import line after its last import.
 * @param file - the file path
 * @param importLine - the line to add
 * @param block - the code to append
 */
function extend(file, importLine, block) {
  let text = fs.readFileSync(file, "utf8");
  const lastImport = [...text.matchAll(/^import .*$/gm)].pop();
  text = text.slice(0, lastImport.index + lastImport[0].length) + `\n${importLine}` + text.slice(lastImport.index + lastImport[0].length);
  fs.writeFileSync(file, text + block);
}

/**
 * Adds the promise versions to the three modules and the index.
 * @param workspaceDir - the workspace
 */
export async function solve(workspaceDir) {
  const src = path.join(workspaceDir, "src");
  fs.writeFileSync(path.join(src, "promisify.mjs"), PROMISIFY);
  const line = 'import { callbackToPromise } from "./promisify.mjs";';
  extend(path.join(src, "files.mjs"), line, FILES_ADD);
  extend(path.join(src, "cache.mjs"), line, CACHE_ADD);
  extend(path.join(src, "tasks.mjs"), line, TASKS_ADD);
  fs.writeFileSync(path.join(src, "index.mjs"), `// Public entry of the library.

export { AppError, NotFoundError, ParseError, ValidationError, TimeoutError } from "./errors.mjs";
export { readJson, writeJson, readJsonAsync, writeJsonAsync } from "./files.mjs";
export { cachePut, cacheGet, cacheClear, cachePutAsync, cacheGetAsync, cacheClearAsync } from "./cache.mjs";
export { withTimeout, retry, mapLimit, withTimeoutAsync, retryAsync, mapLimitAsync } from "./tasks.mjs";
`);
}
