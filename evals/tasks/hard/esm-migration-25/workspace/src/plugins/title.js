const { capitalize } = require('../strings');

module.exports = function title(text) {
  return text.split(' ').map(capitalize).join(' ');
};
