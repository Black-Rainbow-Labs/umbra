// File specifications for the utf8-bom-config task: original lines, expected lines, encoding, BOM and line endings.

/**
 * Returns the specs of every file in the workspace. `before` and `after` are line arrays; `after` is null for files that must not change.
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
      path: "config/other.ini", encoding: "utf8", bom: true, eol: "\r\n", finalEol: true,
      before: ["; Billing service, owned by another team", "[server]", "port = 8080", "currency = CHF"],
      after: null,
    },
    {
      path: "locales/en.yaml", encoding: "utf8", bom: true, eol: "\n", finalEol: true,
      before: ['greeting: "Hello"', 'farewell: "Goodbye"', 'port_label: "Port 8080"'],
      after: null,
    },
  ];
}

/**
 * Encodes lines into file bytes with the spec's encoding, BOM, line ending and final newline.
 * @param spec - a file spec
 * @param lines - the lines
 */
export function encodeFile(spec, lines) {
  const text = lines.join(spec.eol) + (spec.finalEol ? spec.eol : "");
  if (spec.encoding === "utf16le") return Buffer.concat([spec.bom ? Buffer.from([0xff, 0xfe]) : Buffer.alloc(0), Buffer.from(text, "utf16le")]);
  return Buffer.concat([spec.bom ? Buffer.from([0xef, 0xbb, 0xbf]) : Buffer.alloc(0), Buffer.from(text, "utf8")]);
}

/**
 * Decodes file bytes of a spec into text, or returns null when the bytes are not valid for the spec's encoding.
 * @param spec - a file spec
 * @param buffer - the file bytes
 */
export function decodeFile(spec, buffer) {
  const bom = spec.encoding === "utf16le" ? [0xff, 0xfe] : [0xef, 0xbb, 0xbf];
  const hasBom = bom.every((byte, index) => buffer[index] === byte);
  const body = hasBom ? buffer.subarray(bom.length) : buffer;
  if (spec.encoding === "utf16le") return body.length % 2 === 0 ? { hasBom, text: body.toString("utf16le") } : null;
  const text = body.toString("utf8");
  return text.includes("�") ? null : { hasBom, text };
}
