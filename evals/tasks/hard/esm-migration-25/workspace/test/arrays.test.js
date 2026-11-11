const assert = require('assert');
const { test } = require('./harness');
const { chunk, unique, flatten } = require('../src/arrays');
const { summarize } = require('../src/stats');

test('chunk splits a list', () => assert.deepStrictEqual(chunk([1, 2, 3, 4, 5], 2), [[1, 2], [3, 4], [5]]));
test('unique removes repeats', () => assert.deepStrictEqual(unique([1, 1, 2, 3, 3]), [1, 2, 3]));
test('flatten flattens deeply', () => assert.deepStrictEqual(flatten([1, [2, [3, [4]]]]), [1, 2, 3, 4]));
test('summarize counts', () => assert.deepStrictEqual(summarize([2, 2, 4]), { count: 3, sum: 8, average: 8 / 3, distinct: 2 }));
