# Umbra code survey (task-2614)

Surveyed: Umbra 0.5.2, branch `task-2614`, worktree `C:/jason/dev/umbra/.claude/worktrees/task-2614`, HEAD `e563f7d`. All paths below are relative to that worktree. Nothing was built or tested. I ran the installed `umbra` 0.5.2 binary (`C:/Users/jason/.cargo/bin/umbra`) only for `umbra prompt`, `umbra skills` and `umbra instructions`, which make no model call. I also sent two tiny chat requests (max_tokens 8) to the local server on port 8080 to see what usage fields it returns.

Labels used in section 11: "observed" means I read the code that does it (location given). "Speculation" means it is a reasoned prediction that needs a measurement.

## 0. Facts that matter most

1. Umbra has exactly one built in tool: `bash`. There is no read, write or edit tool. Files are read with `cat`/`sed -n`, and edited with heredocs, `sed` or scripts, as the system prompt tells the model (`src/templates/system.jinja:26`). `src/services/agent_service.rs:941-950` is the whole tool list (plus MCP tools and `tool_search`).
2. The local model server on this machine at `localhost:8080` is NInfer, not llama.cpp. `GET /props` returns `{"n_ctx":262144,"total_slots":1,"model_path":"ninfer-qwen3.8-27b-coletti-v9","engine":"ninfer"}`. Its chat completion replies include `usage.prompt_tokens_details.cached_tokens`, `usage.completion_tokens_details.reasoning_tokens` and a `timings` object (`cache_n`, `prompt_n`, `prompt_ms`, `predicted_n`, `predicted_per_second`, draft counters). A second identical request showed `cached_tokens: 23` of 30 prompt tokens, so prefix caching works on this server.
3. A headless mode exists without a new flag: `--yolo` with stdin not a terminal (section 9). There is no JSON event stream except `umbra acp`. The structured record of a run is the trajectory JSON file.
4. Defaults that will distort an eval on this machine: `~/.claude/CLAUDE.md` (40,666 bytes) and about 130 skills are loaded into the system prompt (about 17,300 tokens total instead of about 540), every MCP server found in other tools' config files is started, the bash timeout is 30 seconds, there is no step limit, no wall time limit and the cost limit is meaningless for a free local model. Isolate with `UMBRA_HOME_OVERRIDE`, `--no-mcp`, `--no-skills`, `--no-instructions`, `--step-limit` and a `-c` config file (section 9).
5. Compaction thresholds scale with the window. With the 262,144 token window reported by NInfer, masking starts at 209,715 tokens (80%). A run that stays below that never compacts (section 5).
6. The streaming path keeps `usage` but drops the server's `timings` object (`src/services/providers/openai.rs:337-339` and `371-388`). Per request latency and prompt processing speed are therefore not in the trajectory unless `stream = false` is set for the provider (the whole reply JSON, `timings` included, is then stored in `extra.response`).

## 1. Module map

Total: about 13,850 lines of Rust in `src/`, plus 11 Jinja templates (453 lines) in `src/templates/`, and 3,300 lines of tests. Line counts from `wc -l`.

### Entry
| Lines | File | Role |
|---|---|---|
| 38 | `src/main.rs` | Parses the CLI and dispatches to a service; exit code from the service, 2 on error |
| 232 | `src/cli.rs` | clap definitions: `RunArgs` (79-147), `CommonArgs` (151-187), subcommands (19-41) |

### Models (plain data)
| Lines | File | Role |
|---|---|---|
| 53 | `src/models/action.rs` | `Action` (Bash or Mcp) and `Observation {output, returncode, exception_info, images}` |
| 48 | `src/models/compaction.rs` | `CompactionRecord` kept in `info.compactions` |
| 392 | `src/models/config.rs` | `UmbraConfig` and every sub config with defaults |
| 89 | `src/models/context.rs` | Instruction file, skill, slash command, MCP definition types |
| 39 | `src/models/image.rs` | `ImageData` (base64, size, source) and its token estimate |
| 95 | `src/models/message.rs` | `Message` with roles system, user, assistant, tool, exit; `extra` is trajectory only |
| 11 | `src/models/mod.rs` | Module list |
| 27 | `src/models/model_response.rs` | `ModelResponse` and `Usage {input, output, cache_read, cache_write}` |
| 56 | `src/models/session.rs` | `SessionInfo`, `ResumeRecord`, `SessionIndexEntry` |
| 45 | `src/models/trajectory.rs` | `Trajectory`, `TrajectoryInfo`, `ModelStats` |

### Repositories (disk access)
| Lines | File | Role |
|---|---|---|
| 119 | `src/repositories/command_repository.rs` | Finds slash commands (`.claude/commands`, opencode, others) |
| 189 | `src/repositories/config_repository.rs` | Loads and merges config files; mini-swe-agent YAML conversion; built in providers (133-151) |
| 317 | `src/repositories/instruction_repository.rs` | Finds CLAUDE.md, AGENTS.md, GEMINI.md, Cursor, Copilot, Windsurf, Cline rules; `@path` imports; 256 KB budget |
| 350 | `src/repositories/mcp_config_repository.rs` | Finds MCP servers in every harness's config files |
| 31 | `src/repositories/mod.rs` | `DiscoveryContext` |
| 103 | `src/repositories/session_repository.rs` | Session index files in `~/.umbra/sessions`, listing, id lookup |
| 170 | `src/repositories/skill_repository.rs` | Finds SKILL.md files in every harness's folders and installed Claude Code plugins |
| 33 | `src/repositories/trajectory_repository.rs` | Writes the trajectory (whole file, temp file then rename) |

### Services
| Lines | File | Role |
|---|---|---|
| 176 | `src/services/action_parser.rs` | Turns a reply into actions, a plain reply, or a format error message |
| 55 | `src/services/agent_events.rs` | `AgentEvents` trait (front end hooks); `NoEvents` for the terminal |
| 1055 | `src/services/agent_service.rs` | The agent loop, limits, compaction triggers, observations, saving |
| 464 | `src/services/compaction_service.rs` | Budget math, token estimate, mask pass, cut, summary request |
| 83 | `src/services/context_service.rs` | Loads instructions, skills, commands, MCP definitions; expands `/command` tasks |
| 91 | `src/services/environment/docker.rs` | Docker environment (`docker exec`) |
| 186 | `src/services/environment/local.rs` | Local bash environment: spawn, timeout, kill, output capture |
| 76 | `src/services/environment/mod.rs` | `Environment` trait; the submit sentinel check |
| 215 | `src/services/image_service.rs` | Loads, scales and validates images; detects image paths in text |
| 174 | `src/services/inspect_service.rs` | `skills`, `instructions`, `commands`, `mcp`, `config`, `sessions` subcommands |
| 530 | `src/services/interaction_service.rs` | Confirm prompt, human mode, follow up prompt, output formatting |
| 215 / 262 / 129 / 202 / 135 | `src/services/mcp/{client,http,mod,stdio,transport}.rs` | MCP client over stdio and HTTP transports |
| 145 | `src/services/prompt_service.rs` | minijinja rendering; template selection (`Templates::resolve`, 50-63) |
| 295 | `src/services/providers/anthropic.rs` | Anthropic Messages API (not used for the local model) |
| 141 | `src/services/providers/credentials.rs` | API key and Claude Code login lookup |
| 197 | `src/services/providers/mod.rs` | `Provider` trait, `ProviderError`, retries (163-180), overflow detection (42-87) |
| 463 | `src/services/providers/openai.rs` | OpenAI Chat Completions, streaming assembler, context window lookup |
| 70 | `src/services/providers/pricing.rs` | Price table; unknown models (local) cost 0 |
| 108 | `src/services/providers/replay.rs` | Scripted provider for tests (`-m replay/script.json`) |
| 488 | `src/services/run_service.rs` | Builds a run from the CLI: config, context, MCP, environment, provider, session |
| 185 | `src/services/session_service.rs` | `--resume`, `--continue`, history repair |
| 147 | `src/services/setup_service.rs` | `umbra setup` wizard |
| 158 | `src/services/tool_search_service.rs` | Defers MCP tool definitions behind a `tool_search` tool |
| 316 / 270 / 280 / 454 / 207 | `src/services/acp/{content,events,mod,session,wire}.rs` | `umbra acp` JSON-RPC server for editors |
| 19 | `src/services/mod.rs` | Module list |

### Utils
| Lines | File | Role |
|---|---|---|
| 635 | `src/utils/banner.rs` | Startup banner and logo animation |
| 530 | `src/utils/console.rs` | Terminal output, line reading, stdin reader thread, verbose and quiet flags |
| 35 | `src/utils/env_expand.rs` | `${VAR}` expansion in MCP config |
| 65 | `src/utils/frontmatter.rs` | YAML frontmatter parser |
| 196 | `src/utils/http.rs` | Blocking HTTP client, streaming, interruptible POST |
| 154 | `src/utils/interrupt.rs` | Ctrl+C flag and per thread cancel flag |
| 300 | `src/utils/keys.rs` | Raw terminal key decoding |
| 93 | `src/utils/paths.rs` | Home folder (`UMBRA_HOME_OVERRIDE`), project root, tilde expansion |
| 95 | `src/utils/process.rs` | Child process groups and Windows job objects |
| 661 | `src/utils/screen.rs` | Fullscreen alternate screen view |
| 421 | `src/utils/status_bar.rs` | Status bar under the output |
| 173 | `src/utils/terminal_mode.rs` | Raw input mode |
| 52 | `src/utils/tokens.rs` | `chars/4` token estimate, window table |
| 15 | `src/utils/mod.rs` | Module list |

### Templates (`src/templates/`)
`system.jinja` 134 lines (6,306 bytes), `instance_tools.jinja` 99, `instance_text.jinja` 88, `instance_reply.jinja` 0 (contains only `{{task}}`), `observation_tools.jinja` 16, `observation_text.jinja` 27, `format_error_reply.jinja` 11, `format_error_tools.jinja` 20, `format_error_text.jinja` 27, `compact_summary.jinja` 22, `compact_resume.jinja` 9.

### Tests (`tests/`)
`acp.rs` 443, `agent_loop.rs` 183, `common/mod.rs` 233, `compaction.rs` 175, `console_windows.rs` 182, `discovery.rs` 106, `images.rs` 130, `interrupt.rs` 249, `live_local.rs` 156, `mcp.rs` 225, `sessions.rs` 261, `status_bar.rs` 98, `terminal_output.rs` 100. Unit tests are inline in `src/` files (`#[cfg(test)]`).

### Docs and design documents
`docs/architecture.md`, `configuration.md`, `providers.md`, `acp.md`, `mcp.md`, `discovery.md`, `releasing.md`. Six TDDs in `tasks/`: harness (task-2559), compaction (task-2562), images/resume/interrupt, ACP and distribution (task-2583), conversational turns and fullscreen (task-2588), improvements v2 terminal UI (task-2607, a design only, not built).

## 2. The agent loop

Design origin: a port of mini-swe-agent's `DefaultAgent` (`src/services/agent_service.rs:1-5`). One linear message list. One model call per step. Actions run in order. Observations are appended.

### Structure
- `run_task` (`src/services/run_service.rs:134-185`) builds the agent and calls `Agent::run` (`agent_service.rs:252-278`).
- `run` calls `run_turn` (321-333) in a loop. After a successful turn it asks for a follow up task only when stdin is a terminal and `confirm_exit` is true (`interaction_service.rs:134-137`). Headless, it ends after one turn.
- `run_turn` calls `step()` (399-452) until a step returns `Exit`, saving the trajectory after every step (328).

### What one step does (`step`, 399-452)
1. Take a pending Ctrl+C. 2. Add lines the person typed while it worked (457-470, terminal only). 3. `check_limits` (512-531). 4. `compact_if_needed` (575-598). 5. `stats.api_calls += 1`. 6. `call_model` (535-546). 7. `record_usage` (748-752). 8. `parse_actions` (757-764). 9. Push the assistant message. 10. Confirm (skipped in yolo). 11. `execute_actions` (811-835): each action runs in order, one observation message per tool call is appended with `push_observations` (858-877).

### Where a turn is built and sent
`Provider::complete(&self.messages, &self.tools)` at `agent_service.rs:551`. The whole message list is sent every call, plus the tool list. For the local server `OpenAiProvider::request_body` (`openai.rs:34-51`) builds: `model`, `messages`, `stream: true`, `stream_options: {include_usage: true}`, `tools` (OpenAI function format), optional `max_tokens`, optional `temperature`, then merges `extra_body`.

### Tool call parsing
- Native OpenAI tool calls are the default (`action_format = "tools"`, `config.rs:99`). Tool call deltas are assembled from the stream in `StreamAssembler` (`openai.rs:293-389`).
- Argument text that is not valid JSON is kept as a string (`openai.rs:274-279`) and the parser rejects it with: `Error parsing tool call arguments: "<raw>" is not a JSON object.` (`action_parser.rs:80-83`).
- `bash` needs a string `command`, otherwise `Missing 'command' argument in bash tool call.` (84-88). An unknown tool name gives `Unknown tool '<name>'.` (92). One bad call rejects the whole reply (95).
- Text mode is a different mode, chosen with `--text` or `action_format = "text"`. It is not an automatic fallback. It parses exactly one fenced block tagged `umbra_bash` (regex at `prompt_service.rs:25`) or one `umbra_mcp` JSON block (`action_parser.rs:102-129`) and sends no tools.
- A reply with no tool call: in the default `finish = "reply"` style it is accepted as the final answer and ends the turn with status `Replied`, unless it is empty (`Your reply was empty...`) or `finish_reason == "length"` (`Your reply was cut off by the output token limit...`). See `reply_without_action`, `action_parser.rs:27-38`. In `finish = "submit"` style a reply with no tool call is a format error.

### Stop conditions (exit statuses)
| Status | Cause | Location |
|---|---|---|
| `Replied` | Reply with no tool call (default style); exit code 0 | `agent_service.rs:769-775` |
| `Submitted` | A command whose output starts with the line `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT` and exits 0 (works in both styles) | `environment/mod.rs:46-57` |
| `LimitsExceeded` | `step_limit` or `cost_limit` reached | `agent_service.rs:518-529` |
| `TimeExceeded` | `wall_time_limit_seconds` reached | 515-517 |
| `RepeatedFormatError` | 3 unusable replies in a row (`max_consecutive_format_errors`) | 786-789 |
| `ModelError` | Model call failed after retries and any overflow compaction | 326 |
| `Interrupted` | Ctrl+C with no terminal to ask | 478-480 |
| `UserQuit` | Human mode quit | 888 |

### Max turns
`step_limit` default 0 (no limit), `cost_limit` default 3.0 dollars, `wall_time_limit_seconds` default 0 (`config.rs:101-103`). `--step-limit` and `--cost-limit` are CLI flags. `wall_time_limit_seconds` can only be set in a config file (it is not in `cli.rs`). The cost limit never triggers for a local model because the price is 0 (`pricing.rs:54`).

### Retry and error handling
- HTTP 408, 409, 429, 5xx and network errors are retried: up to 9 retries (10 attempts) with waits of 4, 8, 16, 32, then 60 seconds (`providers/mod.rs:49-53, 163-186`). `UMBRA_RETRY_BASE_MS` changes the base wait. Other 4xx errors fail at once.
- A stream that breaks mid reply is a network error and is retried (`openai.rs:112`). A server error object inside a 200 response is also retried unless it is a context overflow (74-79).
- Context overflow (HTTP 400 or 413 with phrases such as `exceeds the available context size`) triggers one summary compaction and one more attempt (`agent_service.rs:535-546`).
- A model call that still fails ends the run with `ModelError` (326).

### Malformed tool calls
The bad reply is not kept in the history. The model receives only a user message built from `format_error_reply.jinja` that contains the error text (`agent_service.rs:781-791`, TDD `umbra-conversational-turns-and-fullscreen-tdd.md` lines 36-42 explains this is copied from mini-swe-agent). The counter resets on any good step (428) and on a plain reply (770).

### Loop and stall detection
None found. A search for repeated command or stall logic across `src/` returned nothing. The only protections are the three consecutive format errors, `step_limit`, `wall_time_limit_seconds` and the per command timeout. A model that repeats the same valid bash command forever runs until a limit (observed in code, absence).

### Streaming
OpenAI compatible servers are streamed by default (`openai.rs:54-56`). The stream is assembled completely before anything is parsed or shown. Its purpose is cancellation (dropping the stream stops the server), not incremental display (`docs/providers.md:105-118`). `reasoning_content` or `reasoning` deltas are collected into `raw` (`openai.rs:346`) but are not sent back on later turns (`convert_message`, 227-241 only sends `content` and `tool_calls`).

### Other loop behavior
- Several tool calls in one reply run in order. A submit sentinel skips the rest (825).
- `Ctrl+C` handling is thorough (docs/architecture.md:203-226).
- The trajectory and the session index are rewritten completely after every step (`agent_service.rs:921-936`).

## 3. System prompt

### Template selection
`Templates::resolve` (`prompt_service.rs:50-63`). Default config is `finish = "reply"` and `action_format = "tools"`. The system prompt is rendered once in `open_new` (`agent_service.rs:284-288`) and stored as `messages[0]`. The instance (first user) message for the reply style is just the task text (`src/templates/instance_reply.jinja` contains `{{task}}`). The submit style (`--finish submit`, or any mini-swe-agent YAML config) uses `system.jinja` lines 63-93 ("You are a helpful assistant that can interact with a computer.") plus `instance_tools.jinja` (3,591 bytes, about 900 tokens, a 6 step issue solving workflow).

### Exact default system prompt (reply style, tools mode, Windows, no instructions, no skills, no MCP)
This is the output of `umbra prompt --no-instructions --no-skills "hello"` run on this machine. The only per machine parts are the platform line, the working directory and the shell.

```
You are Umbra, a coding agent. You work in the user's project through their terminal or editor: you read and change files and run commands with the tools you are given.

# How to work

Match the work to the message. Decide what the user wants before you run anything.
- A greeting, thanks or small talk: reply briefly in plain text. Do not run commands.
- A question you can answer from what you already know, or from the conversation so far: answer it directly.
- A question about this project's code: read the code it is about first, then answer. Read what the question needs, not the whole repository.
- A request to change something: do all of it. Read the relevant code, make the change, run the build or tests that check it, fix what fails, and then report.
- An unclear request: ask one short question instead of guessing.

Do only what was asked. Do not start work nobody asked for, and do not commit, push or delete files unless the user asks you to. Follow the conventions of the code you change.

# Ending your turn

A reply that runs no command ends your turn. The user reads it and then sends their next message, and the conversation continues from there. So:
- While you are working, every reply runs at least one command. Do not stop to describe a plan you could carry out.
- When you are finished, or need the user, reply in plain text without running a command. Say what you did and what you found, briefly. Do not repeat output the user has already seen.

# Commands

- Every command runs in a new shell. The working directory and environment variables do not carry over, so use absolute paths or prefix a command with `cd <dir> &&`.
- Commands that wait for input, open an editor or a pager, or never exit will time out. Pass the flags that make a command run without asking.
- Read long files a part at a time, for example `sed -n '1,120p' file`. Search with `grep -rn` or `rg`.
- Create a file with a heredoc (`cat <<'EOF' > file`) and edit one with a short script or `sed`.

<system_information>
Windows   x86_64
Working directory: C:\jason\dev\umbra\.claude\worktrees\task-2614
Commands run with: C:\Program Files\Git\bin\bash.exe -c
</system_information>
```

On macOS one more bullet is added after the `sed` bullet: `- This is macOS: use `sed -i ''` instead of `sed -i`.` (`system.jinja:27-29`). The source is `src/templates/system.jinja:1-61`.

Size: about 2,150 characters, about 540 tokens at chars divided by 4. Nothing in it names `bash` as a tool, describes file editing conventions beyond `sed`/heredocs, mentions verification beyond "run the build or tests that check it", or gives guidance on git, large outputs or when to stop searching.

### How context files are added
After the base text, `system.jinja:95-134` appends, in this order, each only when non empty:
1. `<instructions>`: every instruction file, in `<instruction_file path= source= scope=>` tags, most general first (lines 95-105). Sources and order: `~/.umbra/UMBRA.md`, `~/.claude/CLAUDE.md`, `~/.claude/rules`, Codex, Gemini, opencode, `~/.agents/AGENTS.md`, Windsurf and Cline user files; then project root rule folders; then for every folder from the project root down to the working folder: `AGENTS.md` (or `AGENTS.override.md`), `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md`, `GEMINI.md`, `UMBRA.md` (`instruction_repository.rs:67-140`). `@path` imports inside Claude and Gemini files are followed 4 levels deep (19-20, 221-243). Files are de duplicated by path and content and capped at 256 KB total (`config.rs:242`).
2. `<skills>`: a list of `- name: description` and `path:` for every skill, with the instruction to `cat` the SKILL.md when a task matches (lines 106-115). Descriptions are cut to 1,024 characters (`context_service.rs:18`).
3. `<mcp_server_instructions>` from MCP servers (116-125).
4. `<deferred_tools>`: names of MCP tools whose definitions are not loaded (126-134).
Skill bodies are not in the prompt. A SKILL.md is read with a bash `cat`. A task that starts with `/name` is replaced by the command or skill body (`context_service.rs:52-64`).

### Measured on this machine
`umbra prompt --cwd <this worktree> -m local/qwen hello` printed 69,380 characters. With `--no-skills` it printed 43,243. With `--no-instructions --no-skills` it printed 2,252. So on this machine: user CLAUDE.md about 40.7 KB (about 10,200 tokens), the skills list about 26.1 KB (about 6,500 tokens, 131 lines from `umbra skills`), base prompt about 540 tokens. Total about 17,300 tokens before the first user message. `umbra instructions --cwd <worktree>` found one instruction file, `C:\Users\jason\.claude\CLAUDE.md`. The task-2588 TDD records the same pattern for the main checkout (project CLAUDE.md 35 KB and AGENTS.md 9.3 KB added about 21,000 tokens, `tasks/umbra-conversational-turns-and-fullscreen-tdd.md:223-236`).

### Is the prefix stable between turns?
Yes, for the system message and the tool list, with two exceptions.
- The system prompt is rendered once and stored (`agent_service.rs:284-288`). It does not contain the time, a counter, cost or a directory listing. `n_model_calls`, `model_cost` and `elapsed_seconds` are made available to templates (903-917) but the default `system.jinja` does not use them.
- A resumed session keeps its original system message (`docs/architecture.md:235-238`).
- The tool list is static for `bash` alone.
- Exception 1: with deferred MCP tools, calling `tool_search` adds tool definitions to the `tools` array (`agent_service.rs:182-196`), which changes the beginning of the request for the next call. The code sets `reported = None` and treats this as an estimate reset; the cost is a cache miss for the prefix after the tools (speculation about the server side effect: chat templates usually render tools inside the system block).
- Exception 2: compaction. A mask pass rewrites old tool messages (changes the prefix from the first masked message), and a summary pass replaces everything after the first user message (`compaction_service.rs:156-178, 293-313`). The code comments acknowledge the cache cost (`agent_service.rs:632-633`, `compaction_service.rs:151-152`).
- Messages are otherwise append only. Reasoning text is not sent back (`openai.rs:227-241`), so for a thinking model the history the server re-renders can differ from what it generated (speculation about the effect on the cache: only the previous assistant turn is re-processed; the live check above showed prefix reuse working for a simple case).

## 4. Tools

### `bash` (the only built in tool)
Defined at `src/services/agent_service.rs:941-946`.
- name: `bash`
- description: `Execute a bash command`
- schema:
```json
{ "type": "object", "properties": { "command": { "type": "string", "description": "The bash command to execute" } }, "required": ["command"] }
```
Sent to OpenAI compatible servers as `{"type":"function","function":{"name":"bash","description":"Execute a bash command","parameters":<schema>}}` (`openai.rs:41`).

Token estimate: Umbra's own estimate for this tool is about 38 tokens (`compaction_service.rs:89-91`, chars/4). In a Qwen chat template it will render larger (names, JSON, wrapper tags), likely 70 to 100 tokens. Very small.

### Execution details (`src/services/environment/local.rs`)
- Every command is a fresh process: `bash -c "exec 2>&1\n<command>"` (75-78) in the working folder. Nothing carries over (`cd`, `export`). stdout and stderr are merged in order.
- On Windows the shell is `C:\Program Files\Git\bin\bash.exe` (82-94). Fallback is `cmd /C`.
- stdin is closed (`Stdio::null()`, line 42).
- Timeout: `environment.timeout` default 30 seconds (`config.rs:159`). On timeout the process tree is killed, and the observation has `returncode: -1` and `exception_info`: `An error occurred while executing the command: Command '<command>' timed out after 30 seconds`, with whatever output was captured (131).
- A command that cannot start gives `An error occurred while executing the command: could not start <shell>: <error>` (46).
- Ctrl+C gives `The user interrupted this command (Ctrl+C) after N seconds` (132, 137).
- Output capture has no size cap in the environment; both pipes are read to the end on threads (163-173).
- Environment variables set for every command: `PAGER=cat MANPAGER=cat LESS=-R PIP_PROGRESS_BAR=off TQDM_DISABLE=1 GIT_PAGER=cat` (`config.rs:152`).

### How the output reaches the model
Rendered by `src/templates/observation_tools.jinja` for every tool result (bash and MCP). Under 10,000 characters:
```
{ "returncode": N, "output": "<json string>" [, "exception_info": "<json string>"] }
```
At 10,000 characters or more it is replaced by the first 5,000 characters and the last 5,000 characters plus the count:
```
{ "returncode": N, "output_head": "...", "output_tail": "...", "elided_chars": <length - 10000>, "warning": "Output too long." }
```
(`observation_tools.jinja:1-16`; a unit test at `prompt_service.rs:109-116` checks it.) The text mode template `observation_text.jinja` does the same with prose. The limit is in characters, not tokens or lines. It is JSON escaped, so newlines and quotes inflate the character count the model sees. The full output stays in the trajectory (`extra.raw_output`).

### Edit tool
There is none. The model edits with `sed`, heredocs, `python -c` or any other shell command. There are therefore no matching rules or edit failure messages to report. The command's own output and return code are the only feedback. If the model asks for a tool named `edit`, `read` or `write` it gets `Unknown tool 'edit'.` (`action_parser.rs:92`, and the test at 169-170 uses exactly `edit`). The error is delivered in the format error template and counts toward the three in a row limit.

### Other tools that may appear
- `tool_search` (only when MCP tools are deferred): `src/services/tool_search_service.rs:29-42`. Description: `Loads the definitions of deferred MCP tools so they can be called. The system prompt lists the deferred tools by name. Pass \`select:<name>,<name>\` to load tools by their exact names, or keywords to find tools whose name or description match. The tools found are loaded and can be called by name from the next reply.` Schema: `query` (string, required), `max_results` (integer).
- MCP tools: each becomes `mcp__<server>__<tool>` with description prefixed `[MCP server <name>] ` and the server's own schema (`agent_service.rs:948`). Deferred automatically when their estimated tokens exceed 10% of the context window (`config.rs:310`, `tool_search_service.rs:57-63`). The task-2588 TDD measured 124,715 tokens of MCP definitions across 229 tools on this machine. Use `--no-mcp` for evals.
- MCP call timeout 120 seconds, startup timeout 30 seconds (`config.rs:310`).

## 5. Compaction

Design document: `tasks/umbra-compaction-tdd.md`. Code: `src/services/compaction_service.rs`, `agent_service.rs:568-706`, `src/templates/compact_*.jinja`.

### Context window size for a local model
`resolve_window` (`compaction_service.rs:54-59`): `compaction.context_window` from config (or `--context-window`), else the provider. For OpenAI compatible providers: a built in prefix table (`tokens.rs:19-26`: claude, gpt-5, gpt-4.1, gpt-4o, o3, o4) and then, for a server on localhost, `GET <base>/props` reading `n_ctx` (`openai.rs:141-155, 176-180`), then `GET /models` reading `max_model_len`, `context_length`, `context_window` or `meta.n_ctx_train`. Final fallback 128,000. On this machine `/props` gives 262,144.

### Token counting
The size of the next request is the last reported `prompt_tokens` (input plus cached) plus `chars/4` for messages added since (`compaction_service.rs:105-110`, `agent_service.rs:549-556`). When no reported size exists (start of run, after any compaction) the whole history is estimated at `ceil(bytes/4) + 4` per message plus the tool definitions.

### Trigger and thresholds (defaults, `config.rs:356-373`)
| Setting | Value | At 262,144 window |
|---|---|---|
| `trigger_ratio` | 0.8 | 209,715 tokens |
| `target_ratio` | 0.6 | 157,286 tokens |
| `protect_recent_ratio` / `protect_recent_max` | 0.2 / 40,000 | 40,000 tokens (cap applies) |
| `min_reclaim_ratio` / `min_reclaim_max` | 0.1 / 20,000 | 20,000 tokens |
| `keep_recent_ratio` | 0.1 | 26,214 tokens |
| `mask_min_chars` | 400 | |

Compaction is checked before every model call (`agent_service.rs:410`). It also runs when the server rejects the request as too long (535-546), and on the `/compact [focus]` command at a terminal prompt (386-389).

### Pass 1: mask (no model call), `compact_by_masking` (635-651), `plan_mask` (156-178)
- Walks tool outputs from newest to oldest. Outputs after the last assistant message are never masked, because the model has not seen them yet.
- The newest seen outputs worth `protect_recent` tokens are kept. Outputs shorter than `mask_min_chars`, outputs already masked, and outputs of commands containing `SKILL.md` are kept.
- If the pass would free less than `min_reclaim` tokens, it masks nothing.
- Masked content becomes (`compaction_service.rs:191`): `[Output removed to save context after you read it: <chars> characters, return code <n>[ and N images]. Run the command again if you need it.]`. The command and the assistant text stay. The full output stays in `extra.raw_output` in the trajectory.
- So old tool outputs are pruned before any summary. They are kept intact for the whole run until the 80% trigger fires.

### Pass 2: summary (one model call), only when masking leaves the estimate above `target`
- `agent_service.rs:585-597`: skipped when the summarizable history is below `min_reclaim` (task-2588 fix), with a warning.
- `find_cut` (`compaction_service.rs:228-242`): the head is everything through the first user message (system prompt and original task). The tail is the newest turns up to `keep_recent` tokens, starting at an assistant message so tool results stay with their calls. Follow up tasks the person typed in the middle are kept (253-257).
- `request_summary` (323-335) sends head, middle, and this prompt as a final user message, with the run's own `tools` attached. On a context overflow of the summary request itself, the oldest half of the middle is dropped and it tries once more.
- New history: head, kept follow up tasks, one user message from `compact_resume.jinja`, then the tail (293-313). The summary call's cost is added to the totals and does not count as a step.
- On failure (call error, empty summary text) the history is left unchanged and a warning is printed (`agent_service.rs:668-673`). There is no flag preventing the same attempt before the next model call.

### Summary prompt (`src/templates/compact_summary.jinja`, exact)
```
The conversation so far is about to be compacted to save context. Older messages will be removed
and replaced by the summary you write now. The system prompt, the original task and your most
recent turns will be kept as they are.

Do not call any tools and do not run any commands. Reply with the summary as plain text only.

Write the summary so that you could continue the task from it alone, without the removed
messages. If the conversation contains an earlier summary, include everything in it that still
matters. Use these headings:

1. Task and requests: the task and every request or correction the user made, word for word.
2. Work done: what has been done so far, in order.
3. Files: every file created, changed or read that matters, with its full path and what changed.
4. Commands and findings: the important commands, what they showed, and facts you learned about
   the code, the environment or the tools.
5. Errors and fixes: each error you hit and how it was fixed, or that it is still open.
6. Current state: what is true right now, including tests that pass or fail.
7. Next step: exactly what you were about to do next.
```
plus, when `/compact <focus>` was used: `The user asked the summary to focus on: <focus>`.

### Resume message (`compact_resume.jinja`, exact)
```
This session was compacted to save context. The messages between the task above and the most
recent turns below were removed and replaced by this summary, which you wrote:

<summary>
{{ summary }}
</summary>

The outputs of earlier commands are no longer visible. Run a command again if you need its
output. Continue the task from where you left off.
```

### What is recorded
Each compaction adds a `CompactionRecord` to `info.compactions`: kind (mask or summary), reason (threshold, overflow, manual), step, `tokens_before`, `tokens_after`, window, masked indexes, and for summaries the summary text, cost and every removed message in full (`models/compaction.rs`, `docs/architecture.md:271-274`).

### Measured results in the TDD
`tasks/umbra-compaction-tdd.md:239-267`: Haiku at a forced 16,000 window and Qwen 3.8 27B at 262,144 with forced low ratios. All reads of seven files answered correctly. No benchmark with long multi file tasks exists.

## 6. Session management

### Storage
- Trajectory: `~/.umbra/trajectories/<session id>.traj.json`, or the path in `-o/--output` (`trajectory_repository.rs:13-19`). Pretty printed JSON in mini-swe-agent `1.1` format. The whole file is rewritten after every step through a temp file and rename (`trajectory_repository.rs:25-33`). It contains `info` (stats, config record, exit status, submission, compactions with removed messages, session block) and every message with `extra` (usage, cost, raw response, raw output, timestamp, images as base64).
- Index: `~/.umbra/sessions/<id>.json`, one small file per session with folder, model, first task (200 characters), status, calls, cost, message count, times (`session_repository.rs:23-33`, `models/session.rs:110-128`). `UMBRA_HOME_OVERRIDE` moves `~/.umbra` and the home folder for discovery.
- Session id: milliseconds since epoch (`session_repository.rs:18`).
- On this machine `~/.umbra` holds 23 index files and a `trajectories` folder. The config file is 37 bytes (section 8).

### Resume, continue, fork, list, name
- Resume: `--resume <id|id prefix|trajectory path>`, or `--resume` alone to pick from the 10 newest sessions of the current folder (`session_service.rs:37-69`). `--continue` takes the newest session of this folder. The resumed session runs in its original folder and model unless the command line says otherwise (`run_service.rs:280-287`). The exit marker is removed and unanswered tool calls get `This action was not run: the session ended before it finished.` (`session_service.rs:16, 124-146`). Each resume is recorded in `info.session.resumes`.
- Fork: `--resume X --fork` or `--continue --fork` copies the history to a new session id and file and leaves the old file untouched (`run_service.rs:322-347`). There is no branch tree inside a session, no way to branch at an earlier message, and no tree view.
- Listing: `umbra sessions [--cwd D] [--all] [--json]` (`session_service.rs:151-153`, `cli.rs:211-221`). A line shows id, age, status, calls, model, first 70 characters of the task.
- Naming: the first task text truncated to 200 characters (`agent_service.rs:375-379`, `run_service.rs:325`). No generated titles. A design for titles and a resume picker is in `tasks/umbra-improvements-v2-tdd.md:523-548` (not built).
- ACP: sessions are shared with the terminal (`session/list`, `session/load`, `session/resume`).

### Interrupt handling
Ctrl+C sets a flag checked every 100 ms by the model call, retry wait, running command, MCP call and prompts (`docs/architecture.md:203-226`). A model call in progress is dropped without keeping an assistant message. A running command is killed and its result says so. With a terminal, Umbra asks for a comment and sends it as `Interrupted by user: <text>`. With no terminal the run ends with status `Interrupted` and exit code 130. Lines typed while the agent works are queued and added before the next model call (`agent_service.rs:457-470`). Tests are in `tests/interrupt.rs` and `tests/console_windows.rs`.

### Images
`-i/--image PATH` (repeatable), image paths typed in a task, follow up or comment (`images.detect_paths`), and images returned by MCP tools. PNG, JPEG, GIF, WebP. Scaled so the long edge is at most 1,568 pixels, refused when base64 exceeds 10 MB (`config.rs:387-391`). Sent as `image_url` parts with `data:` URLs on the OpenAI path. Images returned by tools go in a follow up user message after the tool results (`openai.rs:192-223`). Stored in the trajectory as base64. The mask pass moves images of masked outputs to `extra.masked_images` and stops sending them (`compaction_service.rs:208-210`). Image token estimate is one token per 28 by 28 patch. A text only server errors and the run ends with `ModelError`.

## 7. Token and cost accounting

Recorded per model call (`ModelResponse.usage`, `models/model_response.rs:53-60`) and stored in the assistant message under `extra.usage`:
- `input_tokens`: prompt tokens that were not cached (`prompt_tokens - cached_tokens`, `openai.rs:260-267`)
- `output_tokens`: `completion_tokens`, which includes reasoning tokens
- `cache_read_tokens`: `usage.prompt_tokens_details.cached_tokens`
- `cache_write_tokens`: always 0 on the OpenAI path
- `extra.cost` in dollars (0 for a local model), `extra.finish_reason`, `extra.timestamp` (seconds, taken when the reply was processed), and `extra.response` holding the raw response (for streamed replies: reconstructed, `streamed: true`, with `usage`, and `reasoning_content` when the model produced it).

Totals in `info.model_stats` (`models/trajectory.rs:15-22`): `instance_cost`, `api_calls`, `input_tokens` (this total adds cached and cache write tokens to the uncached count, `agent_service.rs:748-752`), `output_tokens`, `compactions`. The cached count is not totaled separately, so cache hit rate has to be computed by summing `extra.usage.cache_read_tokens` over assistant messages. Summary calls are added to the totals but are not messages in the final history; their cost and text are in `info.compactions[].cost` and `.summary`.

Not recorded: request latency, time to first token, prompt processing speed, reasoning token counts (the streaming assembler keeps only `usage`; `completion_tokens_details.reasoning_tokens` and `timings` from NInfer are dropped when streaming, see section 0 item 6). Tool execution time can be derived only from message timestamps (assistant timestamp to the following tool message timestamp).

Reading it programmatically:
- The trajectory file: `python -c "import json;t=json.load(open(P));print(t['info']['model_stats'])"` and iterate `t['messages']` for `extra.usage`.
- The index file `~/.umbra/sessions/<id>.json` has `api_calls` and `cost` only.
- Live, per call numbers: only through `umbra acp` (`usage_update` with `used`, `size`, `cost`, `docs/acp.md`). `-v/--verbose` prints totals at the end (`run_service.rs:452-455`).
- Pricing: `src/services/providers/pricing.rs`; `input_cost_per_mtok` and `output_cost_per_mtok` on a provider make a price for any model.

## 8. Provider and model configuration

### Config files (`config_repository.rs:11-36`, `docs/configuration.md`)
Merge order, later wins key by key: built in defaults, `~/.umbra/config.toml`, `<project root>/.umbra/config.toml`, each `-c/--config FILE` in order (TOML, or a mini-swe-agent YAML which forces `finish = "submit"`), then CLI flags. `umbra config` prints the merged result. `UMBRA_HOME_OVERRIDE=<dir>` replaces the home folder, so `<dir>/.umbra/config.toml` is read, `<dir>/.claude/...` is what discovery sees, and sessions and trajectories go under `<dir>/.umbra/`.

### Pointing Umbra at a local OpenAI compatible server
The built in provider `local` is `kind = "openai"`, `base_url = "http://localhost:8080/v1"`, key env `LOCAL_API_KEY` (`config_repository.rs:147`). No key is needed for localhost (`providers/mod.rs:138-143`). So this already works with no config:
```
umbra -m local/ninfer-qwen3.8-27b-coletti-v9 ...
```
(the live test default, `tests/live_local.rs:22`). llama.cpp ignores the model name; the name is sent as `model`. For another server or sampling settings:
```toml
# eval.toml, passed with -c
[providers.box]
kind = "openai"
base_url = "http://127.0.0.1:8080/v1"
max_tokens = 8192
temperature = 0.6
stream = true                  # default true; false stores the whole reply JSON including timings
extra_body = { top_p = 0.95, top_k = 20, min_p = 0.0, chat_template_kwargs = { enable_thinking = false } }
```
Use it as `-m box/<model name>`.

Environment variables read: `UMBRA_MODEL` (model when none is configured, `run_service.rs:54-56`), `LOCAL_API_KEY` or the provider's `api_key_env`, `UMBRA_HOME_OVERRIDE`, `UMBRA_RETRY_BASE_MS`, `UMBRA_FORCE_INTERACTIVE`, `UMBRA_BANNER`, `UMBRA_BANNER_ANIMATION`, `UMBRA_STATUS_BAR`, `UMBRA_FULLSCREEN`, `UMBRA_REPLAY_RECORD`. Flags: `-m`, `-c`, `--text`, `--finish`, `--cwd`, `--context-window`, `--docker`, `--no-mcp`, `--mcp`, `--no-skills`, `--no-instructions`, `--no-source` (`cli.rs:79-187`).

### Sampling parameters sent
Only these, and only when set: `max_tokens` (`ProviderConfig.max_tokens`), `temperature`, plus anything in `extra_body` (`openai.rs:43-49`). Defaults send neither, so the server's sampling defaults apply. `top_p`, `top_k`, `min_p`, repetition penalty and seed have no dedicated field and must go through `extra_body`. The 16,000 `max_tokens` default in the docs applies to Anthropic only.

### Thinking and reasoning
Umbra does not turn thinking on or off for OpenAI compatible servers. Whatever the server's chat template does by default happens. `reasoning_content` or `reasoning` from the stream is stored in the trajectory raw record and never sent back or shown (`openai.rs:346`, `227-241`). Reasoning tokens count in `completion_tokens` and so in `output_tokens`. To disable thinking for a Qwen template, use `extra_body = { chat_template_kwargs = { enable_thinking = false } }` (in `docs/providers.md:81`). Anthropic has `effort`; nothing for OpenAI.

### Default model setup on this machine
`C:/Users/jason/.umbra/config.toml` contains exactly one line, `model = "anthropic/claude-haiku-5-5"` (37 bytes). There is no provider table, so a plain `umbra "task"` here uses Claude Haiku 5.5 (via `ANTHROPIC_API_KEY` or the Claude Code login), not the local model. The local server on port 8080 is NInfer serving `ninfer-qwen3.8-27b-coletti-v9`, single slot, window 262,144, with `max_model_len` 262144 in `/v1/models`. Other tests and TDDs call the local model "Qwen 3.8 27B".

## 9. Non interactive use

### Headless mode
There is no flag called headless or print. A run is headless when stdin is not a terminal. In that case `Interaction.interactive` is false (`interaction_service.rs:72`), the run refuses to start unless yolo is set (`run_service.rs:148-150`: "stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking"), it does not ask for a follow up task (`follow_up_task`, 134-137), does not raise limits (`raise_limits`, 161), and ends on Ctrl+C with `Interrupted`. The banner and the status bar are not drawn into a pipe by default. `--quiet` prints only the final answer on stdout.

### Exit codes
0 for `Replied` or `Submitted`; 130 for `Interrupted`; 1 for any other status (`LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError`, `ModelError`); 2 when startup fails (`main.rs:34-36`, `run_service.rs:180-184`).

### Exact command lines
One prompt, run to completion, local model, no user context, bounded:
```
umbra -m local/ninfer-qwen3.8-27b-coletti-v9 --yolo --quiet --no-fullscreen --exit-immediately --no-mcp --no-skills --no-instructions --step-limit 60 --cwd C:/path/to/workdir -o C:/path/to/out.traj.json -t "the task text" < /dev/null
```
With an isolated home so no `~/.claude` files or sessions are touched, and a config for the bash timeout and a wall time limit:
```
UMBRA_HOME_OVERRIDE=C:/path/to/emptyhome umbra -c C:/path/to/eval.toml -m local/ninfer-qwen3.8-27b-coletti-v9 --yolo --quiet --no-mcp --cwd C:/path/to/workdir -o C:/path/to/out.traj.json -t "the task text" < /dev/null
```
with `eval.toml`:
```toml
wall_time_limit_seconds = 1200
step_limit = 80
[environment]
timeout = 300
[compaction]
context_window = 65536        # optional, to force compaction at a chosen size
```
Resume a session headless: add `--resume <id> -t "next task"`. Fork: `--resume <id> --fork`. Compaction off: `--no-compact`. Context window override: `--context-window N`. Submit style (every reply needs a command, sentinel ends): `--finish submit`. Text mode: `--text`. Docker: `--docker IMAGE`. Print the exact first request: `umbra prompt "task"`. Replay a script without a model: `-m replay/script.json`; `UMBRA_REPLAY_RECORD=file` logs each request as a JSON line.

### JSON event output
None for `umbra` itself. `umbra acp` speaks JSON-RPC (ACP protocol version 1, `src/services/acp/mod.rs:34`) on stdin/stdout and sends `session/update` notifications: `agent_message_chunk`, `tool_call` and `tool_call_update` (with output up to 20,000 characters), `usage_update {used, size, cost}`, and a `notice` for compaction and format errors (`docs/acp.md`). `tests/acp.rs` shows a client driving it. Each `session/prompt` answer carries a `stopReason` (`end_turn`, `max_turn_requests` or `cancelled`).

### Smallest change for a JSON event mode
`AgentEvents` (`src/services/agent_events.rs:10-50`) already has the hooks `reply`, `action_started`, `action_finished`, `action_not_run`, `notice` and `usage`. `run_task` hard codes `events: Box::new(NoEvents)` at `run_service.rs:170`. Add a `--json` flag in `RunArgs` (`cli.rs`), a `JsonEvents` struct implementing the trait that prints one JSON object per line to stdout, pass it at that line, and force `--quiet` behavior. About 60 to 80 lines. To record latency and cache hits add `timings` and `reasoning_tokens` capture in `StreamAssembler::push_chunk` (`openai.rs:328-351`) and store them in `Usage` or `extra`. A `--wall-time` flag would be a 3 line addition in `cli.rs` and `apply_run_args`.

## 10. Existing tests and eval code

### Tests
- Run with `cargo test` (unit tests inline plus integration tests in `tests/`). Each integration test starts the real `umbra` binary with a fake home (`UMBRA_HOME_OVERRIDE`) and `-m replay/<script>.json` scripted replies (`tests/common/mod.rs`, `tests/agent_loop.rs`). They cover the loop, format errors, compaction, sessions, images, MCP (fake stdio server), discovery layouts for other harnesses, terminal output, status bar, interrupts, and ACP.
- `tests/console_windows.rs` and `tests/interrupt.rs` need a real Windows console.
- `tests/live_local.rs` has 4 or more `#[ignore]` tests that need the local model: image reading, resume memory, Ctrl+C while the model writes, Ctrl+C on a running command. Run with `cargo test --test live_local -- --ignored --test-threads 1`. `UMBRA_LIVE_MODEL` picks the model; `UMBRA_LIVE_EVIDENCE=<dir>` keeps the trajectories.
- CI: `.github/` was not part of this survey; `docs/releasing.md` covers releases.

### Eval or benchmark code
None. There is no benchmark runner, no task set, no grading script, and no performance measurement code. The compaction TDD (`tasks/umbra-compaction-tdd.md:239-267`) records one off manual runs with trajectories under `_agent_output/task-2562-compaction/`. The submit style and trajectory format are kept compatible with mini-swe-agent so its SWE-bench tooling can read trajectories (`finish = "submit"`, `README.md:177`), but nothing in this repository runs it.

## 11. Weaknesses and risks for a 27B local model

Each item: where, and whether observed in code or speculation.

### Accuracy
1. **No edit or read tool.** Observed: only `bash` is built in (`agent_service.rs:941-950`). Editing is `sed`, heredocs or scripts (`system.jinja:26`). Failure modes: escaping errors for quotes and special characters in heredocs, `sed` regex mistakes, silent wrong edits with no diff shown, line ending and Git Bash path problems on Windows. The effect on accuracy for a 27B model is speculation, but a dedicated exact match edit tool is the common design in other coding agents.
2. **A plain text reply ends the turn.** Observed: any assistant reply with no tool call is accepted as the final answer (`action_parser.rs:27-38`, `agent_service.rs:769-775`). A model that writes "Let me check the tests." without a call ends the task early. The prompt tries to prevent it (`system.jinja:18`). How often a 27B model does this is speculation.
3. **No verification step.** Observed: the reply style prompt says "run the build or tests that check it" (`system.jinja:10`) and that is all. There is no check that tests were run, no nudge before finishing, no "you have not run anything since your last edit" prompt. The older submit style has a 6 step workflow with "Verify your fix works" but it is not the default.
4. **Malformed tool call arguments are not repaired.** Observed: invalid JSON arguments become a format error (`openai.rs:274-279`, `action_parser.rs:80-83`). The bad reply is dropped from history and the model sees only the error text, so it cannot see what it wrote (`agent_service.rs:785`). Three in a row end the run with `RepeatedFormatError`. There is no lenient JSON repair and no re-prompt that includes the model's own call.
5. **One bad call rejects all calls in the reply.** Observed (`action_parser.rs:95`).
6. **Fresh shell for every command.** Observed (`local.rs:75-78`). The model must prefix every command with `cd <dir> &&` or use absolute paths, repeatedly. Builds with environment setup, virtualenv activation and `export` do not persist. Speculation about how often a 27B model forgets.
7. **30 second command timeout.** Observed (`config.rs:159`). Test suites, builds, installs and model downloads are killed with `returncode: -1`. The default is the mini-swe-agent value. An eval needs `environment.timeout` raised.
8. **Timeout and truncation messages are mini-swe-agent's, not model friendly.** Observed: `An error occurred while executing the command: Command '<cmd>' timed out after 30 seconds` (`local.rs:131`) and a JSON `warning: "Output too long."` with no advice on how to narrow the output (`observation_tools.jinja:13`).
9. **Instruction and skill content competes for attention.** Observed: with the default discovery on this machine the system prompt is about 17,300 tokens, of which 10,200 are the user's own CLAUDE.md (formatting and style rules, 40 KB) and 6,500 are a skills list (`umbra prompt` measurement above). For an eval, this adds noise. Effect on a 27B model is speculation.
10. **No loop or stall detection.** Observed (section 2). A model that repeats the same failing command has no limit except `step_limit` (default 0, off) and wall time (default 0, off).
11. **Reasoning is discarded.** Observed (`openai.rs:227-241`): `reasoning_content` is not sent back, which matches Qwen template behavior for previous turns but means the model cannot see its earlier plan after a tool result. Speculation about effect.
12. **Thinking mode is not controlled.** Observed: nothing sets `enable_thinking`. If the server template defaults to thinking, every call spends reasoning tokens counted in `output_tokens` (speculation about the default for this server; not tested).

### Token cost and latency
13. **Tool outputs are kept at full size until 80% of the window.** Observed (`compaction_service.rs:156-178`, `config.rs:361`). With a 262,144 window nothing is pruned until 209,715 tokens, and then outputs older than the newest 40,000 tokens are masked in one batch. A 27B model is likely to be slower and less accurate at 100,000+ tokens of context (speculation) and each call re-reads a growing prefix. A single large output up to 10,000 characters per call, about 2,500 tokens, is the per call cap (`observation_tools.jinja`).
14. **Output truncation cuts the middle by characters.** Observed (`observation_tools.jinja:10-11`): first 5,000 and last 5,000 characters, JSON escaped. Nothing is done about very long single lines or binary output, and the environment itself reads all output into memory (`local.rs:163-173`).
15. **Cache breaking compaction.** Observed in comments (`agent_service.rs:632-633`): masking rewrites earlier messages, so the server re-prefills from the first masked message. The summary pass rewrites everything after the first user message. On a single slot server a re-prefill of 100,000 tokens takes minutes at the 70 tokens per second prompt speed seen in the 30 token request above (the speed number is one small sample; speculation for large prompts).
16. **Summary call attaches the tools and the history with no output budget.** Observed (`compaction_service.rs:323-335`): no `max_tokens` unless configured. A model that replies with a tool call or only reasoning yields `the model returned no summary text`, the history is left as it was, and the same attempt can repeat before every later call (`agent_service.rs:597`, no suppression flag) (the repeat is observed in code; how often it happens is speculation).
17. **`input_tokens` in totals include cached tokens.** Observed (`agent_service.rs:750`). Reading `model_stats.input_tokens` as "tokens processed" overstates the work when the cache works. Per call cached counts exist in each message's `extra.usage`.
18. **Trajectory rewrite is O(size) per step.** Observed (`agent_service.rs:921-936`, `trajectory_repository.rs:25-33`): the entire pretty printed file is rewritten after every step, with each output stored twice (`content` and `extra.raw_output`) and the raw response stored too. For long runs with large outputs this is megabytes per step (speculation on the actual time cost).
19. **Streaming assembles the whole reply before showing anything.** Observed (`openai.rs:102-116`). No effect on correctness; it hides time to first token from any measurement.
20. **Only `bash` is sent, and it is tiny.** Observed: the tool definition is about 40 tokens by Umbra's estimate. This is a strength for token cost and for KV cache stability.
21. **The prefix is stable.** Observed (section 3). System prompt, tool list and history are append only until a compaction or a `tool_search` load.

### Measurement gaps
22. **No timings, no reasoning token counts when streaming** (section 0 item 6). Observed.
23. **No per tool latency**, only message timestamps. Observed.
24. **Cost limit is not usable for a local model** (price 0), and `wall_time_limit_seconds` has no CLI flag. Observed (`pricing.rs:54`, `cli.rs`).
25. **Default model on this machine is Claude Haiku**, not the local model (`~/.umbra/config.toml`). An eval must pass `-m` explicitly. Observed.
26. **Windows specifics.** Observed: commands run through Git Bash; the working directory in the prompt is a Windows path (`C:\...`), the shell expects Unix style paths in some tools; the prompt does not explain this. Effect is speculation.
