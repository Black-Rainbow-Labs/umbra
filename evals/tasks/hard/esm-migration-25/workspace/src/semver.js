function parse(text) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(text);
  if (!match) throw new Error('bad version: ' + text);
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

function compare(a, b) {
  const left = parse(a);
  const right = parse(b);
  for (const part of ['major', 'minor', 'patch']) {
    if (left[part] !== right[part]) return left[part] < right[part] ? -1 : 1;
  }
  return 0;
}

module.exports = compare;
module.exports.parse = parse;
module.exports.satisfies = function satisfies(version, minimum) {
  return compare(version, minimum) >= 0;
};
