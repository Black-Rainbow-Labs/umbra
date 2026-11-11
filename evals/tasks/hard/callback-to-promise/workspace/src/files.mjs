// JSON file helpers in callback style.

import fs from "node:fs";
import path from "node:path";
import { NotFoundError, ParseError, ValidationError } from "./errors.mjs";

/**
 * Reads a file and parses it as JSON. Calls back with (error, data). A missing file gives a
 * NotFoundError and text that is not JSON gives a ParseError; other errors are passed on as they are.
 * @param file - the file path
 * @param callback - function (error, data)
 */
export function readJson(file, callback) {
  fs.readFile(file, "utf8", (error, text) => {
    if (error) {
      callback(error.code === "ENOENT" ? new NotFoundError(`not found: ${file}`, { cause: error }) : error);
      return;
    }
    let data;
    try {
      data = JSON.parse(text);
    } catch (parseError) {
      callback(new ParseError(`invalid JSON in ${file}`, { cause: parseError }));
      return;
    }
    callback(null, data);
  });
}

/**
 * Writes data as JSON (2 space indent, trailing newline) and creates missing parent folders.
 * Calls back with (error). Undefined data gives a ValidationError and data that cannot be turned
 * into JSON (for example a cycle) gives a ParseError.
 * @param file - the file path
 * @param data - the value to write
 * @param callback - function (error)
 */
export function writeJson(file, data, callback) {
  if (data === undefined) {
    process.nextTick(callback, new ValidationError("nothing to write: data is undefined"));
    return;
  }
  let text;
  try {
    text = `${JSON.stringify(data, null, 2)}\n`;
  } catch (stringifyError) {
    process.nextTick(callback, new ParseError(`cannot serialize data for ${file}`, { cause: stringifyError }));
    return;
  }
  fs.mkdir(path.dirname(file), { recursive: true }, (mkdirError) => {
    if (mkdirError) {
      callback(mkdirError);
      return;
    }
    fs.writeFile(file, text, "utf8", (writeError) => callback(writeError ?? null));
  });
}
