import { a } from './a.mjs';

export function c(n) {
  return n <= 0 ? 'c' : 'c' + a(n - 1);
}
