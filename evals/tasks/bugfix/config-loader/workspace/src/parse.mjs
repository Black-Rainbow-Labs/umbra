// Parses ini style text into nested objects of strings.

/**
 * Parses text with "key = value" lines and optional [section] headers. Lines starting with # are comments.
 * @param text - the config file contents
 */
export function parseIni(text) {
  const root = {};
  let current = root;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;
    const section = /^\[([^\]]+)\]$/.exec(line);
    if (section) {
      current = root[section[1].trim()] ??= {};
      continue;
    }
    assignLine(current, line);
  }
  return root;
}

/**
 * Stores one "key = value" line on the target object.
 * @param target - the root or section object receiving the value
 * @param line - a trimmed line that is not blank, a comment or a section header
 */
function assignLine(target, line) {
  const equals = line.indexOf("=");
  if (equals === -1) throw new Error(`invalid config line: ${line}`);
  target[line.slice(0, equals).trim()] = line.slice(equals + 1).trim();
}
