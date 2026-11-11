// Writes the 24 order files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildOrders } from "./fixture.mjs";

/**
 * Generates orders/order-01.txt to order-24.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const order of buildOrders(seed).orders) {
    const file = path.join(workspaceDir, order.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, order.text);
  }
}
