export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

export function sum(list) {
  return list.reduce((total, n) => total + n, 0);
}

export function average(list) {
  return list.length === 0 ? 0 : sum(list) / list.length;
}
