import { pathToFileURL } from 'node:url';
import { sum } from '../src/numbers.mjs';
import slugify from '../src/slug.mjs';
import { VERSION } from '../src/constants.mjs';

export function main(argv) {
  const [command, ...args] = argv;
  if (command === 'sum') return { code: 0, out: String(sum(args.map(Number))) };
  if (command === 'slug') return { code: 0, out: slugify(args.join(' ')) };
  if (command === 'version') return { code: 0, out: VERSION };
  return { code: 2, out: `unknown command: ${command}` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const result = main(process.argv.slice(2));
  (result.code === 0 ? process.stdout : process.stderr).write(result.out + '\n');
  process.exitCode = result.code;
}
