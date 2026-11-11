const tests = [];

exports.test = function test(name, fn) {
  tests.push({ name, fn });
};

exports.runAll = async function runAll() {
  let passed = 0;
  let failed = 0;
  for (const { name, fn } of tests) {
    try {
      await fn();
      passed += 1;
      console.log('ok - ' + name);
    } catch (error) {
      failed += 1;
      console.log('not ok - ' + name + ': ' + error.message);
    }
  }
  console.log(`passed ${passed}, failed ${failed}`);
  return failed;
};
