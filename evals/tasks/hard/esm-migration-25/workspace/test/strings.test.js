const assert = require('assert');
const { test } = require('./harness');
const { capitalize, padLeft, truncate } = require('../src/strings');

test('capitalize upper cases the first letter', () => assert.strictEqual(capitalize('abc'), 'Abc'));
test('padLeft pads to width', () => assert.strictEqual(padLeft(7, 3, '0'), '007'));
test('truncate shortens long text', () => assert.strictEqual(truncate('abcdefghij', 6), 'abc...'));
test('truncate keeps short text', () => assert.strictEqual(truncate('abc', 6), 'abc'));
