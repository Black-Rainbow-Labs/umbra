let counter = 0;

module.exports = function nextId(prefix = 'id') {
  counter += 1;
  return prefix + '-' + counter;
};
