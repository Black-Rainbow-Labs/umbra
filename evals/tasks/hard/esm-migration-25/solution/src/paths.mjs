import path from 'node:path';

export function srcDir() {
  return import.meta.dirname;
}

export function dataDir() {
  return path.join(import.meta.dirname, '..', 'data');
}

export function dataPath(name) {
  return path.join(dataDir(), name);
}

export function thisFile() {
  return path.basename(import.meta.filename);
}
