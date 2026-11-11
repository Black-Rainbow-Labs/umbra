exports.capitalize = function capitalize(text) {
  return text.charAt(0).toUpperCase() + text.slice(1);
};

exports.padLeft = function padLeft(text, width, fill = ' ') {
  let out = String(text);
  while (out.length < width) out = fill + out;
  return out;
};

exports.truncate = function truncate(text, max) {
  return text.length <= max ? text : text.slice(0, Math.max(0, max - 3)) + '...';
};
