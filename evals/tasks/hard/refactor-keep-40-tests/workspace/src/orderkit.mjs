// orderkit: money arithmetic, order validation, pricing and reporting for the checkout service.
// Everything lives in this one file. Amounts are integers in minor units ("cents") everywhere.

// ---------------------------------------------------------------------------------------------
// Section 1: money
// ---------------------------------------------------------------------------------------------

const CURRENCIES = {
  USD: { symbol: "$", decimals: 2 },
  EUR: { symbol: "€", decimals: 2 },
  GBP: { symbol: "£", decimals: 2 },
  JPY: { symbol: "¥", decimals: 0 },
};

const ROUNDING_MODES = ["half-up", "half-even", "down"];

/** Shared settings read by the money, validation, pricing and report code. */
const settings = { currency: "USD", roundingMode: "half-up" };

/**
 * Changes the shared settings. Unknown currencies or rounding modes are rejected.
 * @param options - { currency, roundingMode }, either may be left out
 */
export function configure(options = {}) {
  if (options.currency !== undefined && !CURRENCIES[options.currency]) {
    throw new RangeError(`unsupported currency: ${options.currency}`);
  }
  if (options.roundingMode !== undefined && !ROUNDING_MODES.includes(options.roundingMode)) {
    throw new RangeError(`unsupported rounding mode: ${options.roundingMode}`);
  }
  if (options.currency !== undefined) settings.currency = options.currency;
  if (options.roundingMode !== undefined) settings.roundingMode = options.roundingMode;
  return { ...settings };
}

/**
 * Returns a copy of the shared settings.
 */
export function getSettings() {
  return { ...settings };
}

/**
 * Puts the shared settings back to USD and half-up rounding.
 */
export function resetSettings() {
  settings.currency = "USD";
  settings.roundingMode = "half-up";
}

/**
 * Returns the metadata of the currently configured currency.
 */
export function currencyInfo() {
  return { code: settings.currency, ...CURRENCIES[settings.currency] };
}

/**
 * Rounds a number to an integer using the configured rounding mode.
 * @param value - the number to round
 * @param mode - overrides the configured rounding mode
 */
export function roundValue(value, mode = settings.roundingMode) {
  const sign = value < 0 ? -1 : 1;
  const magnitude = Math.abs(Number(value.toPrecision(12)));
  const floor = Math.floor(magnitude);
  const fraction = magnitude - floor;
  if (mode === "down") return sign * floor;
  if (Math.abs(fraction - 0.5) < 1e-9) {
    if (mode === "half-even") return sign * (floor % 2 === 0 ? floor : floor + 1);
    return sign * (floor + 1);
  }
  return sign * Math.round(magnitude);
}

/**
 * Converts a decimal amount (number or numeric string) to minor units.
 * @param amount - for example 12.34 or "12.34"
 */
export function toCents(amount) {
  const number = typeof amount === "string" ? Number(amount.trim()) : amount;
  if (typeof number !== "number" || !Number.isFinite(number)) {
    throw new TypeError(`not a money amount: ${String(amount)}`);
  }
  const factor = 10 ** currencyInfo().decimals;
  return roundValue(number * factor);
}

/**
 * Converts minor units to a decimal number.
 * @param cents - an integer amount in minor units
 */
export function fromCents(cents) {
  const factor = 10 ** currencyInfo().decimals;
  return cents / factor;
}

/**
 * Adds two amounts in minor units.
 * @param a - first amount
 * @param b - second amount
 */
export function addMoney(a, b) {
  return a + b;
}

/**
 * Subtracts b from a, both in minor units.
 * @param a - the amount to subtract from
 * @param b - the amount to subtract
 */
export function subtractMoney(a, b) {
  return a - b;
}

/**
 * Multiplies an amount by a factor and rounds with the configured rounding mode.
 * @param cents - amount in minor units
 * @param factor - any finite number, for example a tax rate
 */
export function multiplyMoney(cents, factor) {
  return roundValue(cents * factor);
}

/**
 * Adds up a list of amounts. An empty list gives 0.
 * @param list - amounts in minor units
 */
export function sumMoney(list) {
  let total = 0;
  for (const amount of list) total = addMoney(total, amount);
  return total;
}

/**
 * Compares two amounts and returns -1, 0 or 1.
 * @param a - first amount
 * @param b - second amount
 */
export function compareMoney(a, b) {
  if (a === b) return 0;
  return a < b ? -1 : 1;
}

/**
 * Splits a total across weights with the largest remainder method so the parts add up exactly.
 * Ties on the remainder go to the earlier weight. Zero total weight gives all zeros.
 * @param total - amount in minor units
 * @param weights - non negative numbers
 */
export function allocateCents(total, weights) {
  const weightSum = weights.reduce((sum, weight) => sum + weight, 0);
  if (weights.length === 0) return [];
  if (weightSum === 0) return weights.map(() => 0);
  const exact = weights.map((weight) => (total * weight) / weightSum);
  const parts = exact.map((value) => Math.floor(value));
  let remaining = total - parts.reduce((sum, part) => sum + part, 0);
  const order = exact
    .map((value, index) => ({ index, remainder: value - Math.floor(value) }))
    .sort((left, right) => right.remainder - left.remainder || left.index - right.index);
  for (const { index } of order) {
    if (remaining <= 0) break;
    parts[index] += 1;
    remaining -= 1;
  }
  return parts;
}

/**
 * Formats an amount for display, for example "$1,234.50" or "-$0.99".
 * @param cents - amount in minor units
 * @param options - { symbol: false } leaves the currency symbol out
 */
export function formatMoney(cents, options = {}) {
  const info = currencyInfo();
  const negative = cents < 0;
  const absolute = Math.abs(cents);
  const factor = 10 ** info.decimals;
  const whole = Math.floor(absolute / factor);
  const fraction = absolute % factor;
  const grouped = String(whole).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const decimals = info.decimals > 0 ? `.${String(fraction).padStart(info.decimals, "0")}` : "";
  const symbol = options.symbol === false ? "" : info.symbol;
  return `${negative ? "-" : ""}${symbol}${grouped}${decimals}`;
}

/**
 * Parses text such as "$1,234.50" or "12.5" into minor units. Throws on text that is not money.
 * @param text - the text to parse
 */
export function parseMoney(text) {
  const info = currencyInfo();
  const cleaned = String(text).trim().replace(info.symbol, "").replace(/,/g, "");
  if (!/^-?\d+(\.\d+)?$/.test(cleaned)) throw new TypeError(`not a money amount: ${text}`);
  return toCents(cleaned);
}

/**
 * Returns the larger of two amounts.
 * @param a - first amount
 * @param b - second amount
 */
export function maxMoney(a, b) {
  return compareMoney(a, b) >= 0 ? a : b;
}

/**
 * Returns the smaller of two amounts.
 * @param a - first amount
 * @param b - second amount
 */
export function minMoney(a, b) {
  return compareMoney(a, b) <= 0 ? a : b;
}

// ---------------------------------------------------------------------------------------------
// Section 2: validation
// ---------------------------------------------------------------------------------------------

/** Error thrown by assertValidOrder. The list of messages is in `errors`. */
export class ValidationError extends Error {
  /**
   * Creates the error from a list of problems.
   * @param errors - the problem messages
   */
  constructor(errors) {
    super(`invalid order: ${errors.join("; ")}`);
    this.name = "ValidationError";
    this.errors = errors;
  }
}

const SKU_PATTERN = /^[A-Z]{3}-\d{4}$/;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const REGION_CODES = ["US-CA", "US-NY", "US-TX", "DE", "GB", "JP"];

/**
 * Checks that a value is a plain object (not null, not an array).
 * @param value - the value to test
 */
function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Checks a SKU such as "ABC-1234".
 * @param sku - the value to test
 */
export function validateSku(sku) {
  return typeof sku === "string" && SKU_PATTERN.test(sku);
}

/**
 * Checks an email address in a simple way: one at sign and a dot in the domain.
 * @param email - the value to test
 */
export function validateEmail(email) {
  return typeof email === "string" && EMAIL_PATTERN.test(email);
}

/**
 * Validates one order line and returns a list of problem messages.
 * @param line - { sku, qty, unitPrice } where unitPrice is a decimal amount
 * @param index - the line position, used in messages
 */
export function validateLine(line, index = 0) {
  const errors = [];
  const label = `line ${index + 1}`;
  if (!isPlainObject(line)) return [`${label}: must be an object`];
  if (!validateSku(line.sku)) errors.push(`${label}: bad sku`);
  if (!Number.isInteger(line.qty) || line.qty < 1) errors.push(`${label}: qty must be a positive integer`);
  try {
    const cents = toCents(line.unitPrice);
    if (cents < 0) errors.push(`${label}: unitPrice must not be negative`);
  } catch {
    errors.push(`${label}: unitPrice is not a number`);
  }
  return errors;
}

/**
 * Validates a shipping address and returns a list of problem messages.
 * @param address - { name, street, city, region, postal }
 */
export function validateAddress(address) {
  if (!isPlainObject(address)) return ["address: must be an object"];
  const errors = [];
  for (const field of ["name", "street", "city", "postal"]) {
    if (typeof address[field] !== "string" || address[field].trim() === "") errors.push(`address: ${field} is required`);
  }
  if (!REGION_CODES.includes(address.region)) errors.push("address: unknown region");
  return errors;
}

/**
 * Validates a whole order and returns { valid, errors }. The order currency, when present, must
 * match the configured currency.
 * @param order - { email, currency, lines, address }
 */
export function validateOrder(order) {
  if (!isPlainObject(order)) return { valid: false, errors: ["order: must be an object"] };
  const errors = [];
  if (!validateEmail(order.email)) errors.push("order: bad email");
  if (order.currency !== undefined && order.currency !== getSettings().currency) {
    errors.push(`order: currency ${order.currency} does not match ${getSettings().currency}`);
  }
  if (!Array.isArray(order.lines) || order.lines.length === 0) {
    errors.push("order: needs at least one line");
  } else {
    order.lines.forEach((line, index) => errors.push(...validateLine(line, index)));
  }
  errors.push(...validateAddress(order.address));
  return { valid: errors.length === 0, errors };
}

/**
 * Throws a ValidationError when the order is invalid and returns the order when it is valid.
 * @param order - the order to check
 */
export function assertValidOrder(order) {
  const { valid, errors } = validateOrder(order);
  if (!valid) throw new ValidationError(errors);
  return order;
}

/**
 * Returns a cleaned copy of an order: strings trimmed, sku upper case, qty defaulting to 1,
 * the email lower case, and the currency filled in from the settings. The input is not changed.
 * @param order - the raw order
 */
export function normalizeOrder(order) {
  const address = isPlainObject(order.address) ? { ...order.address } : {};
  for (const key of Object.keys(address)) {
    if (typeof address[key] === "string") address[key] = address[key].trim();
  }
  const lines = (Array.isArray(order.lines) ? order.lines : []).map((line) => ({
    ...line,
    sku: typeof line.sku === "string" ? line.sku.trim().toUpperCase() : line.sku,
    qty: line.qty === undefined ? 1 : line.qty,
  }));
  return {
    ...order,
    email: typeof order.email === "string" ? order.email.trim().toLowerCase() : order.email,
    currency: order.currency === undefined ? getSettings().currency : order.currency,
    lines,
    address,
  };
}

/**
 * Lists the region codes the shop ships to.
 */
export function supportedRegions() {
  return [...REGION_CODES];
}

/**
 * Validates a list of orders and returns { valid: [...], invalid: [{ order, errors }] }.
 * @param orders - the orders to check
 */
export function partitionOrders(orders) {
  const valid = [];
  const invalid = [];
  for (const order of orders) {
    const result = validateOrder(order);
    if (result.valid) valid.push(order);
    else invalid.push({ order, errors: result.errors });
  }
  return { valid, invalid };
}

// ---------------------------------------------------------------------------------------------
// Section 3: pricing
// ---------------------------------------------------------------------------------------------

/** Tax rates by region code. A region that is not listed has rate 0. */
export const TAX_RATES = {
  "US-CA": 0.0725,
  "US-NY": 0.04,
  "US-TX": 0.0625,
  DE: 0.19,
  GB: 0.2,
  JP: 0.1,
};

const FREE_SHIPPING_THRESHOLD = 10000;
const FLAT_SHIPPING = 599;
const REMOTE_SURCHARGE = 400;

/** Cache of looked up tax rates, filled by taxRateFor. */
const taxRateCache = new Map();

/** Registered coupons by code, with how often each was redeemed. */
const coupons = new Map();

/**
 * Looks up the tax rate of a region and remembers the answer.
 * @param region - a region code such as "DE"
 */
export function taxRateFor(region) {
  if (taxRateCache.has(region)) return taxRateCache.get(region);
  const rate = TAX_RATES[region] ?? 0;
  taxRateCache.set(region, rate);
  return rate;
}

/**
 * Tells how many tax rates are currently remembered.
 */
export function taxCacheSize() {
  return taxRateCache.size;
}

/**
 * Forgets every remembered tax rate. Needed after TAX_RATES is edited.
 */
export function clearTaxCache() {
  taxRateCache.clear();
}

/**
 * Registers a coupon. type is "percent" (value 0 to 100) or "fixed" (value in decimal money).
 * @param code - the coupon code, stored upper case
 * @param definition - { type, value, maxUses }
 */
export function registerCoupon(code, definition) {
  if (!["percent", "fixed"].includes(definition.type)) throw new RangeError(`unknown coupon type: ${definition.type}`);
  if (definition.type === "percent" && (definition.value < 0 || definition.value > 100)) {
    throw new RangeError("percent coupons must be between 0 and 100");
  }
  coupons.set(code.toUpperCase(), { ...definition, uses: 0 });
}

/**
 * Returns a copy of a registered coupon, or undefined.
 * @param code - the coupon code, any case
 */
export function getCoupon(code) {
  const coupon = coupons.get(String(code).toUpperCase());
  return coupon ? { ...coupon } : undefined;
}

/**
 * Removes every registered coupon.
 */
export function clearCoupons() {
  coupons.clear();
}

/**
 * Counts one use of a coupon and returns the coupon. Throws when it is unknown or used up.
 * @param code - the coupon code, any case
 */
export function redeemCoupon(code) {
  const key = String(code).toUpperCase();
  const coupon = coupons.get(key);
  if (!coupon) throw new Error(`unknown coupon: ${code}`);
  if (coupon.maxUses !== undefined && coupon.uses >= coupon.maxUses) throw new Error(`coupon used up: ${key}`);
  coupon.uses += 1;
  return { ...coupon };
}

/**
 * Computes the price of one line in minor units: unit price times quantity.
 * @param line - { unitPrice, qty }
 */
export function lineSubtotal(line) {
  return multiplyMoney(toCents(line.unitPrice), line.qty);
}

/**
 * Computes the discount a coupon gives on a subtotal. It never exceeds the subtotal.
 * @param subtotal - amount in minor units
 * @param coupon - { type, value }
 */
export function discountAmount(subtotal, coupon) {
  if (!coupon) return 0;
  const raw = coupon.type === "percent" ? multiplyMoney(subtotal, coupon.value / 100) : toCents(coupon.value);
  return minMoney(raw, subtotal);
}

/**
 * Computes shipping in minor units. Orders of 100.00 or more ship free (before tax and
 * discounts, in the configured currency), others pay the flat fee plus a surcharge for JP.
 * @param subtotal - the order subtotal in minor units
 * @param region - the destination region code
 */
export function shippingCost(subtotal, region) {
  const threshold = toCents(FREE_SHIPPING_THRESHOLD / 100);
  if (subtotal >= threshold) return 0;
  const surcharge = region === "JP" ? REMOTE_SURCHARGE : 0;
  return toCents(FLAT_SHIPPING / 100) + toCents(surcharge / 100);
}

/**
 * Spreads an order level discount over the lines in proportion to their subtotals.
 * @param lineTotals - line subtotals in minor units
 * @param discount - the order discount in minor units
 */
export function allocateDiscount(lineTotals, discount) {
  return allocateCents(discount, lineTotals);
}

/**
 * Prices an order. Tax is computed per line on the discounted line amount and rounded per line.
 * Returns { lines, subtotal, discount, shipping, tax, total } with every amount in minor units.
 * @param order - a valid order, optionally with a couponCode
 */
export function priceOrder(order) {
  const totals = order.lines.map(lineSubtotal);
  const subtotal = sumMoney(totals);
  const coupon = order.couponCode ? getCoupon(order.couponCode) : undefined;
  const discount = discountAmount(subtotal, coupon);
  const shares = allocateDiscount(totals, discount);
  const rate = taxRateFor(order.address.region);
  const lines = order.lines.map((line, index) => {
    const net = subtractMoney(totals[index], shares[index]);
    return { sku: line.sku, qty: line.qty, subtotal: totals[index], discount: shares[index], net, tax: multiplyMoney(net, rate) };
  });
  const tax = sumMoney(lines.map((line) => line.tax));
  const shipping = shippingCost(subtotal, order.address.region);
  const total = sumMoney([subtotal, -discount, shipping, tax]);
  return { lines, subtotal, discount, shipping, tax, total };
}

/**
 * Prices an order and counts the coupon use when a coupon is applied.
 * @param order - a valid order, optionally with a couponCode
 */
export function checkoutOrder(order) {
  assertValidOrder(order);
  if (order.couponCode) redeemCoupon(order.couponCode);
  return priceOrder(order);
}

/**
 * Puts every piece of shared state back to its starting value: settings, the tax cache and coupons.
 */
export function resetAll() {
  resetSettings();
  clearTaxCache();
  clearCoupons();
}

// ---------------------------------------------------------------------------------------------
// Section 4: reports
// ---------------------------------------------------------------------------------------------

/**
 * Pads text on the right to a width.
 * @param text - the text
 * @param width - the final width
 */
function padRight(text, width) {
  return String(text).padEnd(width, " ");
}

/**
 * Pads text on the left to a width.
 * @param text - the text
 * @param width - the final width
 */
function padLeft(text, width) {
  return String(text).padStart(width, " ");
}

/**
 * Renders rows as a fixed width text table. Columns with align "right" are right aligned.
 * @param rows - array of objects
 * @param columns - [{ key, title, align }]
 */
export function renderTable(rows, columns) {
  const widths = columns.map((column) => Math.max(column.title.length, ...rows.map((row) => String(row[column.key]).length)));
  const format = (values) =>
    values.map((value, index) => (columns[index].align === "right" ? padLeft(value, widths[index]) : padRight(value, widths[index]))).join("  ").trimEnd();
  const lines = [format(columns.map((column) => column.title)), widths.map((width) => "-".repeat(width)).join("  ")];
  for (const row of rows) lines.push(format(columns.map((column) => row[column.key])));
  return lines.join("\n");
}

/**
 * Formats the priced order as a plain text receipt.
 * @param priced - the result of priceOrder
 * @param options - { title } the first line of the receipt
 */
export function formatReceipt(priced, options = {}) {
  const rows = priced.lines.map((line) => ({
    sku: line.sku,
    qty: line.qty,
    amount: formatMoney(line.net),
  }));
  const table = renderTable(rows, [
    { key: "sku", title: "SKU" },
    { key: "qty", title: "Qty", align: "right" },
    { key: "amount", title: "Amount", align: "right" },
  ]);
  const summary = [
    `Subtotal: ${formatMoney(priced.subtotal)}`,
    `Discount: ${formatMoney(-priced.discount)}`,
    `Shipping: ${formatMoney(priced.shipping)}`,
    `Tax: ${formatMoney(priced.tax)}`,
    `Total: ${formatMoney(priced.total)}`,
  ];
  return [options.title ?? "Receipt", "", table, "", ...summary].join("\n");
}

/**
 * Turns the priced order into CSV text with a header and one row per line.
 * @param priced - the result of priceOrder
 */
export function toCsv(priced) {
  const rows = ["sku,qty,subtotal,discount,net,tax"];
  for (const line of priced.lines) {
    const amounts = [line.subtotal, line.discount, line.net, line.tax].map((cents) => formatMoney(cents, { symbol: false }).replace(/,/g, ""));
    rows.push([line.sku, line.qty, ...amounts].join(","));
  }
  return rows.join("\n");
}

/**
 * Prices a list of valid orders and returns totals: { orders, units, revenue, tax, average }.
 * average is revenue divided by orders in minor units, rounded with the configured mode; 0 for no orders.
 * @param orders - valid orders
 */
export function summarizeOrders(orders) {
  const priced = orders.map(priceOrder);
  const revenue = sumMoney(priced.map((entry) => entry.total));
  const units = orders.reduce((sum, order) => sum + order.lines.reduce((inner, line) => inner + line.qty, 0), 0);
  return {
    orders: orders.length,
    units,
    revenue,
    tax: sumMoney(priced.map((entry) => entry.tax)),
    average: orders.length === 0 ? 0 : roundValue(revenue / orders.length),
  };
}

/**
 * Groups orders by destination region and totals each group: { [region]: { orders, revenue } }.
 * @param orders - valid orders
 */
export function groupByRegion(orders) {
  const groups = {};
  for (const order of orders) {
    const region = order.address.region;
    const group = groups[region] ?? { orders: 0, revenue: 0 };
    group.orders += 1;
    group.revenue = addMoney(group.revenue, priceOrder(order).total);
    groups[region] = group;
  }
  return groups;
}

/**
 * Returns the best selling SKUs as [{ sku, units }], most units first, ties by sku ascending.
 * @param orders - valid orders
 * @param count - how many entries to return
 */
export function topSkus(orders, count = 3) {
  const units = new Map();
  for (const order of orders) {
    for (const line of order.lines) units.set(line.sku, (units.get(line.sku) ?? 0) + line.qty);
  }
  return [...units.entries()]
    .map(([sku, total]) => ({ sku, units: total }))
    .sort((left, right) => right.units - left.units || left.sku.localeCompare(right.sku))
    .slice(0, count);
}

/**
 * Renders the summary and the regional breakdown as text for the daily report.
 * @param orders - valid orders
 */
export function dailyReport(orders) {
  const summary = summarizeOrders(orders);
  const regions = groupByRegion(orders);
  const regionRows = Object.keys(regions)
    .sort()
    .map((region) => ({ region, orders: regions[region].orders, revenue: formatMoney(regions[region].revenue) }));
  const lines = [
    `Orders: ${summary.orders}`,
    `Units: ${summary.units}`,
    `Revenue: ${formatMoney(summary.revenue)}`,
    `Average: ${formatMoney(summary.average)}`,
    "",
  ];
  if (regionRows.length > 0) {
    lines.push(
      renderTable(regionRows, [
        { key: "region", title: "Region" },
        { key: "orders", title: "Orders", align: "right" },
        { key: "revenue", title: "Revenue", align: "right" },
      ]),
    );
  }
  return lines.join("\n");
}
