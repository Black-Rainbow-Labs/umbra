import fs from 'node:fs';
import path from 'node:path';

const PLUGIN_DIR = path.join(import.meta.dirname, 'plugins');

export function listPlugins() {
  return fs.readdirSync(PLUGIN_DIR).filter((file) => file.endsWith('.mjs')).map((file) => file.slice(0, -4)).sort();
}

export async function loadPlugin(name) {
  if (!listPlugins().includes(name)) throw new Error('unknown plugin: ' + name);
  const loaded = await import('./plugins/' + name + '.mjs');
  return loaded.default;
}
