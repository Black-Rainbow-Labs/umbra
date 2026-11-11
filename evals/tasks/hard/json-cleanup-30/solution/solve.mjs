// Reference solution: cleans every data/customers-NN.json in the workspace following the task rules.

import fs from "node:fs";
import path from "node:path";

const DROP = ["internal_score", "notes", "ssn_last4", "debug"];
const ORDER = ["id", "name", "email", "phone", "signup", "country", "tags", "last_login"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/**
 * Returns the date as YYYY-MM-DD when the parts form a real calendar date, otherwise null.
 * @param year - four digit year
 * @param month - 1 to 12
 * @param day - 1 to 31
 */
function realDate(year, month, day) {
  const date = new Date(Date.UTC(year, month - 1, day));
  const ok = date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  return ok ? `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}` : null;
}

/**
 * Converts a signup value in one of the four accepted formats to YYYY-MM-DD, or null.
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
  return null;
}

/**
 * Converts a phone value to `+1 AAA-BBB-CCCC` or null.
 * @param value - the raw value
 */
function cleanPhone(value) {
  if (typeof value !== "string") return null;
  const cut = value.search(/x|ext/i);
  const digits = (cut === -1 ? value : value.slice(0, cut)).replace(/\D/g, "");
  const ten = digits.length === 10 ? digits : digits.length === 11 && digits[0] === "1" ? digits.slice(1) : null;
  return ten ? `+1 ${ten.slice(0, 3)}-${ten.slice(3, 6)}-${ten.slice(6)}` : null;
}

/**
 * Cleans one customer record (step 1 and the phone and signup rules).
 * @param customer - the raw customer
 */
function cleanCustomer(customer) {
  const out = {};
  for (const [key, value] of Object.entries(customer)) {
    if (DROP.includes(key)) continue;
    if (key === "name") out.name = typeof value === "string" ? value.replace(/[ \t]+/g, " ").trim() : value;
    else if (key === "email") out.email = typeof value === "string" ? value.trim().toLowerCase() : value;
    else if (key === "country") out.country = typeof value === "string" ? value.trim().toUpperCase() : value;
    else if (key === "phone") out.phone = cleanPhone(value);
    else if (key === "signup") out.signup = cleanDate(value);
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
 * Merges the duplicates of one email into one customer.
 * @param group - cleaned customers in file order
 */
function mergeGroup(group) {
  const merged = { id: group[0].id };
  const keys = new Set(group.flatMap((customer) => Object.keys(customer)));
  for (const key of keys) {
    if (key === "id") continue;
    if (key === "tags") { merged.tags = [...new Set(group.flatMap((customer) => customer.tags || []))]; continue; }
    const last = [...group].reverse().find((customer) => usable(customer[key]));
    if (last) merged[key] = last[key];
    else if (key in group[0]) merged[key] = group[0][key];
  }
  return merged;
}

/**
 * Cleans the customers array of a document.
 * @param customers - the raw customers
 */
function cleanCustomers(customers) {
  const groups = new Map();
  for (const customer of customers.map(cleanCustomer)) {
    if (!usable(customer.email)) continue;
    if (!groups.has(customer.email)) groups.set(customer.email, []);
    groups.get(customer.email).push(customer);
  }
  const merged = [...groups.values()].map(mergeGroup).sort((a, b) => a.id - b.id);
  return merged.map((customer) => Object.fromEntries(ORDER.filter((key) => key in customer).map((key) => [key, customer[key]])));
}

/**
 * Cleans every data file in the workspace in place.
 * @param workspaceDir - the workspace folder
 */
export async function solve(workspaceDir) {
  const dataDir = path.join(workspaceDir, "data");
  for (const name of fs.readdirSync(dataDir)) {
    const file = path.join(dataDir, name);
    const doc = JSON.parse(fs.readFileSync(file, "utf8").replace(/^﻿/, ""));
    doc.customers = cleanCustomers(doc.customers);
    doc.count = doc.customers.length;
    fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n");
  }
}
