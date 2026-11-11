// Helper for the hidden module tests: imports a workspace file by its path under WORKSPACE_DIR.
import path from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Imports a module from the workspace.
 * @param relative - path under the workspace, for example src/money.mjs
 */
export async function load(relative) {
  return import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);
}

export const NAMES = {
  money: ["configure", "getSettings", "resetSettings", "currencyInfo", "roundValue", "toCents", "fromCents", "addMoney", "subtractMoney", "multiplyMoney", "sumMoney", "compareMoney", "allocateCents", "formatMoney", "parseMoney", "maxMoney", "minMoney"],
  validate: ["ValidationError", "validateSku", "validateEmail", "validateLine", "validateAddress", "validateOrder", "assertValidOrder", "normalizeOrder", "supportedRegions", "partitionOrders"],
  pricing: ["TAX_RATES", "taxRateFor", "taxCacheSize", "clearTaxCache", "registerCoupon", "getCoupon", "clearCoupons", "redeemCoupon", "lineSubtotal", "discountAmount", "shippingCost", "allocateDiscount", "priceOrder", "checkoutOrder", "resetAll"],
  report: ["renderTable", "formatReceipt", "toCsv", "summarizeOrders", "groupByRegion", "topSkus", "dailyReport"],
};
