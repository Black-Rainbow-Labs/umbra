// Fixed settings of the suite: the model server, the model, and the default sampling.

import os from "node:os";
import path from "node:path";

export const UPSTREAM = { host: "127.0.0.1", port: 8080 };
export const DEFAULT_MODEL = process.env.EVAL_MODEL || "ninfer-qwen3.8-27b-coletti-v9";
export const DEFAULT_SAMPLING = { temperature: 0.6, top_p: 0.95, top_k: 20, enableThinking: true };
export const WORK_ROOT = process.env.EVAL_WORK_DIR || path.join(os.homedir(), "umbra-evals-work");
