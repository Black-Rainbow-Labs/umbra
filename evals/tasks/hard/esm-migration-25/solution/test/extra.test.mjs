import assert from 'node:assert';
import { test } from './harness.mjs';
import compare, { parse, satisfies } from '../src/semver.mjs';
import { getLimit, limitNames } from '../src/limits.mjs';
import * as registry from '../src/registry.mjs';
import { FILE_MODE, nextTicket } from '../src/legacy.mjs';
import { money, percent } from '../src/format.mjs';
import { a } from '../src/ring/a.mjs';
import { listPlugins } from '../src/pluginLoader.mjs';

test('semver compares versions numerically', () => assert.strictEqual(compare('1.2.3', '1.10.0'), -1));
test('semver parses a version', () => assert.deepStrictEqual(parse('2.0.11'), { major: 2, minor: 0, patch: 11 }));
test('semver satisfies a minimum', () => assert.strictEqual(satisfies('1.4.0', '1.3.9'), true));
test('limits reads data/limits.json', () => {
  assert.strictEqual(getLimit('maxRows'), 100);
  assert.deepStrictEqual(limitNames(), ['maxCols', 'maxRows', 'timeoutMs']);
  assert.throws(() => getLimit('nope'), RangeError);
});
test('registry fires handlers until reset', () => {
  let seen = 0;
  registry.register('tick', () => { seen += 1; });
  assert.strictEqual(registry.fire('tick'), 1);
  registry.reset();
  assert.strictEqual(registry.fire('tick'), 0);
  assert.strictEqual(seen, 1);
});
test('legacy mode and ticket counter', () => {
  assert.strictEqual(FILE_MODE, 493);
  assert.deepStrictEqual([nextTicket(), nextTicket(), nextTicket()], [1, 2, 3]);
});
test('format money and percent', () => {
  assert.strictEqual(money(1205), '$12.05');
  assert.strictEqual(percent(1, 3), '33%');
  assert.strictEqual(percent(1, 0), '0%');
});
test('ring modules call each other', () => assert.strictEqual(a(2), 'abcabcabc'));
test('plugins are listed', () => assert.deepStrictEqual(listPlugins(), ['reverse', 'title', 'upper']));
