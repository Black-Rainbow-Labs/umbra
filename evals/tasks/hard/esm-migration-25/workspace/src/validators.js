exports.isEmail = function isEmail(value) {
  return typeof value === 'string' && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
};

exports.isPositiveInt = function isPositiveInt(value) {
  return Number.isInteger(value) && value > 0;
};

exports.isNonEmptyString = function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
};
