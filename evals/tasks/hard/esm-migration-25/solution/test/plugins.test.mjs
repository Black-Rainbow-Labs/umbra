import assert from 'node:assert';
import { test } from './harness.mjs';
import { loadPlugin } from '../src/pluginLoader.mjs';
import { mainAsync } from '../bin/cli.mjs';

test('loadPlugin returns the plugin function', async () => assert.strictEqual((await loadPlugin('title'))('a b'), 'A B'));
test('loadPlugin rejects an unknown plugin', async () => {
  await assert.rejects(loadPlugin('../paths'), { message: 'unknown plugin: ../paths' });
});
test('mainAsync applies a plugin', async () => assert.deepStrictEqual(await mainAsync(['apply', 'upper', 'a', 'b']), { code: 0, out: 'A B' }));
