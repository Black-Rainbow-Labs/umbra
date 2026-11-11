// Shared builder for the orders.csv fixture and the expected report. Used by generate.mjs, solve.mjs and verify.mjs.

const STATUSES = ["completed", "completed", "completed", "completed", "completed", "completed", "cancelled", "cancelled", "refunded"];
const HEADER = "order_id,customer_id,order_date,quantity,unit_price,status";

/**
 * Returns a deterministic random number generator.
 * @param seed - integer seed
 */
function mulberry32(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Picks a random integer from min to max inclusive.
 * @param random - the generator
 * @param min - lowest value
 * @param max - highest value
 */
function randomInt(random, min, max) {
  return min + Math.floor(random() * (max - min + 1));
}

/**
 * Builds one order row as an object.
 * @param random - the generator
 * @param index - zero based order index
 */
function buildOrder(random, index) {
  const month = randomInt(random, 1, 6);
  const day = randomInt(random, 1, 28);
  return {
    orderId: `O${String(1000 + index)}`,
    customerId: `C${String(randomInt(random, 1, 25)).padStart(3, "0")}`,
    date: `2024-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`,
    quantity: randomInt(random, 1, 6),
    priceCents: randomInt(random, 500, 12000),
    status: STATUSES[randomInt(random, 0, STATUSES.length - 1)],
  };
}

/**
 * Formats an order as a CSV line.
 * @param order - the order object
 */
function formatOrder(order) {
  return [order.orderId, order.customerId, order.date, order.quantity, (order.priceCents / 100).toFixed(2), order.status].join(",");
}

/**
 * Builds the CSV text: 400 orders plus 18 exact duplicate rows inserted at random places.
 * @param seed - integer seed
 */
export function buildOrdersCsv(seed) {
  const random = mulberry32(seed);
  const orders = Array.from({ length: 400 }, (_, index) => buildOrder(random, index));
  const lines = orders.map(formatOrder);
  for (let copy = 0; copy < 18; copy += 1) {
    const source = lines[randomInt(random, 0, 399)];
    lines.splice(randomInt(random, 1, lines.length), 0, source);
  }
  return `${HEADER}\n${lines.join("\n")}\n`;
}

/**
 * Parses the CSV text into rows, counting each order_id only once (first row wins).
 * @param csvText - the orders.csv text
 */
function parseUniqueOrders(csvText) {
  const seen = new Set();
  const orders = [];
  for (const line of csvText.trim().split("\n").slice(1)) {
    const [orderId, customerId, date, quantity, price, status] = line.split(",");
    if (seen.has(orderId)) continue;
    seen.add(orderId);
    orders.push({ orderId, customerId, month: date.slice(0, 7), cents: Number(quantity) * Math.round(Number(price) * 100), status });
  }
  return orders;
}

/**
 * Computes the expected report object from the CSV text.
 * @param csvText - the orders.csv text
 */
export function computeExpectedReport(csvText) {
  const orders = parseUniqueOrders(csvText);
  const completed = orders.filter((order) => order.status === "completed");
  const byCustomer = new Map();
  const byMonth = new Map();
  let totalCents = 0;
  for (const order of completed) {
    totalCents += order.cents;
    byCustomer.set(order.customerId, (byCustomer.get(order.customerId) || 0) + order.cents);
    byMonth.set(order.month, (byMonth.get(order.month) || 0) + order.cents);
  }
  const [topCustomer, topCents] = [...byCustomer.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0];
  const revenueByMonth = {};
  for (const month of [...byMonth.keys()].sort()) revenueByMonth[month] = byMonth.get(month) / 100;
  return {
    totalRevenue: totalCents / 100,
    orderCount: completed.length,
    cancelledCount: orders.filter((order) => order.status === "cancelled").length,
    averageOrderValue: Math.round(totalCents / completed.length) / 100,
    topCustomer,
    topCustomerRevenue: topCents / 100,
    revenueByMonth,
  };
}
