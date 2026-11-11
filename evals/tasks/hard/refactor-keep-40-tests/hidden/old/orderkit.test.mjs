import test, { beforeEach } from "node:test";
import assert from "node:assert/strict";
import * as kit from "../src/orderkit.mjs";

const {
  configure, getSettings, resetSettings, currencyInfo, roundValue, toCents, fromCents, addMoney, subtractMoney, multiplyMoney,
  sumMoney, compareMoney, allocateCents, formatMoney, parseMoney, maxMoney, minMoney, ValidationError, validateSku, validateEmail,
  validateLine, validateAddress, validateOrder, assertValidOrder, normalizeOrder, supportedRegions, partitionOrders, TAX_RATES,
  taxRateFor, taxCacheSize, clearTaxCache, registerCoupon, getCoupon, redeemCoupon, lineSubtotal, discountAmount, shippingCost,
  priceOrder, checkoutOrder, resetAll, renderTable, formatReceipt, toCsv, summarizeOrders, groupByRegion, topSkus, dailyReport,
} = kit;

const address = { name: "Ada", street: "1 Main St", city: "Springfield", region: "US-CA", postal: "90001" };

/** Builds a valid order for tests. */
function makeOrder(extra = {}) {
  return { email: "ada@example.com", lines: [{ sku: "ABC-1234", qty: 2, unitPrice: 10 }], address: { ...address }, ...extra };
}

beforeEach(() => resetAll());

test("toCents and fromCents convert decimal amounts", () => {
  assert.equal(toCents(12.34), 1234);
  assert.equal(toCents("0.07"), 7);
  assert.equal(fromCents(1999), 19.99);
});

test("toCents rejects values that are not money", () => {
  assert.throws(() => toCents("abc"), TypeError);
  assert.throws(() => toCents(NaN), TypeError);
});

test("roundValue supports half-up, half-even and down", () => {
  assert.equal(roundValue(2.5), 3);
  assert.equal(roundValue(-2.5), -3);
  assert.equal(roundValue(2.5, "half-even"), 2);
  assert.equal(roundValue(3.5, "half-even"), 4);
  assert.equal(roundValue(2.99, "down"), 2);
  assert.equal(roundValue(-2.99, "down"), -2);
});

test("configure changes the rounding mode used by multiplyMoney", () => {
  assert.equal(multiplyMoney(25, 0.1), 3);
  configure({ roundingMode: "half-even" });
  assert.equal(multiplyMoney(25, 0.1), 2);
  assert.equal(getSettings().roundingMode, "half-even");
});

test("configure rejects unknown currencies and modes", () => {
  assert.throws(() => configure({ currency: "XYZ" }), RangeError);
  assert.throws(() => configure({ roundingMode: "up" }), RangeError);
  assert.equal(getSettings().currency, "USD");
});

test("getSettings returns a copy", () => {
  const copy = getSettings();
  copy.currency = "EUR";
  assert.equal(getSettings().currency, "USD");
});

test("resetSettings restores USD and half-up", () => {
  configure({ currency: "EUR", roundingMode: "down" });
  resetSettings();
  assert.deepEqual(getSettings(), { currency: "USD", roundingMode: "half-up" });
});

test("currencyInfo describes the configured currency", () => {
  configure({ currency: "JPY" });
  assert.deepEqual(currencyInfo(), { code: "JPY", symbol: "¥", decimals: 0 });
});

test("add, subtract, sum, compare, min and max work on minor units", () => {
  assert.equal(addMoney(150, 275), 425);
  assert.equal(subtractMoney(150, 275), -125);
  assert.equal(sumMoney([1, 2, 3]), 6);
  assert.equal(sumMoney([]), 0);
  assert.equal(compareMoney(1, 2), -1);
  assert.equal(compareMoney(2, 2), 0);
  assert.equal(maxMoney(5, 9), 9);
  assert.equal(minMoney(5, 9), 5);
});

test("allocateCents splits with the largest remainder and keeps the total", () => {
  assert.deepEqual(allocateCents(100, [1, 1, 1]), [34, 33, 33]);
  assert.deepEqual(allocateCents(10, [3, 1]), [8, 2]);
  assert.deepEqual(allocateCents(5, [0, 0]), [0, 0]);
  assert.deepEqual(allocateCents(5, []), []);
});

test("formatMoney groups thousands and handles negatives", () => {
  assert.equal(formatMoney(123450), "$1,234.50");
  assert.equal(formatMoney(-99), "-$0.99");
  assert.equal(formatMoney(5, { symbol: false }), "0.05");
});

test("formatMoney follows the configured currency", () => {
  configure({ currency: "EUR" });
  assert.equal(formatMoney(100000), "€1,000.00");
  configure({ currency: "JPY" });
  assert.equal(formatMoney(1500), "¥1,500");
});

test("parseMoney reads formatted text", () => {
  assert.equal(parseMoney("$1,234.50"), 123450);
  assert.equal(parseMoney(" 12.5 "), 1250);
  assert.throws(() => parseMoney("12 dollars"), TypeError);
});

test("validateSku and validateEmail", () => {
  assert.equal(validateSku("ABC-1234"), true);
  assert.equal(validateSku("abc-1234"), false);
  assert.equal(validateSku(12), false);
  assert.equal(validateEmail("a@b.co"), true);
  assert.equal(validateEmail("a@b"), false);
});

test("validateLine reports each problem with the line number", () => {
  assert.deepEqual(validateLine({ sku: "ABC-1234", qty: 1, unitPrice: 5 }, 0), []);
  assert.deepEqual(validateLine({ sku: "x", qty: 0, unitPrice: -1 }, 2), [
    "line 3: bad sku",
    "line 3: qty must be a positive integer",
    "line 3: unitPrice must not be negative",
  ]);
  assert.deepEqual(validateLine(null), ["line 1: must be an object"]);
});

test("validateAddress requires fields and a known region", () => {
  assert.deepEqual(validateAddress(address), []);
  assert.deepEqual(validateAddress({ ...address, city: " ", region: "FR" }), ["address: city is required", "address: unknown region"]);
  assert.deepEqual(validateAddress(undefined), ["address: must be an object"]);
});

test("validateOrder accepts a good order", () => {
  assert.deepEqual(validateOrder(makeOrder()), { valid: true, errors: [] });
});

test("validateOrder collects errors from every part", () => {
  const result = validateOrder({ email: "nope", lines: [], address: {} });
  assert.equal(result.valid, false);
  assert.equal(result.errors[0], "order: bad email");
  assert.equal(result.errors[1], "order: needs at least one line");
  assert.ok(result.errors.includes("address: unknown region"));
});

test("validateOrder compares the order currency with the configured currency", () => {
  assert.equal(validateOrder(makeOrder({ currency: "EUR" })).valid, false);
  configure({ currency: "EUR" });
  assert.equal(validateOrder(makeOrder({ currency: "EUR" })).valid, true);
});

test("assertValidOrder throws a ValidationError with the messages", () => {
  assert.throws(() => assertValidOrder(makeOrder({ email: "x" })), (error) => {
    assert.ok(error instanceof ValidationError);
    assert.ok(error instanceof Error);
    assert.equal(error.name, "ValidationError");
    assert.deepEqual(error.errors, ["order: bad email"]);
    assert.equal(error.message, "invalid order: order: bad email");
    return true;
  });
  const order = makeOrder();
  assert.equal(assertValidOrder(order), order);
});

test("normalizeOrder cleans a copy and fills defaults", () => {
  const raw = makeOrder({ email: "  ADA@Example.com ", lines: [{ sku: " abc-1234 ", unitPrice: 3 }], address: { ...address, city: " Town " } });
  const clean = normalizeOrder(raw);
  assert.equal(clean.email, "ada@example.com");
  assert.equal(clean.lines[0].sku, "ABC-1234");
  assert.equal(clean.lines[0].qty, 1);
  assert.equal(clean.address.city, "Town");
  assert.equal(clean.currency, "USD");
  assert.equal(raw.lines[0].sku, " abc-1234 ");
});

test("supportedRegions returns a copy of the region list", () => {
  const regions = supportedRegions();
  assert.equal(regions.length, 6);
  regions.push("XX");
  assert.equal(supportedRegions().length, 6);
});

test("partitionOrders separates valid and invalid orders", () => {
  const good = makeOrder();
  const bad = makeOrder({ email: "bad" });
  const result = partitionOrders([good, bad]);
  assert.deepEqual(result.valid, [good]);
  assert.equal(result.invalid.length, 1);
  assert.equal(result.invalid[0].order, bad);
  assert.deepEqual(result.invalid[0].errors, ["order: bad email"]);
});

test("taxRateFor looks up rates and caches them", () => {
  assert.equal(taxRateFor("DE"), 0.19);
  assert.equal(taxRateFor("FR"), 0);
  assert.equal(taxCacheSize(), 2);
  TAX_RATES.DE = 0.2;
  assert.equal(taxRateFor("DE"), 0.19);
  clearTaxCache();
  assert.equal(taxRateFor("DE"), 0.2);
  TAX_RATES.DE = 0.19;
});

test("registerCoupon validates its definition", () => {
  assert.throws(() => registerCoupon("X", { type: "bogo", value: 1 }), RangeError);
  assert.throws(() => registerCoupon("X", { type: "percent", value: 120 }), RangeError);
  registerCoupon("save10", { type: "percent", value: 10 });
  assert.deepEqual(getCoupon("SAVE10"), { type: "percent", value: 10, uses: 0 });
  assert.equal(getCoupon("nothing"), undefined);
});

test("redeemCoupon counts uses and stops at maxUses", () => {
  registerCoupon("once", { type: "fixed", value: 5, maxUses: 1 });
  assert.equal(redeemCoupon("ONCE").uses, 1);
  assert.throws(() => redeemCoupon("once"), /coupon used up: ONCE/);
  assert.throws(() => redeemCoupon("missing"), /unknown coupon: missing/);
});

test("lineSubtotal and discountAmount", () => {
  assert.equal(lineSubtotal({ unitPrice: 19.99, qty: 3 }), 5997);
  assert.equal(discountAmount(1000, { type: "percent", value: 15 }), 150);
  assert.equal(discountAmount(1000, { type: "fixed", value: 25 }), 1000);
  assert.equal(discountAmount(1000, undefined), 0);
});

test("shippingCost is free from 100.00 and adds a JP surcharge", () => {
  assert.equal(shippingCost(10000, "DE"), 0);
  assert.equal(shippingCost(9999, "DE"), 599);
  assert.equal(shippingCost(500, "JP"), 999);
});

test("shippingCost uses the configured currency decimals", () => {
  configure({ currency: "JPY" });
  assert.equal(shippingCost(100, "GB"), 0);
  assert.equal(shippingCost(99, "GB"), 6);
});

test("priceOrder computes the totals for a plain order", () => {
  const priced = priceOrder(makeOrder());
  assert.equal(priced.subtotal, 2000);
  assert.equal(priced.discount, 0);
  assert.equal(priced.shipping, 599);
  assert.equal(priced.tax, 145);
  assert.equal(priced.total, 2744);
});

test("priceOrder spreads a coupon over the lines and taxes the discounted amount", () => {
  registerCoupon("TEN", { type: "percent", value: 10 });
  const order = makeOrder({
    couponCode: "ten",
    address: { ...address, region: "DE" },
    lines: [{ sku: "ABC-1234", qty: 1, unitPrice: 10 }, { sku: "DEF-0001", qty: 1, unitPrice: 20.01 }],
  });
  const priced = priceOrder(order);
  assert.equal(priced.subtotal, 3001);
  assert.equal(priced.discount, 300);
  assert.deepEqual(priced.lines.map((line) => line.discount), [100, 200]);
  assert.deepEqual(priced.lines.map((line) => line.tax), [171, 342]);
  assert.equal(priced.tax, 513);
  assert.equal(priced.total, 3001 - 300 + 599 + 513);
});

test("checkoutOrder validates, redeems the coupon and prices", () => {
  registerCoupon("ONE", { type: "fixed", value: 1, maxUses: 1 });
  const first = checkoutOrder(makeOrder({ couponCode: "one" }));
  assert.equal(first.discount, 100);
  assert.throws(() => checkoutOrder(makeOrder({ couponCode: "one" })), /coupon used up/);
  assert.throws(() => checkoutOrder(makeOrder({ email: "bad" })), ValidationError);
  assert.equal(getCoupon("ONE").uses, 1);
});

test("resetAll clears settings, the tax cache and coupons", () => {
  configure({ currency: "GBP" });
  taxRateFor("GB");
  registerCoupon("A", { type: "fixed", value: 1 });
  resetAll();
  assert.equal(getSettings().currency, "USD");
  assert.equal(taxCacheSize(), 0);
  assert.equal(getCoupon("A"), undefined);
});

test("renderTable aligns columns", () => {
  const text = renderTable([{ a: "x", b: 5 }, { a: "long", b: 120 }], [{ key: "a", title: "A" }, { key: "b", title: "B", align: "right" }]);
  assert.equal(text, ["A       B", "----  ---", "x       5", "long  120"].join("\n"));
});

test("formatReceipt prints lines and a summary in the configured currency", () => {
  const receipt = formatReceipt(priceOrder(makeOrder()), { title: "Order 1" });
  assert.match(receipt, /^Order 1\n\nSKU {7}Qty {2}Amount\n/);
  assert.match(receipt, /ABC-1234 {4}2 {2}\$20\.00/);
  assert.match(receipt, /Total: \$27\.44$/);
  configure({ currency: "EUR" });
  assert.match(formatReceipt(priceOrder(makeOrder())), /Total: €27\.44$/);
});

test("toCsv writes plain decimal amounts", () => {
  const csv = toCsv(priceOrder(makeOrder({ lines: [{ sku: "ABC-1234", qty: 1, unitPrice: 1234.5 }] })));
  assert.equal(csv, "sku,qty,subtotal,discount,net,tax\nABC-1234,1,1234.50,0.00,1234.50,89.50");
});

test("summarizeOrders totals revenue, units and the average", () => {
  const orders = [makeOrder(), makeOrder({ address: { ...address, region: "GB" } })];
  const summary = summarizeOrders(orders);
  assert.equal(summary.orders, 2);
  assert.equal(summary.units, 4);
  assert.equal(summary.revenue, 2744 + 2999);
  assert.deepEqual(summarizeOrders([]), { orders: 0, units: 0, revenue: 0, tax: 0, average: 0 });
});

test("groupByRegion and topSkus", () => {
  const orders = [
    makeOrder(),
    makeOrder({ lines: [{ sku: "ZZZ-0001", qty: 5, unitPrice: 1 }, { sku: "ABC-1234", qty: 3, unitPrice: 1 }] }),
    makeOrder({ address: { ...address, region: "DE" }, lines: [{ sku: "AAA-0001", qty: 5, unitPrice: 1 }] }),
  ];
  const groups = groupByRegion(orders);
  assert.equal(groups["US-CA"].orders, 2);
  assert.equal(groups.DE.orders, 1);
  assert.deepEqual(topSkus(orders, 2), [{ sku: "AAA-0001", units: 5 }, { sku: "ABC-1234", units: 5 }]);
});

test("dailyReport lists the summary and the regions in order", () => {
  const report = dailyReport([makeOrder(), makeOrder({ address: { ...address, region: "DE" } })]);
  assert.match(report, /^Orders: 2\nUnits: 4\n/);
  assert.ok(report.indexOf("DE") < report.indexOf("US-CA"));
  assert.match(report, /Revenue: \$/);
});

test("half-even rounding changes the tax on a priced order", () => {
  configure({ roundingMode: "half-even" });
  const priced = priceOrder(makeOrder({ lines: [{ sku: "ABC-1234", qty: 1, unitPrice: 0.5 }], address: { ...address, region: "GB" } }));
  assert.equal(priced.tax, 10);
  assert.equal(priced.shipping, 599);
});
