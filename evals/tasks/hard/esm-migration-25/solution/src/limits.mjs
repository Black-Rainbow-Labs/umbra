import { readFileSync } from 'node:fs';

const limits = JSON.parse(readFileSync(new URL('../data/limits.json', import.meta.url), 'utf8'));

export function getLimit(name) {
  if (!(name in limits)) throw new RangeError('no limit: ' + name);
  return limits[name];
}

export function limitNames() {
  return Object.keys(limits).sort();
}
