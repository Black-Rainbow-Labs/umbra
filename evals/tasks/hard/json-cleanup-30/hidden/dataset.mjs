// Builds the 30 messy input files and, by construction, the clean text each file must become.
// Each file starts from "truth" customers (already clean) and then makes messy copies of them, so the
// expected output does not come from a cleanup implementation.

export const FILE_COUNT = 30;
const KEY_ORDER = ["id", "name", "email", "phone", "signup", "country", "tags", "last_login"];
const DROPPED = ["internal_score", "notes", "ssn_last4", "debug"];
const WORDS_A = ["ada", "grace", "alan", "linus", "maria", "dennis", "barbara", "ken", "radia", "tim", "jose", "zoe", "lukas", "amelie", "soren", "noor", "priya", "wei", "hiro", "fatima", "omar", "ines", "karl", "mei"];
const WORDS_B = ["lovelace", "hopper", "turing", "torvalds", "garcia", "ritchie", "liskov", "thompson", "perlman", "berners", "mueller", "nowak", "dubois", "larsen", "khan", "patel", "chen", "sato", "ali", "silva", "costa", "weber", "stone", "brook"];
const FIRST_NAMES = ["Ada", "Grace", "Alan", "Linus", "Margaret", "Dennis", "Barbara", "Ken", "Radia", "Tim", "José", "Zoë", "Łukasz", "Amélie", "Søren", "Noor", "Priya", "Wei", "Hiro", "Fatima", "Omar", "Inès", "Karl", "Mei"];
const LAST_NAMES = ["Lovelace", "Hopper", "Turing", "Torvalds", "García", "Ritchie", "Liskov", "Thompson", "Perlman", "Berners-Lee", "Müller", "Nowak", "Dubois", "Larsen", "Khan", "Patel", "Chen", "Satō", "Ali", "Silva", "Costa", "Weber", "O'Neil", "Brook"];
const DOMAINS = ["example.com", "mail.test", "corp.example", "inbox.test"];
const COUNTRIES = ["US", "GB", "DE", "CA", "FR", "JP", "BR", "IN", "MX", "AU"];
const TAGS = ["vip", "beta", "newsletter", "trial", "partner", "churn-risk", "enterprise", "student", "press", "legacy"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/**
 * Returns a deterministic random number generator (mulberry32) for a seed.
 * @param seed - integer seed
 */
function makeRng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Helpers bound to one generator. */
function tools(rng) {
  const int = (n) => Math.floor(rng() * n);
  const pick = (list) => list[int(list.length)];
  const chance = (p) => rng() < p;
  const shuffle = (list) => { const out = [...list]; for (let i = out.length - 1; i > 0; i -= 1) { const j = int(i + 1); [out[i], out[j]] = [out[j], out[i]]; } return out; };
  const pad = (n, w = 2) => String(n).padStart(w, "0");
  return { int, pick, chance, shuffle, pad };
}

/**
 * Builds one clean customer (the expected result for it).
 * @param t - the tools
 * @param id - the customer id
 * @param used - set of emails already used in the file
 */
function truthCustomer(t, id, used) {
  let email;
  do { email = `${t.pick(WORDS_A)}.${t.pick(WORDS_B)}${id}@${t.pick(DOMAINS)}`; } while (used.has(email));
  used.add(email);
  const area = 200 + t.int(800);
  const phone = t.chance(0.15) ? null : `+1 ${area}-${200 + t.int(800)}-${String(t.int(10000)).padStart(4, "0")}`;
  const year = 2015 + t.int(10);
  const signup = t.chance(0.12) ? null : isoDate(year, 1 + t.int(12), 1 + t.int(28), t);
  const tags = t.shuffle(TAGS).slice(0, t.int(4));
  const customer = { id, name: `${t.pick(FIRST_NAMES)} ${t.pick(LAST_NAMES)}`, email, phone, signup, country: t.pick(COUNTRIES), tags };
  if (t.chance(0.7)) customer.last_login = `${2020 + t.int(5)}-${t.pad(1 + t.int(12))}-${t.pad(1 + t.int(28))}T${t.pad(t.int(24))}:${t.pad(t.int(60))}:00Z`;
  if (t.chance(0.08)) { customer.signup = "2020-02-29"; }
  return customer;
}

/** Formats a date as YYYY-MM-DD. */
function isoDate(year, month, day, t) {
  return `${year}-${t.pad(month)}-${t.pad(day)}`;
}

/**
 * Writes a clean date in one of the four accepted input formats, with optional surrounding spaces.
 * @param t - the tools
 * @param iso - YYYY-MM-DD
 */
function messyDate(t, iso) {
  const [y, m, d] = iso.split("-").map(Number);
  const text = t.pick([
    iso,
    `${m}/${d}/${y}`,
    `${t.pad(m)}/${t.pad(d)}/${y}`,
    `${d}.${m}.${y}`,
    `${t.pad(d)}.${t.pad(m)}.${y}`,
    `${MONTHS[m - 1]} ${d}, ${y}`,
    `${MONTHS[m - 1]} ${t.pad(d)}, ${y}`,
  ]);
  return t.chance(0.3) ? `  ${text} ` : text;
}

/** A date value that must become null. */
function badDate(t) {
  return t.pick([null, "", "N/A", "2021/03/14", "31.02.2021", "13/01/2021", "02/30/2021", "March 14th", "3-14-2021", "2021-13-01", "Sept 5, 2021", "00/10/2021", "29.02.2021", "12/25/21"]);
}

/** Writes a clean phone number in one of several accepted input formats, sometimes with an extension. */
function messyPhone(t, phone) {
  const [, a, b, c] = /^\+1 (\d{3})-(\d{3})-(\d{4})$/.exec(phone);
  const body = t.pick([`(${a}) ${b}-${c}`, `${a}.${b}.${c}`, `${a} ${b} ${c}`, `1-${a}-${b}-${c}`, `+1 (${a}) ${b}-${c}`, `${a}${b}${c}`, `+1${a}${b}${c}`, `1 ${a} ${b} ${c}`, `${a}-${b}-${c}`, phone]);
  const extension = t.chance(0.3) ? t.pick([" x123", " ext. 45", " Ext 9", "x7", " EXT 2200"]) : "";
  return body + extension;
}

/** A phone value that must become null. */
function badPhone(t) {
  return t.pick([null, "", "555-0132", "n/a", "+44 20 7946 0958", "12345", "   ", "415-555-013"]);
}

/** Changes the case of some letters of an email and adds spaces around it. */
function messyEmail(t, email) {
  const flipped = [...email].map((ch) => (t.chance(0.25) ? ch.toUpperCase() : ch)).join("");
  return t.chance(0.3) ? ` ${flipped}  ` : flipped;
}

/** Adds extra spaces and tabs to a name. */
function messyName(t, name) {
  const inner = name.replace(/ /g, () => t.pick([" ", "  ", "\t", " \t "]));
  return t.chance(0.5) ? `  ${inner}\t` : inner;
}

/** Writes tags with changed case, spaces, empty entries and repeats. */
function messyTags(t, tags) {
  const out = [];
  for (const tag of tags) {
    out.push(t.pick([tag, tag.toUpperCase(), ` ${tag} `, tag[0].toUpperCase() + tag.slice(1)]));
    if (t.chance(0.2)) out.push(tag.toUpperCase());
  }
  if (t.chance(0.3)) out.splice(t.int(out.length + 1), 0, t.pick(["", "  "]));
  return out;
}

/** Messy values for the scalar fields of a clean customer. */
function messyValues(t, truth) {
  return {
    name: messyName(t, truth.name),
    email: messyEmail(t, truth.email),
    phone: truth.phone === null ? badPhone(t) : messyPhone(t, truth.phone),
    signup: truth.signup === null ? badDate(t) : messyDate(t, truth.signup),
    country: t.pick([truth.country, truth.country.toLowerCase(), ` ${truth.country.toLowerCase()} `]),
    tags: messyTags(t, truth.tags),
  };
}

/** Adds the fields that must be dropped, in random numbers. */
function addDroppedFields(t, record) {
  if (t.chance(0.6)) record.internal_score = t.int(100);
  if (t.chance(0.5)) record.notes = t.pick(["call back", "VIP  ", "", "moved to EU"]);
  if (t.chance(0.3)) record.ssn_last4 = String(1000 + t.int(9000));
  if (t.chance(0.2)) record.debug = { trace: "x", n: t.int(5) };
  return record;
}

/** Puts a record's keys into random order. */
function shuffleKeys(t, record) {
  const out = {};
  for (const key of t.shuffle(Object.keys(record))) out[key] = record[key];
  return out;
}

/** A value that counts as empty after cleaning: missing (undefined), null or blank. */
function emptyLike(t, kind) {
  const options = kind === "text" ? [undefined, null, "", "   "] : [undefined, null];
  return t.pick(options);
}

/**
 * Builds the messy duplicates of one clean customer. The first duplicate has the clean id. For every
 * scalar field one duplicate holds the clean value, the ones before it hold other valid values and the
 * ones after it are empty, so the last duplicate with a value is the clean one.
 * @param t - the tools
 * @param truth - the clean customer
 * @param count - number of duplicates (2 or 3)
 * @param nextId - function returning a new unused id
 */
function makeDuplicates(t, truth, count, nextId) {
  const records = Array.from({ length: count }, (_, i) => ({ id: i === 0 ? truth.id : nextId() }));
  const stale = { name: () => messyName(t, `${t.pick(FIRST_NAMES)} ${t.pick(LAST_NAMES)}`), phone: () => messyPhone(t, `+1 ${300 + t.int(600)}-555-${String(t.int(10000)).padStart(4, "0")}`), signup: () => messyDate(t, "2014-06-30"), country: () => t.pick(["xx", " zz "]), last_login: () => "2019-01-01T00:00:00Z" };
  for (const record of records) record.email = messyEmail(t, truth.email);
  for (const field of ["name", "phone", "signup", "country", "last_login"]) {
    const hasValue = truth[field] !== undefined && truth[field] !== null;
    if (!hasValue) {
      records.forEach((record) => { if (field in truth) record[field] = field === "phone" ? badPhone(t) : field === "signup" ? badDate(t) : null; });
      continue;
    }
    const holder = t.int(count);
    const clean = messyValues(t, truth)[field] ?? truth[field];
    records.forEach((record, i) => {
      if (i === holder) record[field] = field === "last_login" ? truth.last_login : clean;
      else if (i < holder) record[field] = stale[field]();
      else if (field === "last_login") { const v = emptyLike(t, "other"); if (v !== undefined) record[field] = v; }
      else { const v = emptyLike(t, "text"); if (v !== undefined) record[field] = v; else if (field === "phone" || field === "signup") record[field] = field === "phone" ? badPhone(t) : badDate(t); }
    });
  }
  splitTags(t, truth.tags, records);
  return records.map((record) => shuffleKeys(t, addDroppedFields(t, record)));
}

/** Spreads clean tags over duplicates so that the union in first seen order is the clean list. */
function splitTags(t, tags, records) {
  records.forEach((record) => { record.tags = []; });
  const started = new Set();
  tags.forEach((tag, index) => {
    const owner = Math.min(records.length - 1, Math.floor((index * records.length) / Math.max(1, tags.length)));
    const first = !started.has(owner);
    started.add(owner);
    records[owner].tags.push(tag);
    if (first && owner > 0 && t.chance(0.3)) records[owner - 1].tags.push(tag.toUpperCase());
  });
  records.forEach((record) => { record.tags = messyTags(t, record.tags); });
}

/** A record with no usable email. It must be dropped. */
function emailLessRecord(t, id, used) {
  const truth = truthCustomer(t, id, used);
  const messy = messyValues(t, truth);
  const record = { id, name: messy.name, phone: messy.phone, signup: messy.signup, country: messy.country, tags: messy.tags };
  const style = t.int(4);
  if (style === 1) record.email = null;
  if (style === 2) record.email = "";
  if (style === 3) record.email = "   ";
  return shuffleKeys(t, addDroppedFields(t, record));
}

/**
 * Builds the input records and the clean customers for one file.
 * @param t - the tools
 */
function buildFileRecords(t) {
  const ids = new Set();
  const newId = () => { let id; do { id = t.chance(0.3) ? 1 + t.int(99) : t.chance(0.5) ? 100 + t.int(900) : 1000 + t.int(9000); } while (ids.has(id)); ids.add(id); return id; };
  const used = new Set();
  const truths = Array.from({ length: 10 + t.int(14) }, () => truthCustomer(t, newId(), used));
  const decoys = makeDecoys(t, truths, newId, used);
  const groups = [];
  for (const truth of [...truths, ...decoys]) {
    const roll = t.int(100);
    const size = roll < 70 ? 1 : roll < 93 ? 2 : 3;
    groups.push(size === 1 ? [shuffleKeys(t, addDroppedFields(t, { id: truth.id, ...messyValues(t, truth), ...(truth.last_login ? { last_login: truth.last_login } : {}) }))] : makeDuplicates(t, truth, size, newId));
  }
  for (let i = 0; i < 1 + t.int(3); i += 1) groups.push([emailLessRecord(t, newId(), used)]);
  return { records: interleave(t, groups), truths: [...truths, ...decoys] };
}

/** Customers whose emails look like another customer's email but are different, so they must not merge. */
function makeDecoys(t, truths, newId, used) {
  const decoys = [];
  for (const source of truths.slice(0, 3 + t.int(3))) {
    const [local, domain] = source.email.split("@");
    const email = t.pick([`${local}+promo@${domain}`, `${local}@${domain.replace(/\.[a-z]+$/, ".org")}`, `x${local}@${domain}`]);
    if (used.has(email)) continue;
    used.add(email);
    decoys.push({ ...truthCustomer(t, newId(), used), email });
  }
  return decoys;
}

/** Interleaves groups of records at random positions, keeping the order inside each group. */
function interleave(t, groups) {
  const slots = t.shuffle(groups.flatMap((group, index) => group.map(() => index)));
  const next = groups.map(() => 0);
  return slots.map((index) => groups[index][next[index]++]);
}

/** Serializes a document in a messy way: indent, line endings, byte order mark, escapes. */
function messyText(t, doc, index) {
  const indent = [2, 4, "\t", 0][index % 4];
  let text = JSON.stringify(doc, null, indent || undefined);
  if (index % 3 === 0) text = text.replace(/[\u0080-￿]/g, (ch) => `\\u${ch.charCodeAt(0).toString(16).padStart(4, "0")}`);
  if (index % 7 === 2) text = text.replace(/\n/g, "\r\n");
  if (t.chance(0.5)) text += index % 7 === 2 ? "\r\n" : "\n";
  return (index % 5 === 0 ? "﻿" : "") + text;
}

/**
 * Builds all files. Returns [{ name, input, expected }] for customers-01.json to customers-30.json.
 * @param seed - the task seed
 */
export function buildDataset(seed) {
  const files = [];
  for (let index = 0; index < FILE_COUNT; index += 1) {
    const t = tools(makeRng(seed * 1000 + index));
    const { records, truths } = buildFileRecords(t);
    const topKeys = t.shuffle(["source", "exportedAt", "count", "customers"]);
    const make = (customers) => Object.fromEntries(topKeys.map((key) => [key, { source: `crm-export-${String(index + 1).padStart(2, "0")}`, exportedAt: `2024-0${1 + (index % 9)}-15T08:00:00Z`, count: customers.length, customers }[key]]));
    const sorted = [...truths].sort((x, y) => x.id - y.id).map((truth) => orderedCustomer(truth));
    files.push({ name: `customers-${String(index + 1).padStart(2, "0")}.json`, input: messyText(t, make(records), index), expected: JSON.stringify(make(sorted), null, 2) + "\n" });
  }
  return files;
}

/** Returns the customer with its keys in the required order. */
function orderedCustomer(truth) {
  const out = {};
  for (const key of KEY_ORDER) if (key in truth) out[key] = truth[key];
  return out;
}

export { DROPPED };
