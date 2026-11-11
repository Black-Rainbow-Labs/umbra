export default function memoize(fn) {
  const seen = new Map();
  function wrapped(...args) {
    const key = JSON.stringify(args);
    if (!seen.has(key)) seen.set(key, fn(...args));
    return seen.get(key);
  }
  wrapped.size = () => seen.size;
  return wrapped;
}
