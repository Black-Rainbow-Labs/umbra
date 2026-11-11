// JSDoc typedefs shared by the eval suite. This module exports nothing at run time.

/**
 * One phase of a task. A task with no `phases` has one phase built from its `prompt`.
 * @typedef {object} TaskPhase
 * @property {string} prompt - the prompt sent to the agent in this phase
 * @property {"new"|"resume"|"fork"} [mode] - start a new session, resume one, or fork one (default "new" for the first phase, "resume" after)
 * @property {number} [from] - index of the phase whose session is resumed or forked (default: the previous phase)
 * @property {number} [killAfterSeconds] - the runner kills the agent this many seconds into the phase
 * @property {{dir: string, count: number}} [killWhenFiles] - the runner kills the agent once that workspace folder holds count files
 * @property {number} [contextWindow] - context window reported to the agent in this phase (overrides the task value)
 * @property {string} [label] - a short name for the phase, used in result.json
 */

/**
 * A task as loaded from tasks/<category>/<id>/task.json, plus resolved paths.
 * @typedef {object} EvalTask
 * @property {string} id - unique task id, equal to its folder name
 * @property {string} category - the category folder name
 * @property {string} prompt - the prompt for a single phase task
 * @property {number} timeoutSeconds - wall time limit per phase
 * @property {number} [contextWindow] - context window reported to the agent
 * @property {number} [stepLimit] - model request limit per phase (default 60)
 * @property {TaskPhase[]} [phases] - phases for session tasks
 * @property {string[]} [largeFixtures] - workspace relative paths deleted after verification
 * @property {boolean} [excludeFromDefault] - left out of a run unless named with --tasks
 * @property {string} [scenario] - session scenario name (resume, interrupt, fork, long-resume)
 * @property {string} dir - absolute path of the task folder
 */

/**
 * Settings the measuring proxy applies to every chat request.
 * @typedef {object} SamplingConfig
 * @property {number} temperature - sampling temperature
 * @property {number} top_p - nucleus sampling
 * @property {number} top_k - top k sampling
 * @property {boolean} enableThinking - value for chat_template_kwargs.enable_thinking
 */

/**
 * What the runner passes to an agent adapter for one phase.
 * @typedef {object} AdapterInput
 * @property {string} workspaceDir - the folder the agent works in
 * @property {string} prompt - the prompt for this phase
 * @property {string} proxyBaseUrl - base URL of the measuring proxy, without /v1
 * @property {string} model - model id sent to the server
 * @property {number} [contextWindow] - context window the agent should assume
 * @property {number} timeoutSeconds - wall time limit
 * @property {number} stepLimit - model request limit
 * @property {string} homeDir - isolated home folder for the agent
 * @property {string} phaseDir - folder for this phase's stdout and artifacts
 * @property {{mode: "resume"|"fork", sessionRef: string}|null} resume - session to resume or fork, or null for a new session
 */

/**
 * What an adapter returns for one phase.
 * @typedef {object} AdapterResult
 * @property {number|null} exitCode - process exit code, null when killed
 * @property {boolean} timedOut - true when the runner killed it at the wall limit
 * @property {boolean} killed - true when the runner killed it for any reason
 * @property {string} killReason - "", "timeout", "step-limit" or "planned-interrupt"
 * @property {number} wallMs - wall time in milliseconds
 * @property {string} stdoutPath - path of the captured stdout
 * @property {string|null} sessionRef - id or path that resumes this session
 */

/**
 * One line of requests.jsonl written by the proxy.
 * @typedef {object} RequestRecord
 * @property {string} kind - "chat" or "other"
 * @property {string} method - HTTP method
 * @property {string} path - request path
 * @property {number} phase - phase index at the time of the request
 * @property {string} startedAt - ISO time the request arrived
 * @property {number|null} firstByteMs - ms from arrival to the first response byte
 * @property {number|null} firstContentMs - ms from arrival to the first content, reasoning or tool call delta
 * @property {number} durationMs - ms from arrival to the end of the response
 * @property {number} status - HTTP status
 * @property {object|null} usage - prompt_tokens, cached_tokens, completion_tokens, reasoning_tokens
 * @property {object|null} timings - the server's timings object
 * @property {number} messageCount - number of messages in the request
 * @property {number} toolCount - number of tool definitions
 * @property {number} systemBytes - byte length of the system message
 * @property {number} toolsBytes - byte length of the tools array as JSON
 * @property {string|null} finishReason - finish reason of the first choice
 * @property {boolean} hasToolCall - true when the response contained a tool call
 * @property {boolean} windowExceeded - prompt_tokens above the reported context window
 * @property {number|null} contextWindow - the context window in force for the request
 * @property {string[]} droppedFields - sampling fields not sent because the server rejected them
 */

export {};
