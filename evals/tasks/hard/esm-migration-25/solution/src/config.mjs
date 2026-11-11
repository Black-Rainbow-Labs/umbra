import { readFileSync } from 'node:fs';
import { deepMerge } from './objects.mjs';

const defaults = JSON.parse(readFileSync(new URL('../data/defaults.json', import.meta.url), 'utf8'));

export function getDefaults() {
  return JSON.parse(JSON.stringify(defaults));
}

export function getConfig(overrides) {
  return deepMerge(getDefaults(), overrides || {});
}
