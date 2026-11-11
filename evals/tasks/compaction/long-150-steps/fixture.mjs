// Builds the 70 inspection files for long-150-steps. Each states one weight in digits, item 1
// states the batch limit. The code word appears only in the prompt. Shared by generate.mjs, the
// solution and the verifier.

import { seededRandom, randomInt, fillerText, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const ITEM_COUNT = 70;
export const CODE_WORD = "harrier";

/**
 * Returns the path of item n, for example items/item-07.txt.
 * @param n - the item number
 */
export function itemPath(n) {
  return `items/item-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Returns the CHECK line the prompt asks for, for one item.
 * @param weight - the measured weight
 * @param limit - the batch limit
 */
export function expectedCheckLine(weight, limit) {
  return `CHECK: ${weight} | ${weight > limit ? "OVER" : "OK"} #${CODE_WORD}`;
}

/**
 * Builds every item with its text and weight, plus the batch limit and the over count.
 * @param seed - the task seed
 */
export function buildItems(seed) {
  const random = seededRandom(seed);
  const limit = randomInt(random, 400, 600);
  const items = [];
  for (let n = 1; n <= ITEM_COUNT; n += 1) {
    let weight = randomInt(random, 200, 800);
    if (weight === limit) weight += 1;
    const paragraphs = fillerText(random, 2800);
    plantSentence(random, paragraphs, `The measured weight of this item is ${weight} units.`);
    if (n === 1) plantSentence(random, paragraphs, `The batch limit that applies to all items in this folder is ${limit} units.`);
    items.push({ path: itemPath(n), weight, text: [`Inspection item ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n") });
  }
  return { limit, items, overCount: items.filter((item) => item.weight > limit).length };
}
