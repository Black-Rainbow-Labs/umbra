// Hidden tests for ini.mjs. Prints one JSON line: { total, bad }.

import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const cases = [];

/**
 * Registers a named test case.
 * @param name - test name
 * @param body - function that throws on failure
 */
function test(name, body) {
  cases.push({ name, body });
}

/**
 * Asserts that parse throws an Error with exactly the given message.
 * @param parse - the parse function
 * @param text - the input text
 * @param message - the expected error message
 */
function assertThrowsMessage(parse, text, message) {
  assert.throws(() => parse(text), (error) => error instanceof Error && error.message === message);
}

/**
 * Loads the workspace module and runs all cases, then prints the summary line.
 */
async function main() {
  let parse;
  try {
    ({ parse } = await import(pathToFileURL(path.join(process.env.WORKSPACE, "ini.mjs")).href));
    assert.equal(typeof parse, "function", "parse is not exported");
  } catch (error) {
    console.log(JSON.stringify({ total: 1, bad: [`load failed: ${error.message}`] }));
    return;
  }
  const bad = [];
  for (const { name, body } of cases) {
    try {
      body(parse);
    } catch (error) {
      bad.push(`${name}: ${String(error.message).split("\n")[0].slice(0, 160)}`);
    }
  }
  console.log(JSON.stringify({ total: cases.length, bad }));
}

test("example", (parse) => assert.deepStrictEqual(parse("name = demo\n; note\n[server]\nhost = localhost\nport = 8080\n"), { name: "demo", server: { host: "localhost", port: "8080" } }));
test("empty text", (parse) => assert.deepStrictEqual(parse(""), {}));
test("comments and blanks", (parse) => assert.deepStrictEqual(parse("# one\n\n  ; two\n   \n[a]\n# three\nk=v"), { a: { k: "v" } }));
test("crlf", (parse) => assert.deepStrictEqual(parse("[a]\r\nk = v\r\nj = w\r\n"), { a: { k: "v", j: "w" } }));
test("first equals splits", (parse) => assert.deepStrictEqual(parse("url = http://x/?a=1&b=2"), { url: "http://x/?a=1&b=2" }));
test("no inline comments", (parse) => assert.deepStrictEqual(parse("url = http://x/?a=1 ; not a comment\nc = a # b"), { url: "http://x/?a=1 ; not a comment", c: "a # b" }));
test("empty value", (parse) => assert.deepStrictEqual(parse("a =\nb = \n[s]\nc="), { a: "", b: "", s: { c: "" } }));
test("quoted value keeps spaces", (parse) => assert.deepStrictEqual(parse('name = "  padded  "\nq = ""\nr = "a = b ; c"'), { name: "  padded  ", q: "", r: "a = b ; c" }));
test("single quote char is not unquoted", (parse) => assert.deepStrictEqual(parse('a = "\nb = x"\nc = "x'), { a: '"', b: 'x"', c: '"x' }));
test("values stay strings", (parse) => assert.deepStrictEqual(parse("n = 42\nb = true\nz = null"), { n: "42", b: "true", z: "null" }));
test("section name trimmed, empty section kept", (parse) => assert.deepStrictEqual(parse("[ spaced name ]\n[empty]\n"), { "spaced name": {}, empty: {} }));
test("duplicate key last wins", (parse) => assert.deepStrictEqual(parse("a = 1\na = 2\n[s]\nb = 1\nb = 3"), { a: "2", s: { b: "3" } }));
test("duplicate section merges", (parse) => assert.deepStrictEqual(parse("[s]\na = 1\n[t]\nx = 1\n[s]\nb = 2"), { s: { a: "1", b: "2" }, t: { x: "1" } }));
test("top level key after sections stays in last section", (parse) => assert.deepStrictEqual(parse("top = 1\n[s]\nin = 2"), { top: "1", s: { in: "2" } }));
test("key and value trimmed", (parse) => assert.deepStrictEqual(parse("   spaced key   =    spaced value   \n\tt\t=\tv\t"), { "spaced key": "spaced value", t: "v" }));
test("plain object results", (parse) => {
  const result = parse("[s]\na=1");
  assert.equal(Object.getPrototypeOf(result), Object.prototype);
  assert.equal(Object.getPrototypeOf(result.s), Object.prototype);
});
test("error without equals", (parse) => assertThrowsMessage(parse, "a = 1\n\nbroken line\nb = 2", "Invalid line 3: broken line"));
test("error line number counts blanks and comments", (parse) => assertThrowsMessage(parse, "; c\n\n# c\n   oops   \n", "Invalid line 4: oops"));
test("error unclosed header", (parse) => assertThrowsMessage(parse, "[a]\n[b\n", "Invalid line 2: [b"));
test("error empty key", (parse) => assertThrowsMessage(parse, "x = 1\n = value", "Invalid line 2: = value"));
test("error empty section name", (parse) => {
  assertThrowsMessage(parse, "[]", "Invalid line 1: []");
  assertThrowsMessage(parse, "\n[   ]", "Invalid line 2: [   ]");
});
test("error with crlf line numbers", (parse) => assertThrowsMessage(parse, "a=1\r\nb=2\r\nbad\r\n", "Invalid line 3: bad"));

await main();
