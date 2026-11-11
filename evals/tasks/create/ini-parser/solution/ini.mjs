// INI text parser.

/**
 * Removes one pair of surrounding double quotes from a value when present.
 * @param value - the trimmed value text
 */
function unquoteValue(value) {
  if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) return value.slice(1, -1);
  return value;
}

/**
 * Builds the error thrown for a line that cannot be parsed.
 * @param lineNumber - 1 based line number
 * @param line - the trimmed line text
 */
function invalidLine(lineNumber, line) {
  return new Error(`Invalid line ${lineNumber}: ${line}`);
}

/**
 * Returns the section name of a header line, or throws for a broken header.
 * @param line - trimmed line starting with [
 * @param lineNumber - 1 based line number
 */
function readSectionName(line, lineNumber) {
  if (!line.endsWith("]")) throw invalidLine(lineNumber, line);
  const name = line.slice(1, -1).trim();
  if (!name) throw invalidLine(lineNumber, line);
  return name;
}

/**
 * Splits a key and value line at the first = sign.
 * @param line - trimmed line
 * @param lineNumber - 1 based line number
 */
function readKeyValue(line, lineNumber) {
  const at = line.indexOf("=");
  if (at < 0) throw invalidLine(lineNumber, line);
  const key = line.slice(0, at).trim();
  if (!key) throw invalidLine(lineNumber, line);
  return [key, unquoteValue(line.slice(at + 1).trim())];
}

/**
 * Parses INI text into a plain object with nested objects for sections.
 * @param text - the INI text
 */
export function parse(text) {
  const result = {};
  let target = result;
  String(text).split(/\r?\n/).forEach((rawLine, index) => {
    const line = rawLine.trim();
    if (!line || line.startsWith(";") || line.startsWith("#")) return;
    if (line.startsWith("[")) {
      const name = readSectionName(line, index + 1);
      if (!Object.prototype.hasOwnProperty.call(result, name) || typeof result[name] !== "object") result[name] = {};
      target = result[name];
      return;
    }
    const [key, value] = readKeyValue(line, index + 1);
    target[key] = value;
  });
  return result;
}
