import { sum, average } from './numbers.mjs';
import { unique } from './arrays.mjs';

export function summarize(list) {
  return { count: list.length, sum: sum(list), average: average(list), distinct: unique(list).length };
}
