// Usage: node tools/csv-table.mjs <input.csv> <output.html>. Renders a CSV file as an HTML table.
import fs from "node:fs";

/**
 * Escapes the characters that matter in HTML text.
 * @param text - raw text
 */
function escapeHtml(text) {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\"/g, "&quot;").replace(/'/g, "&#39;");
}

/**
 * Parses CSV text into records. A quote starts a quoted field only at the start of a field, "" inside quotes is a quote,
 * line breaks inside quotes are kept as \n, and records that are one empty field (blank lines) are skipped.
 * @param text - the whole file
 */
function parseCsv(text) {
  const records = [];
  let record = [];
  let field = "";
  let quoted = false;
  let fieldStart = true;
  const endRecord = () => {
    record.push(field);
    if (!(record.length === 1 && record[0] === "")) records.push(record);
    record = [];
    field = "";
    fieldStart = true;
  };
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (quoted) {
      if (char === '"') {
        if (text[i + 1] === '"') { field += '"'; i++; } else quoted = false;
      } else if (!(char === "\r" && text[i + 1] === "\n")) field += char;
    } else if (char === '"' && fieldStart) { quoted = true; fieldStart = false; }
    else if (char === ",") { record.push(field); field = ""; fieldStart = true; }
    else if (char === "\r" && text[i + 1] === "\n") continue;
    else if (char === "\n") endRecord();
    else { field += char; fieldStart = false; }
  }
  if (field !== "" || record.length) endRecord();
  return records;
}

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  console.error("usage: csv-table.mjs <input.csv> <output.html>");
  process.exit(2);
}
const rows = parseCsv(fs.readFileSync(input, "utf8"));
const html = ["<table>"];
rows.forEach((row, index) => {
  const tag = index === 0 ? "th" : "td";
  html.push("<tr>" + row.map((cell) => `<${tag}>${escapeHtml(cell)}</${tag}>`).join("") + "</tr>");
});
html.push("</table>");
fs.writeFileSync(output, html.join("\n") + "\n");
