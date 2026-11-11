const fs = require('fs');
const path = require('path');
const { runAll } = require('./test/harness');

const dir = path.join(__dirname, 'test');
for (const file of fs.readdirSync(dir).filter((name) => name.endsWith('.test.js')).sort()) {
  require(path.join(dir, file));
}

runAll().then((failed) => {
  process.exitCode = failed ? 1 : 0;
});
