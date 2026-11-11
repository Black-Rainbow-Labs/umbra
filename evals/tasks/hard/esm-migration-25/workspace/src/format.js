const { padLeft } = require('./strings');

module.exports = {
  money(cents) {
    return '$' + Math.floor(cents / 100) + '.' + padLeft(cents % 100, 2, '0');
  },
  percent(part, whole) {
    return whole === 0 ? '0%' : Math.round((part / whole) * 100) + '%';
  },
};
