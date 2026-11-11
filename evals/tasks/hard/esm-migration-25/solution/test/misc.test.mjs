import assert from 'node:assert';
import { test } from './harness.mjs';
import Cache from '../src/cache.mjs';
import slugify from '../src/slug.mjs';
import { getConfig } from '../src/config.mjs';
import { main } from '../bin/cli.mjs';

test('cache evicts the oldest entry', () => {
  const cache = new Cache(2);
  cache.set('a', 1).set('b', 2).set('c', 3);
  assert.strictEqual(cache.get('a'), undefined);
  assert.strictEqual(cache.size, 2);
});
test('slugify builds a slug', () => assert.strictEqual(slugify('  Hello, World! '), 'hello-world'));
test('getConfig merges overrides', () => assert.strictEqual(getConfig({ server: { port: 9 } }).server.host, 'localhost'));
test('cli sums numbers', () => assert.deepStrictEqual(main(['sum', '1', '2', '3']), { code: 0, out: '6' }));
