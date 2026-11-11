import test from "node:test";
import assert from "node:assert/strict";
import { postUrl } from "../src/posts.mjs";
import { tagUrl } from "../src/tags.mjs";

test("postUrl slugs a plain title", () => {
  assert.equal(postUrl("Hello, World!"), "/posts/hello-world");
});

test("tagUrl slugs a plain name", () => {
  assert.equal(tagUrl("Node JS"), "/tags/node-js");
});
