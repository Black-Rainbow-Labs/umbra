function isPlain(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function pick(source, keys) {
  const out = {};
  for (const key of keys) if (key in source) out[key] = source[key];
  return out;
}

export function omit(source, keys) {
  const out = {};
  for (const key of Object.keys(source)) if (!keys.includes(key)) out[key] = source[key];
  return out;
}

export function deepMerge(base, extra) {
  const out = { ...base };
  for (const key of Object.keys(extra || {})) {
    out[key] = isPlain(out[key]) && isPlain(extra[key]) ? deepMerge(out[key], extra[key]) : extra[key];
  }
  return out;
}
