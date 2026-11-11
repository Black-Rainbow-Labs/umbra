// Builds the 40 weekly reports for inventory-64k-reconcile. Week 1 states the opening stock,
// week 2 the reorder point and week 3 the supplier. Shared by generate.mjs, the solution and the
// verifier.

import { seededRandom, randomInt, fillerText, numberToWords, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const WEEK_COUNT = 40;
export const SUPPLIER = "Brightwater Components";

/**
 * Returns the path of week n, for example weeks/week-07.txt.
 * @param n - the week number
 */
export function weekPath(n) {
  return `weeks/week-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Builds the text of one weekly report with its planted facts.
 * @param random - seeded generator
 * @param n - the week number
 * @param week - { received, shipped }
 * @param facts - { opening, reorder }
 */
function weekText(random, n, week, facts) {
  const paragraphs = fillerText(random, 11800);
  plantSentence(random, paragraphs, `This week ${numberToWords(week.received)} units were received and ${numberToWords(week.shipped)} units were shipped.`);
  if (n === 1) plantSentence(random, paragraphs, `The opening stock of the product at the start of week one is ${numberToWords(facts.opening)} units.`);
  if (n === 2) plantSentence(random, paragraphs, `The reorder point for the product is ${numberToWords(facts.reorder)} units.`);
  if (n === 3) plantSentence(random, paragraphs, `The supplier of this product is ${SUPPLIER}.`);
  if (random() < 0.4) plantSentence(random, paragraphs, `A transfer of ${numberToWords(randomInt(random, 50, 400))} units to another site was cancelled and does not count.`);
  return [`Warehouse report, week ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n");
}

/**
 * Builds every week with its text and true closing stock, plus the facts and the final answers.
 * @param seed - the task seed
 */
export function buildWeeks(seed) {
  const random = seededRandom(seed);
  const opening = randomInt(random, 3000, 3400);
  const reorder = randomInt(random, 2800, 3200);
  let stock = opening;
  let below = 0;
  const weeks = [];
  for (let n = 1; n <= WEEK_COUNT; n += 1) {
    const received = randomInt(random, 100, 500);
    const shipped = randomInt(random, 100, 500);
    stock += received - shipped;
    if (stock < reorder) below += 1;
    weeks.push({ path: weekPath(n), text: weekText(random, n, { received, shipped }, { opening, reorder }), received, shipped, closing: stock });
  }
  return { opening, reorder, weeks, finalStock: stock, weeksBelow: below };
}
