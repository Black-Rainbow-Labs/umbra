// Reference solution: cleans every data/customers-NN.json in the workspace following the task rules and writes
// data/summary.json.

import fs from "node:fs";
import path from "node:path";

const DROP = ["internal_score", "notes", "ssn_last4", "debug"];
const ORDER = ["id", "name", "email", "phone", "signup", "country", "tags", "balance", "last_login"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/**
 * Returns the date as YYYY-MM-DD when the parts form a real calendar date from 2010-01-01 to 2025-12-31, otherwise null.
 * @param year - four digit year
 * @param month - 1 to 12
 * @param day - 1 to 31
 */
function realDate(year, month, day) {
  const date = new Date(Date.UTC(year, month - 1, day));
  const ok = date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  if (!ok || year < 2010 || year > 2025) return null;
  return `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

/**
 * Converts a signup value in one of the five accepted formats to YYYY-MM-DD, or null.
 * @param value - the raw value
 */
function cleanDate(value) {
  if (typeof value !== "string") return null;
  const text = value.trim();
  let m;
  if ((m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text))) return realDate(+m[1], +m[2], +m[3]);
  if ((m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(text))) return realDate(+m[3], +m[1], +m[2]);
  if ((m = /^(\d{1,2})\.(\d{1,2})\.(\d{4})$/.exec(text))) return realDate(+m[3], +m[2], +m[1]);
  if ((m = /^([A-Z][a-z]{2}) (\d{1,2}), (\d{4})$/.exec(text)) && MONTHS.includes(m[1])) return realDate(+m[3], MONTHS.indexOf(m[1]) + 1, +m[2]);
  if ((m = /^(\d{1,2}) ([A-Z][a-z]{2}) (\d{4})$/.exec(text)) && MONTHS.includes(m[2])) return realDate(+m[3], MONTHS.indexOf(m[2]) + 1, +m[1]);
  return null;
}

/**
 * Converts a phone value to `+1 AAA-BBB-CCCC` or null. Area codes and exchanges that start with 0 or 1 give null.
 * @param value - the raw value
 */
function cleanPhone(value) {
  if (typeof value !== "string") return null;
  const cut = value.search(/x|ext/i);
  const digits = (cut === -1 ? value : value.slice(0, cut)).replace(/\D/g, "");
  const ten = digits.length === 10 ? digits : digits.length === 11 && digits[0] === "1" ? digits.slice(1) : null;
  if (!ten || /[01]/.test(ten[0]) || /[01]/.test(ten[3])) return null;
  return `+1 ${ten.slice(0, 3)}-${ten.slice(3, 6)}-${ten.slice(6)}`;
}

/**
 * Converts a balance value (a JSON number of dollars or a string such as "$1,234.50" or "(12.00)") to whole cents, or null.
 * @param value - the raw value
 */
function cleanBalance(value) {
  if (typeof value === "number") return Number.isFinite(value) ? Math.round(value * 100) : null;
  if (typeof value !== "string") return null;
  let text = value.trim();
  let negative = false;
  const paren = /^\((.*)\)$/.exec(text);
  if (paren) { negative = true; text = paren[1]; } else if (text.startsWith("-")) { negative = true; text = text.slice(1); }
  const m = /^\$?(\d{1,3}(?:,\d{3})+|\d+)(?:\.(\d{1,2}))?$/.exec(text);
  if (!m) return null;
  const cents = Number(m[1].replace(/,/g, "")) * 100 + (m[2] ? Number(m[2].padEnd(2, "0")) : 0);
  return negative && cents ? -cents : cents;
}

/**
 * Cleans one customer record (step 1 and the id, phone, signup and balance rules).
 * @param customer - the raw customer
 */
function cleanCustomer(customer) {
  const out = {};
  for (const [key, value] of Object.entries(customer)) {
    if (DROP.includes(key)) continue;
    if (key === "id") out.id = typeof value === "string" && /^\d+$/.test(value) ? Number(value) : value;
    else if (key === "name") out.name = typeof value === "string" ? value.replace(/[ \t]+/g, " ").trim().normalize("NFC") : value;
    else if (key === "email") out.email = typeof value === "string" ? value.trim().toLowerCase() : value;
    else if (key === "country") out.country = typeof value === "string" ? value.trim().toUpperCase() : value;
    else if (key === "phone") out.phone = cleanPhone(value);
    else if (key === "signup") out.signup = cleanDate(value);
    else if (key === "balance") out.balance = cleanBalance(value);
    else if (key === "tags") out.tags = [...new Set((value || []).map((tag) => String(tag).trim().toLowerCase()).filter(Boolean))];
    else out[key] = value;
  }
  return out;
}

/** Returns true when a cleaned value counts as present. */
function usable(value) {
  return value !== undefined && value !== null && value !== "";
}

/**
 * Merges the duplicates of one email into one customer. Tags are united and balances are added up.
 * @param group - cleaned customers in file order
 */
function mergeGroup(group) {
  const merged = { id: group[0].id };
  const keys = new Set(group.flatMap((customer) => Object.keys(customer)));
  for (const key of keys) {
    if (key === "id") continue;
    if (key === "tags") { merged.tags = [...new Set(group.flatMap((customer) => customer.tags || []))]; continue; }
    if (key === "balance") {
      const amounts = group.map((customer) => customer.balance).filter((amount) => Number.isInteger(amount));
      if (amounts.length) merged.balance = amounts.reduce((a, b) => a + b, 0);
      else if (key in group[0]) merged.balance = group[0].balance;
      continue;
    }
    const last = [...group].reverse().find((customer) => usable(customer[key]));
    if (last) merged[key] = last[key];
    else if (key in group[0]) merged[key] = group[0][key];
  }
  return merged;
}

/**
 * Cleans the customers array of a document and returns the customers with the counts for the summary.
 * @param customers - the raw customers
 */
function cleanCustomers(customers) {
  const groups = new Map();
  let kept = 0;
  for (const customer of customers.map(cleanCustomer)) {
    if (!usable(customer.email)) continue;
    kept += 1;
    if (!groups.has(customer.email)) groups.set(customer.email, []);
    groups.get(customer.email).push(customer);
  }
  const merged = [...groups.values()].map(mergeGroup).sort((a, b) => a.id - b.id);
  const ordered = merged.map((customer) => Object.fromEntries(ORDER.filter((key) => key in customer).map((key) => [key, customer[key]])));
  return { ordered, droppedNoEmail: customers.length - kept, merged: kept - ordered.length };
}

/**
 * Cleans every data file in the workspace in place and writes data/summary.json.
 * @param workspaceDir - the workspace folder
 */
export async function solve(workspaceDir) {
  const dataDir = path.join(workspaceDir, "data");
  const files = [];
  const totals = { records: 0, droppedNoEmail: 0, merged: 0, customers: 0, nullPhones: 0, nullSignups: 0 };
  for (const name of fs.readdirSync(dataDir).filter((n) => /^customers-\d+\.json$/.test(n)).sort()) {
    const file = path.join(dataDir, name);
    const doc = JSON.parse(fs.readFileSync(file, "utf8").replace(/^﻿/, ""));
    const records = doc.customers.length;
    const { ordered, droppedNoEmail, merged } = cleanCustomers(doc.customers);
    doc.customers = ordered;
    doc.count = ordered.length;
    fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n");
    const stats = { file: name, records, droppedNoEmail, merged, customers: ordered.length, nullPhones: ordered.filter((c) => c.phone === null).length, nullSignups: ordered.filter((c) => c.signup === null).length };
    files.push(stats);
    for (const key of Object.keys(totals)) totals[key] += stats[key];
  }
  fs.writeFileSync(path.join(dataDir, "summary.json"), JSON.stringify({ files, totals }, null, 2) + "\n");
}
