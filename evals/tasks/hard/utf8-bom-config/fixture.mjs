// File specifications for the utf8-bom-config task: original lines, expected lines, encoding, BOM and line endings.

const CP1252_EXTRA = new Map([[0x20ac, 0x80], [0x2013, 0x96], [0x2014, 0x97], [0x2019, 0x92]]);

/**
 * Returns the specs of every file in the workspace. `before` and `after` are line arrays; `after` is null for files that must not change.
 * A spec has either `eol` and `finalEol`, or `eols` ({ before, after }: the ending of each line, "" for no ending on the last line).
 */
export function fileSpecs() {
  return [
    {
      path: "config/app.ini", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["; Cafe ordering service", "[server]", "host = 0.0.0.0", "port = 8080", "workers = 4", "", "[ui]", "title = Café Ordering", "motto = Fresh – every day", "", "[cache]", "enabled = true", "backend = memory"],
      after: ["; Cafe ordering service", "[server]", "host = 0.0.0.0", "port = 9090", "workers = 4", "", "[ui]", "title = Café Ordering Zürich", "motto = Fresh – every day", "", "[cache]", "enabled = true", "backend = memory", "ttl = 600"],
    },
    {
      path: "config/legacy.conf", encoding: "utf16le", bom: true, eol: "\r\n", finalEol: true,
      before: ["# Legacy gateway settings", "ServerName gateway.zuerich.example", "Listen 8080", "MaxClients 150", "Contact Müller & Söhne"],
      after: ["# Legacy gateway settings", "ServerName gateway.zuerich.example", "Listen 9090", "LogLevel warn", "MaxClients 300", "Contact Müller & Söhne AG"],
    },
    {
      path: "src/settings.js", encoding: "utf8", bom: false, eol: "\n", finalEol: true,
      before: ["// Runtime settings", "export const DEFAULT_PORT = 8080;", 'export const REGION = "Zürich";', 'export const CURRENCY = "CHF";', "export const WORKERS = 4;"],
      after: ["// Runtime settings", "export const DEFAULT_PORT = 9090;", 'export const REGION = "Zürich (Altstadt)";', 'export const CURRENCY = "CHF";', "export const CACHE_TTL_SECONDS = 600;", "export const WORKERS = 4;"],
    },
    {
      path: "src/server.js", encoding: "utf8", bom: false, eol: "\r\n", finalEol: true,
      before: ["// Démarre le serveur", 'import { DEFAULT_PORT, REGION } from "./settings.js";', "", "export function start() {", '  console.log("Listening on port " + DEFAULT_PORT + " in " + REGION);', "  return DEFAULT_PORT;", "}"],
      after: ["// Démarre le serveur", 'import { DEFAULT_PORT, REGION, CACHE_TTL_SECONDS } from "./settings.js";', "", "export function start() {", '  console.log("Listening on port " + DEFAULT_PORT + " in " + REGION + " (ttl " + CACHE_TTL_SECONDS + "s)");', "  return DEFAULT_PORT;", "}"],
    },
    {
      path: "deploy/deploy.ps1", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["# Déploiement du service", 'param([string]$Target = "prod")', "$Port = 8080", 'Write-Host "Déploiement terminé"'],
      after: ["# Déploiement du service", 'param([string]$Target = "prod")', "$Port = 9090", "$Ttl = 600", 'Write-Host "Déploiement terminé sur le port $Port"'],
    },
    {
      path: "data/regions.csv", encoding: "utf16le", bom: true, eol: "\n", finalEol: true,
      before: ["code,name,port", "ZH,Zürich,8080", "GE,Genève,8081", "BS,Basel,8082"],
      after: ["code,name,port", "ZH,Zürich,9090", "GE,Genève,8081", "BS,Basel,8082", "SP,São Paulo,8083"],
    },
    {
      path: "locales/de.yaml", encoding: "utf8", bom: true, eol: "\n", finalEol: false,
      before: ['greeting: "Grüß Gott"', 'farewell: "Auf Wiedersehen"', 'port_label: "Anschluss 8080"'],
      after: ['greeting: "Grüß Gott"', 'farewell: "Auf Wiedersehen"', 'port_label: "Anschluss 9090"', 'ttl_label: "Gültigkeit 600 s"'],
    },
    {
      path: "README.txt", encoding: "utf8", bom: false, eol: "\n", finalEol: true,
      before: ["Café Ordering service", "=====================", "", "Default port: 8080", "Health check: http://localhost:8080/health", "Admin console: http://localhost:8080/admin", "Support: Müller & Söhne"],
      after: ["Café Ordering service", "=====================", "", "Default port: 9090", "Health check: http://localhost:9090/health", "Admin console: http://localhost:9090/admin", "Support: Müller & Söhne"],
    },
    {
      path: "config/billing.toml", encoding: "cp1252", bom: false, eol: "\n", finalEol: true,
      before: ["# Facturation / Rechnung", "[service]", "port = 8080", 'currency = "CHF"', 'label = "Rechnung für Zürich"', 'fee = "2 €"'],
      after: ["# Facturation / Rechnung", "[service]", "port = 9090", "ttl = 600", 'currency = "CHF"', 'label = "Rechnung für Zürich – Altstadt"', 'fee = "2 €"'],
    },
    {
      path: "docs/notes.txt", encoding: "utf16be", bom: true, eol: "\r\n", finalEol: false,
      before: ["Release notes", "Service runs on port 8080 by default.", "Contact: Zoë Müller"],
      after: ["Release notes", "Service runs on port 9090 by default.", "Contact: Zoë Müller", "TTL: 600 seconds"],
    },
    {
      path: "scripts/start.sh", encoding: "utf8", bom: false, eol: "\n", finalEol: true,
      before: ["#!/bin/sh", "# Starte Dienst für Zürich", "PORT=8080", "export PORT", 'exec ./server --port "$PORT"'],
      after: ["#!/bin/sh", "# Starte Dienst für Zürich", "PORT=9090", "TTL=600", "export PORT TTL", 'exec ./server --port "$PORT"'],
    },
    {
      path: "web/index.html", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["<!doctype html>", '<html lang="de">', '<head><meta charset="utf-8"><title>Café Zürich</title></head>', "<body>", "<p>Bestellung über Port 8080</p>", '<a href="http://localhost:8080/health">Health</a>', "</body>", "</html>"],
      after: ["<!doctype html>", '<html lang="de">', '<head><meta charset="utf-8"><title>Café Zürich – Bestellung</title></head>', "<body>", "<p>Bestellung über Port 9090</p>", '<a href="http://localhost:9090/health">Health</a>', "</body>", "</html>"],
    },
    {
      path: "config/mac.cfg", encoding: "utf8", bom: false, eol: "\r", finalEol: true,
      before: ["# Réglages", "port=8080", "region=Zürich"],
      after: ["# Réglages", "port=9090", "ttl=600", "region=Zürich"],
    },
    {
      path: "data/prices.tsv", encoding: "utf8", bom: true, eol: "\n", finalEol: true,
      before: ["sku\tname\tprice\tport", "A1\tCafé crème\t4.50\t8080", "A2\tThé vert\t3.80\t8080"],
      after: ["sku\tname\tprice\tport", "A1\tCafé crème\t4.50\t9090", "A2\tThé vert\t3.80\t9090", "A3\tÉclair\t5.20\t9090"],
    },
    {
      path: "config/win.inf", encoding: "utf16le", bom: false, eol: "\r\n", finalEol: true,
      before: ["[Service]", "Port=8080", "Name=Gateway Zürich"],
      after: ["[Service]", "Port=9090", "Ttl=600", "Name=Gateway Zürich Nord"],
    },
    {
      path: "CHANGELOG.md", encoding: "utf8", bom: false,
      eols: { before: ["\r\n", "\r\n", "\r\n", "\n", "\n", "\n", "\n", "\n"], after: ["\r\n", "\r\n", "\r\n", "\r\n", "\n", "\n", "\n", "\n", "\n"] },
      before: ["# Changelog", "## 1.2 – Café", "- Port 8080 documented", "- Fixed Zürich bug", "", "## 1.1", "- Initial release", "- Première version"],
      after: ["# Changelog", "## 1.2 – Café", "- Port 9090 documented", "- Cache ttl of 600 seconds", "- Fixed Zürich bug", "", "## 1.1", "- Initial release", "- Première version"],
    },
    {
      path: "docs/todo.txt", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["TODO für Zürich", "- Open port 8080", ""],
      after: ["TODO für Zürich", "- Open port 9090", "- Set ttl 600", ""],
    },
    {
      path: "config/other.ini", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["; Billing service, owned by another team", "[server]", "port = 8080", "currency = CHF"],
      after: null,
    },
    {
      path: "locales/en.yaml", encoding: "utf8", bom: true, eol: "\n", finalEol: true,
      before: ['greeting: "Hello"', 'farewell: "Goodbye"', 'port_label: "Port 8080"'],
      after: null,
    },
    {
      path: "scripts/stop.sh", encoding: "utf8", bom: false, eol: "\n", finalEol: true,
      before: ["#!/bin/sh", "# Dienst für Zürich stoppen", "curl -s http://localhost:8080/stop"],
      after: null,
    },
    {
      path: "docs/archive/old-readme.txt", encoding: "utf16le", bom: true, eol: "\r\n", finalEol: true,
      before: ["Alte Anleitung", "Port 8080 wurde früher verwendet."],
      after: null,
    },
  ];
}

/**
 * Returns the byte order mark bytes of a spec's encoding, or an empty array for encodings that have none.
 * @param spec - a file spec
 */
function bomBytes(spec) {
  if (spec.encoding === "utf16le") return [0xff, 0xfe];
  if (spec.encoding === "utf16be") return [0xfe, 0xff];
  if (spec.encoding === "utf8") return [0xef, 0xbb, 0xbf];
  return [];
}

/**
 * Turns lines into the file text using the spec's line endings.
 * @param spec - a file spec
 * @param lines - the lines
 * @param which - "before" or "after", selects the endings of a spec that lists them per line
 */
function joinLines(spec, lines, which) {
  if (spec.eols) return lines.map((line, index) => line + spec.eols[which][index]).join("");
  return lines.join(spec.eol) + (spec.finalEol ? spec.eol : "");
}

/**
 * Encodes text as Windows-1252 bytes.
 * @param text - the text
 */
function encodeCp1252(text) {
  const bytes = [];
  for (const char of text) {
    const code = char.codePointAt(0);
    if (code < 0x80 || (code >= 0xa0 && code <= 0xff)) bytes.push(code);
    else if (CP1252_EXTRA.has(code)) bytes.push(CP1252_EXTRA.get(code));
    else throw new Error(`cannot encode ${char} as Windows-1252`);
  }
  return Buffer.from(bytes);
}

/**
 * Encodes lines into file bytes with the spec's encoding, BOM, line ending and final newline.
 * @param spec - a file spec
 * @param lines - the lines
 * @param which - "before" or "after" (only matters for specs with per line endings)
 */
export function encodeFile(spec, lines, which = "before") {
  const text = joinLines(spec, lines, which);
  const bom = spec.bom ? Buffer.from(bomBytes(spec)) : Buffer.alloc(0);
  if (spec.encoding === "cp1252") return encodeCp1252(text);
  if (spec.encoding === "utf16le") return Buffer.concat([bom, Buffer.from(text, "utf16le")]);
  if (spec.encoding === "utf16be") return Buffer.concat([bom, Buffer.from(text, "utf16le").swap16()]);
  return Buffer.concat([bom, Buffer.from(text, "utf8")]);
}

/**
 * Decodes file bytes of a spec into text, or returns null when the bytes are not valid for the spec's encoding.
 * @param spec - a file spec
 * @param buffer - the file bytes
 */
export function decodeFile(spec, buffer) {
  const bom = bomBytes(spec);
  const hasBom = bom.length > 0 && bom.every((byte, index) => buffer[index] === byte);
  const body = hasBom ? buffer.subarray(bom.length) : buffer;
  if (spec.encoding === "cp1252") return { hasBom: false, text: new TextDecoder("windows-1252").decode(body) };
  if (spec.encoding === "utf16le" || spec.encoding === "utf16be") {
    if (body.length % 2 !== 0) return null;
    return { hasBom, text: (spec.encoding === "utf16be" ? Buffer.from(body).swap16() : body).toString("utf16le") };
  }
  const text = body.toString("utf8");
  return text.includes("�") ? null : { hasBom, text };
}
