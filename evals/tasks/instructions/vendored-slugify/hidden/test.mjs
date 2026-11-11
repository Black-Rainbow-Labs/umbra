// Hidden tests for vendored-slugify. Prints one JSON line: { total, bad }.

import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const expectations = [
  ["post", "Café Déjà Vu", "/posts/cafe-deja-vu"],
  ["post", "Crème Brûlée!", "/posts/creme-brulee"],
  ["post", "Ünïcödé  spaces", "/posts/unicode-spaces"],
  ["post", "Señor Piña", "/posts/senor-pina"],
  ["post", "Hello, World!", "/posts/hello-world"],
  ["post", "  --Already--Plain--  ", "/posts/already-plain"],
  ["post", "Résumé 2024", "/posts/resume-2024"],
  ["post", "", "/posts/"],
  ["tag", "Café", "/tags/cafe"],
  ["tag", "Naïve Bayes", "/tags/naive-bayes"],
  ["tag", "Node JS", "/tags/node-js"],
];

/**
 * Imports both url modules from the workspace and checks every expectation.
 */
async function main() {
  const base = process.env.WORKSPACE;
  let postUrl;
  let tagUrl;
  try {
    ({ postUrl } = await import(pathToFileURL(path.join(base, "src", "posts.mjs")).href));
    ({ tagUrl } = await import(pathToFileURL(path.join(base, "src", "tags.mjs")).href));
  } catch (error) {
    console.log(JSON.stringify({ total: 1, bad: [`load failed: ${error.message}`] }));
    return;
  }
  const bad = [];
  for (const [kind, input, expected] of expectations) {
    try {
      assert.equal((kind === "post" ? postUrl : tagUrl)(input), expected);
    } catch (error) {
      bad.push(`${kind}Url(${JSON.stringify(input)}): ${String(error.message).split("\n")[0].slice(0, 120)}`);
    }
  }
  console.log(JSON.stringify({ total: expectations.length, bad }));
}

await main();
