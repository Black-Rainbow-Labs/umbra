// Hidden tests for semver.mjs. Prints one JSON line: { total, bad }.

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
 * Asserts that calling body throws an Error whose message starts with "Invalid version".
 * @param body - the function expected to throw
 */
function assertInvalid(body) {
  assert.throws(body, (error) => error instanceof Error && error.message.startsWith("Invalid version"));
}

/**
 * Loads the workspace module and runs all cases, then prints the summary line.
 */
async function main() {
  let semver;
  try {
    semver = await import(pathToFileURL(path.join(process.env.WORKSPACE, "semver.mjs")).href);
    for (const name of ["parse", "compare", "sortVersions"]) assert.equal(typeof semver[name], "function", `${name} is not exported`);
  } catch (error) {
    console.log(JSON.stringify({ total: 1, bad: [`load failed: ${error.message}`] }));
    return;
  }
  const { parse, compare, sortVersions } = semver;
  const bad = [];
  for (const { name, body } of cases) {
    try {
      body({ parse, compare, sortVersions });
    } catch (error) {
      bad.push(`${name}: ${String(error.message).split("\n")[0].slice(0, 160)}`);
    }
  }
  console.log(JSON.stringify({ total: cases.length, bad }));
}

test("parse plain", ({ parse }) => assert.deepStrictEqual(parse("1.2.3"), { major: 1, minor: 2, patch: 3, prerelease: [], build: [] }));
test("parse leading v", ({ parse }) => assert.deepStrictEqual(parse("v10.0.7"), { major: 10, minor: 0, patch: 7, prerelease: [], build: [] }));
test("parse prerelease and build", ({ parse }) => assert.deepStrictEqual(parse("1.2.3-alpha.1+build.5"), { major: 1, minor: 2, patch: 3, prerelease: ["alpha", 1], build: ["build", "5"] }));
test("parse build only", ({ parse }) => assert.deepStrictEqual(parse("1.2.3+007"), { major: 1, minor: 2, patch: 3, prerelease: [], build: ["007"] }));
test("parse prerelease identifier 0a and 0", ({ parse }) => assert.deepStrictEqual(parse("1.0.0-0a.0.x-y").prerelease, ["0a", 0, "x-y"]));
test("invalid inputs", ({ parse }) => {
  for (const bad of ["", " 1.2.3", "1.2.3 ", "1.2", "1.2.3.4", "01.2.3", "1.02.3", "1.2.03", "1.2.3-", "1.2.3-01", "1.2.3-a..b", "V1.2.3", "vv1.2.3", "1.2.3+", "a.b.c", "1.2.-3"]) {
    assertInvalid(() => parse(bad));
  }
});
test("non string input", ({ parse }) => {
  for (const bad of [null, undefined, 123, {}, ["1.2.3"]]) assertInvalid(() => parse(bad));
});
test("compare major minor patch numeric", ({ compare }) => {
  assert.equal(compare("1.10.0", "1.9.0"), 1);
  assert.equal(compare("1.9.0", "1.10.0"), -1);
  assert.equal(compare("2.0.0", "10.0.0"), -1);
  assert.equal(compare("1.0.10", "1.0.9"), 1);
  assert.equal(compare("1.2.3", "1.2.3"), 0);
  assert.equal(compare("v1.2.3", "1.2.3"), 0);
});
test("prerelease lower than release", ({ compare }) => {
  assert.equal(compare("1.0.0-alpha", "1.0.0"), -1);
  assert.equal(compare("1.0.0", "1.0.0-rc.1"), 1);
  assert.equal(compare("1.0.1-alpha", "1.0.0"), 1);
});
test("spec ordering chain", ({ compare }) => {
  const chain = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"];
  for (let i = 0; i < chain.length; i += 1) {
    for (let j = 0; j < chain.length; j += 1) assert.equal(compare(chain[i], chain[j]), Math.sign(i - j), `${chain[i]} vs ${chain[j]}`);
  }
});
test("text identifiers use string order", ({ compare }) => {
  assert.equal(compare("1.0.0-Beta", "1.0.0-alpha"), -1);
  assert.equal(compare("1.0.0-alpha", "1.0.0-alpine"), -1);
});
test("numeric lower than text, shorter lower", ({ compare }) => {
  assert.equal(compare("1.0.0-1", "1.0.0-a"), -1);
  assert.equal(compare("1.0.0-a.1", "1.0.0-a.1.0"), -1);
  assert.equal(compare("1.0.0-a.10", "1.0.0-a.9"), 1);
});
test("build metadata ignored", ({ compare }) => {
  assert.equal(compare("1.0.0+a", "1.0.0+b"), 0);
  assert.equal(compare("1.0.0-rc.1+x", "1.0.0-rc.1"), 0);
  assert.equal(compare("1.0.0+zzz", "1.0.1+aaa"), -1);
});
test("compare throws on invalid", ({ compare }) => {
  assertInvalid(() => compare("1.2", "1.2.3"));
  assertInvalid(() => compare("1.2.3", "x"));
});
test("sortVersions order and purity", ({ sortVersions }) => {
  const input = ["1.0.0", "2.0.0", "1.0.0-rc.1", "1.10.0", "1.9.0", "1.0.0-alpha", "v1.0.1"];
  const copy = [...input];
  const result = sortVersions(input);
  assert.deepStrictEqual(input, copy);
  assert.notStrictEqual(result, input);
  assert.deepStrictEqual(result, ["1.0.0-alpha", "1.0.0-rc.1", "1.0.0", "v1.0.1", "1.9.0", "1.10.0", "2.0.0"]);
});
test("sortVersions stable for equal", ({ sortVersions }) => {
  assert.deepStrictEqual(sortVersions(["1.0.0+b", "0.9.0", "1.0.0+a", "1.0.0"]), ["0.9.0", "1.0.0+b", "1.0.0+a", "1.0.0"]);
  assert.deepStrictEqual(sortVersions([]), []);
});

await main();
