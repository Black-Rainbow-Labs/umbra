// Usage: node tools/render-md.mjs <input.md> <output.html>. Renders a small subset of Markdown.
import fs from "node:fs";

/**
 * Escapes the characters that matter in HTML text.
 * @param text - raw text
 */
function escapeHtml(text) {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\"/g, "&quot;").replace(/'/g, "&#39;");
}

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  console.error("usage: render-md.mjs <input.md> <output.html>");
  process.exit(2);
}
const lines = fs.readFileSync(input, "utf8").split(/\r?\n/);
const html = ["<article>"];
let inList = false;
for (const line of lines) {
  if (inList && !line.startsWith("- ")) { html.push("</ul>"); inList = false; }
  if (line.startsWith("## ")) html.push(`<h2>${escapeHtml(line.slice(3))}</h2>`);
  else if (line.startsWith("# ")) html.push(`<h1>${escapeHtml(line.slice(2))}</h1>`);
  else if (line.startsWith("- ")) { if (!inList) { html.push("<ul>"); inList = true; } html.push(`<li>${escapeHtml(line.slice(2))}</li>`); }
  else if (line.trim() !== "") html.push(`<p>${escapeHtml(line)}</p>`);
}
if (inList) html.push("</ul>");
html.push("</article>");
fs.writeFileSync(output, html.join("\n") + "\n");
