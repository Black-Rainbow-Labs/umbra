// Builds the 24 order files for orders-16k-audit. Every order states a quantity and a unit price
// in words, order 1 states the tax rate. Shared by generate.mjs, the solution and the verifier.

import { seededRandom, randomInt, fillerText, numberToWords, plantSentence } from "../../../src/utils/fixtureText.mjs";

export const ORDER_COUNT = 24;
const RATES = [5, 10, 15, 20, 25];

/**
 * Returns the path of order n, for example orders/order-07.txt.
 * @param n - the order number
 */
export function orderPath(n) {
  return `orders/order-${String(n).padStart(2, "0")}.txt`;
}

/**
 * Returns the total for one order: units times price plus tax. Prices are multiples of 20 so
 * the tax is always a whole number of dollars.
 * @param quantity - units ordered
 * @param price - dollars per unit
 * @param rate - tax rate in percent
 */
export function orderTotal(quantity, price, rate) {
  const subtotal = quantity * price;
  return subtotal + (subtotal * rate) / 100;
}

/**
 * Builds the text of one order.
 * @param random - seeded generator
 * @param n - the order number
 * @param quantity - units ordered
 * @param price - dollars per unit
 * @param rate - tax rate in percent
 */
function orderText(random, n, quantity, price, rate) {
  const paragraphs = fillerText(random, 5900);
  plantSentence(random, paragraphs, `The customer ordered ${numberToWords(quantity)} units at a price of ${numberToWords(price)} dollars for one unit.`);
  if (n === 1) plantSentence(random, paragraphs, `The sales tax rate that applies to every order in this folder is ${numberToWords(rate)} percent.`);
  if (random() < 0.4) plantSentence(random, paragraphs, `A change of ${numberToWords(randomInt(random, 2, 30))} units was requested but was not approved and does not count.`);
  return [`Order ${n}`, "", ...paragraphs.flatMap((p) => [p, ""])].join("\n");
}

/**
 * Builds every order with its text and true total, plus the tax rate, the grand total and the
 * file numbers that share the highest total.
 * @param seed - the task seed
 */
export function buildOrders(seed) {
  const random = seededRandom(seed);
  const rate = RATES[randomInt(random, 0, RATES.length - 1)];
  const orders = [];
  for (let n = 1; n <= ORDER_COUNT; n += 1) {
    const quantity = randomInt(random, 3, 60);
    const price = randomInt(random, 2, 25) * 20;
    orders.push({ path: orderPath(n), text: orderText(random, n, quantity, price, rate), total: orderTotal(quantity, price, rate) });
  }
  const grandTotal = orders.reduce((sum, o) => sum + o.total, 0);
  const highest = Math.max(...orders.map((o) => o.total));
  const largest = orders.map((o, i) => (o.total === highest ? i + 1 : 0)).filter(Boolean);
  return { rate, orders, grandTotal, largest };
}
