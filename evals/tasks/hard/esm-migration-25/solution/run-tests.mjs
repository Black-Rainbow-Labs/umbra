import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { runAll } from './test/harness.mjs';

const dir = path.join(import.meta.dirname, 'test');
for (const file of fs.readdirSync(dir).filter((name) => name.endsWith('.test.mjs')).sort()) {
  await import(pathToFileURL(path.join(dir, file)).href);
}

process.exitCode = (await runAll()) ? 1 : 0;
