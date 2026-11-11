import test from "node:test";
import assert from "node:assert/strict";
import { parseFlags } from "../src/flags.mjs";

test("parses a value flag and a positional", () => {
  assert.deepStrictEqual(parseFlags(["--name", "demo", "file.txt"]), { flags: { name: "demo" }, positional: ["file.txt"] });
});

test("parses a short group", () => {
  assert.deepStrictEqual(parseFlags(["-ab"]), { flags: { a: true, b: true }, positional: [] });
});
