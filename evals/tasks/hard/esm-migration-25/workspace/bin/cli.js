const { sum } = require('../src/numbers');
const slugify = require('../src/slug');
const { VERSION } = require('../src/constants');

function main(argv) {
  const [command, ...args] = argv;
  if (command === 'sum') return { code: 0, out: String(sum(args.map(Number))) };
  if (command === 'slug') return { code: 0, out: slugify(args.join(' ')) };
  if (command === 'version') return { code: 0, out: VERSION };
  return { code: 2, out: `unknown command: ${command}` };
}

module.exports = { main };

if (require.main === module) {
  const result = main(process.argv.slice(2));
  (result.code === 0 ? process.stdout : process.stderr).write(result.out + '\n');
  process.exitCode = result.code;
}
