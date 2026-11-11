import { clamp } from './numbers.mjs';

export function chunk(list, size) {
  const step = clamp(Math.floor(size), 1, Math.max(1, list.length));
  const out = [];
  for (let i = 0; i < list.length; i += step) out.push(list.slice(i, i + step));
  return out;
}

export function unique(list) {
  return [...new Set(list)];
}

export function flatten(list) {
  return list.reduce((out, item) => out.concat(Array.isArray(item) ? flatten(item) : [item]), []);
}
