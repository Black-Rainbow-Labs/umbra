// Builds the glossary and the 36 documents for glossary-codes. The glossary is the first file,
// and every later document refers to one or two codes from it. Shared by generate.mjs, the
// solution and the verifier.

import { seededRandom, randomInt, fillerText, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const DOC_COUNT = 36;
export const GLOSSARY = { QX7: "the billing gateway", MRL2: "the nightly ledger export", TPZ9: "the courier handoff desk", KV4: "the cold storage annex", BRN6: "the returns triage queue", DLX3: "the supplier scorecard", HWE8: "the dock scheduling board", JQA5: "the pallet audit sweep" };
const TEMPLATES = ["Traffic for the pilot was routed through [[CODE]] for the whole week.", "Everyone agreed that [[CODE]] needs a second owner before the next quarter.", "The open question about [[CODE]] was moved to the follow up list.", "Two incidents last month were traced back to [[CODE]].", "Budget for [[CODE]] stays flat until the review is finished."];

/**
 * Returns the path of document n, for example docs/07.md (docs/00-glossary.md for 0).
 * @param n - the document number
 */
export function docPath(n) {
  return n === 0 ? "docs/00-glossary.md" : `docs/${String(n).padStart(2, "0")}.md`;
}

/**
 * Builds the glossary document text.
 * @param random - seeded generator
 */
function glossaryText(random) {
  const entries = Object.entries(GLOSSARY).map(([code, term]) => `- ${code}: ${term}`);
  return ["# Glossary of internal codes", "", "The other documents in this folder refer to these codes as [[CODE]].", "", ...entries, "", ...fillerText(random, 1500).flatMap((p) => [p, ""])].join("\n");
}

/**
 * Builds every document with its original text and the expected text after the codes are
 * replaced by *term*.
 * @param seed - the task seed
 */
export function buildDocs(seed) {
  const random = seededRandom(seed);
  const codes = Object.keys(GLOSSARY);
  const docs = [{ path: docPath(0), text: glossaryText(random), expected: null }];
  for (let n = 1; n <= DOC_COUNT; n += 1) {
    const paragraphs = fillerText(random, 7400);
    const count = randomInt(random, 1, 2);
    for (let i = 0; i < count; i += 1) {
      const code = codes[randomInt(random, 0, codes.length - 1)];
      plantSentence(random, paragraphs, TEMPLATES[randomInt(random, 0, TEMPLATES.length - 1)].replace("CODE", code));
    }
    const text = [`# Document ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n");
    const expected = text.replace(/\[\[([A-Z0-9]+)\]\]/g, (_, code) => `*${GLOSSARY[code]}*`);
    docs.push({ path: docPath(n), text, expected });
  }
  return docs;
}
