import assert from 'node:assert';
import { test } from './harness.mjs';
import { chunk, unique, flatten } from '../src/arrays.mjs';
import { summarize } from '../src/stats.mjs';

test('chunk splits a list', () => assert.deepStrictEqual(chunk([1, 2, 3, 4, 5], 2), [[1, 2], [3, 4], [5]]));
test('unique removes repeats', () => assert.deepStrictEqual(unique([1, 1, 2, 3, 3]), [1, 2, 3]));
test('flatten flattens deeply', () => assert.deepStrictEqual(flatten([1, [2, [3, [4]]]]), [1, 2, 3, 4]));
test('summarize counts', () => assert.deepStrictEqual(summarize([2, 2, 4]), { count: 3, sum: 8, average: 8 / 3, distinct: 2 }));
