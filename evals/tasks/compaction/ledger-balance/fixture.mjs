// Builds the 35 daily ledger files for ledger-balance. Day 1 states the opening balance, each day
// describes its transactions in words. Shared by generate.mjs, the solution and the verifier.

import { seededRandom, randomInt, fillerText, numberToWords, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const DAY_COUNT = 35;
export const WITHDRAWAL_FEE = 2;

/**
 * Returns the path of day n, for example days/day-07.txt.
 * @param n - the day number
 */
export function dayPath(n) {
  return `days/day-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Picks the transactions for one day: one to three deposits or withdrawals, and sometimes a
 * pending request that does not change the balance.
 * @param random - seeded generator
 */
function pickTransactions(random) {
  const count = randomInt(random, 1, 3);
  return Array.from({ length: count }, () => ({ kind: random() < 0.55 ? "deposit" : "withdrawal", amount: randomInt(random, 5, 180) }));
}

/**
 * Builds the text of one day.
 * @param random - seeded generator
 * @param n - the day number
 * @param transactions - the day's transactions
 * @param opening - the opening balance (only stated on day 1)
 */
function dayText(random, n, transactions, opening) {
  const paragraphs = fillerText(random, 7400);
  if (n === 1) plantSentence(random, paragraphs, `The opening balance of the account at the start of day one is ${numberToWords(opening)} dollars.`);
  for (const t of transactions) plantSentence(random, paragraphs, `A ${t.kind} of ${numberToWords(t.amount)} dollars was recorded on this day.`);
  if (random() < 0.4) plantSentence(random, paragraphs, `A pending request of ${numberToWords(randomInt(random, 5, 180))} dollars was not processed and does not count.`);
  return [`Ledger day ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n");
}

/**
 * Returns the change in balance for one day, including the fee for every withdrawal.
 * @param transactions - the day's transactions
 */
export function dayDelta(transactions) {
  return transactions.reduce((sum, t) => sum + (t.kind === "deposit" ? t.amount : -t.amount - WITHDRAWAL_FEE), 0);
}

/**
 * Builds every day with its text, transactions and true closing balance.
 * @param seed - the task seed
 */
export function buildDays(seed) {
  const random = seededRandom(seed);
  const opening = randomInt(random, 600, 900);
  let balance = opening;
  const days = [];
  for (let n = 1; n <= DAY_COUNT; n += 1) {
    const transactions = pickTransactions(random);
    balance += dayDelta(transactions);
    days.push({ path: dayPath(n), text: dayText(random, n, transactions, opening), transactions, closing: balance });
  }
  return { opening, days };
}
