// Builds the generated Python package for the model and serializer task, in its original or solved form.

import { mulberry32, randomInt, pick, shuffled } from "./gen-utils.mjs";

export const CHANGED_FILES = ["shopkit/models/customer.py", "shopkit/serializers/customer.py"];

const ENTITIES = ["account", "address", "agreement", "alert", "asset", "audit", "batch", "booking", "brand", "budget", "campaign", "carrier", "cart", "catalog", "category", "claim", "contact", "contract", "coupon", "delivery", "depot", "device", "discount", "document", "driver", "employee", "event", "feedback", "forecast", "gateway", "group", "inventory", "job", "label", "lead", "location", "manifest", "market", "member", "message", "note", "order", "pallet", "plan", "product", "region", "report", "review", "route", "schedule", "shipment", "store", "supplier", "task", "ticket", "vehicle", "warehouse"];
const FIELD_POOL = [
  { name: "name", type: "str", sample: (entity) => `"${entity} one"` },
  { name: "code", type: "str", sample: (entity) => `"${entity.slice(0, 3).toUpperCase()}-100"` },
  { name: "status", type: "str", sample: () => '"open"' },
  { name: "quantity", type: "int", sample: () => "12" },
  { name: "created_year", type: "int", sample: () => "2021" },
  { name: "amount", type: "float", sample: () => "19.5" },
  { name: "active", type: "bool", sample: () => "True" },
  { name: "notes", type: "str", sample: () => '"none"' },
];

/**
 * Describes the three customer entities and ordinary entities by their field lists.
 * @param rng - seeded generator
 */
function entityDefinitions(rng) {
  const definitions = [
    { entity: "customer", className: "Customer", fields: [{ name: "name", type: "str", sample: () => '"Ada Lovelace"' }, { name: "email", type: "str", sample: () => '"ada@example.com"' }, { name: "created_year", type: "int", sample: () => "2019" }] },
    { entity: "customer_profile", className: "CustomerProfile", fields: [{ name: "customer_id", type: "int", sample: () => "1" }, { name: "bio", type: "str", sample: () => '"likes tea"' }, { name: "language", type: "str", sample: () => '"en"' }] },
    { entity: "customer_summary", className: "CustomerSummary", fields: [{ name: "customer_id", type: "int", sample: () => "1" }, { name: "order_count", type: "int", sample: () => "4" }, { name: "total_spent", type: "float", sample: () => "310.25" }] },
  ];
  for (const entity of ENTITIES) {
    const fields = shuffled(rng, FIELD_POOL).slice(0, randomInt(rng, 3, 5)).map((field) => ({ ...field, sample: () => field.sample(entity) }));
    definitions.push({ entity, className: entity[0].toUpperCase() + entity.slice(1), fields });
  }
  return definitions;
}

/**
 * Builds the model, serializer, repository and service files for one entity.
 * @param definition - { entity, className, fields }
 * @param solved - when true the customer entity gets loyalty_tier
 */
function buildEntityFiles(definition, solved) {
  const { entity, className, fields } = definition;
  const withTier = solved && entity === "customer";
  const modelLines = ["from dataclasses import dataclass", "", "", "@dataclass", `class ${className}:`, "    id: int", ...fields.map((field) => `    ${field.name}: ${field.type}`)];
  if (withTier) modelLines.push('    loyalty_tier: str = "standard"');
  const serializerLines = [`from shopkit.models.${entity} import ${className}`, "", "", `def serialize_${entity}(${entity}):`, `    """Returns the ${entity} as a dict for the API."""`, "    return {", `        "id": ${entity}.id,`, ...fields.map((field) => `        "${field.name}": ${entity}.${field.name},`)];
  if (withTier) serializerLines.push(`        "loyalty_tier": ${entity}.loyalty_tier,`);
  serializerLines.push("    }");
  const sampleArgs = ["id=1", ...fields.map((field) => `${field.name}=${field.sample()}`)].join(", ");
  const files = new Map();
  files.set(`shopkit/models/${entity}.py`, modelLines.join("\n") + "\n");
  files.set(`shopkit/serializers/${entity}.py`, serializerLines.join("\n") + "\n");
  files.set(`shopkit/repositories/${entity}.py`, `from shopkit.models.${entity} import ${className}\n\n\ndef sample_${entity}():\n    """Returns a sample ${entity} used by the demo data."""\n    return ${className}(${sampleArgs})\n`);
  files.set(`shopkit/services/${entity}_service.py`, `from shopkit.repositories.${entity} import sample_${entity}\nfrom shopkit.serializers.${entity} import serialize_${entity}\n\n\ndef export_${entity}():\n    """Returns the serialized sample ${entity}."""\n    return serialize_${entity}(sample_${entity}())\n`);
  return files;
}

/**
 * Builds the whole Python package in memory.
 * @param seed - the fixed seed
 * @param options - { solved } when true Customer has loyalty_tier and its serializer includes it
 */
export function buildRepo(seed, options = {}) {
  const rng = mulberry32(seed);
  const files = new Map();
  for (const definition of entityDefinitions(rng)) {
    for (const [file, text] of buildEntityFiles(definition, Boolean(options.solved))) files.set(file, text);
  }
  for (const folder of ["", "models", "serializers", "repositories", "services"]) files.set(`shopkit/${folder ? folder + "/" : ""}__init__.py`, "");
  files.set("README.md", "# shopkit\n\nSample package with models, serializers, repositories and services.\n");
  return { files };
}
