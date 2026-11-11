# Web research for task-2614: pi versus Umbra on a local Qwen model

Date of research: 2026-11-10. Web only, no code written.

How to read this report. Every claim has a source URL. Pages were read through a fetch tool that summarizes the page with a small model, so quoted strings are only as exact as that summary. Anything that came from a secondary source, a search snippet, or an inference is marked "unverified". The pi repo is github.com/badlogic/pi-mono. The npm package is now published as `@earendil-works/pi-coding-agent` (the old name was `@mariozechner/pi-coding-agent`). Both names appear in current sources (https://huggingface.co/docs/hub/main/agents-local.md, https://docs.ollama.com/integrations/pi.md). The pi docs below are from the main branch in November 2026 and differ from Mario's November 2025 blog post. Where they differ, the docs are newer.

---

## 1. pi in detail

### 1.1 Design claims by the author (blog, 2025-11-30)
Source: https://mariozechner.at/posts/2025-11-30-pi-coding-agent/
- System prompt plus tool definitions are "below 1000 tokens" in total (author's statement; no separate system prompt count given).
- Four tools by default: read, write, edit, bash. Optional read only tools grep, find, ls, off by default.
- No sub agents, no MCP (Playwright MCP is cited as 21 tools and 13.7k tokens), no plan mode, no built in to dos, no background bash (use tmux), no permission prompts.
- Only AGENTS.md (global and project) is appended to the system prompt. The whole system prompt can be replaced.
- Compaction was "not implemented, planned (issue #92)" in that post. It exists now (section 1.5).
- Token tracking is described as best effort, not reliable enough for billing.
- Tool arguments are validated with TypeBox and AJV.
- The post says the author fits "hundreds of exchanges" in one session.

### 1.2 System prompt (current main)
Sources: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/src/core/system-prompt.ts and the published build at https://app.unpkg.com/@earendil-works/pi-coding-agent@0.83.0/files/dist/core/system-prompt.js
- The prompt is built from tagged sections: preamble (untagged), `<tools>`, `<rules>`, `<docs>`, an addendum for `APPEND_SYSTEM.md`, `<project_context>` (each context file in `<project_instructions path="...">`), skills, and the working directory.
- Opening lines (from the 0.83.0 build): "You are an expert coding assistant operating inside pi, a coding agent harness." and "You help users by reading files, executing commands, editing code, and writing new files."
- `<tools>` lists `- name: snippet` for each tool. Read snippet: "Read file contents". Edit snippet: "Make precise file edits with exact text replacement, including multiple disjoint edits in one call".
- Rules include: "Use bash for file operations like ls, rg, find" (only when grep, find, ls are absent), tool guidelines (read: "Use read to examine files instead of cat or sed"), "Be concise in your responses", "Show file paths clearly when working with files".
- `<docs>` section: "Pi documentation (read only when the user asks about pi itself, its SDK, extensions, themes, skills, or TUI)" with paths to the README, docs and examples.
- Skills add only name, description and path of each skill to the prompt; the model reads SKILL.md on demand (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/skills.md).
- Exact token count of the current prompt: unverified. The author's "below 1000 tokens" claim is for the 2025 version. The current docs section and XML tags likely add some. Measure it by capturing the request body from llama.cpp.
- `SYSTEM.md` replaces the default prompt and `APPEND_SYSTEM.md` appends, in `~/.pi/agent/` and `.pi/` (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/configuration.md).
- Context files: `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`, loaded from the agent dir, the cwd and parent directories. Same source.

### 1.3 Tools and schemas
Sources: files under https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent/src/core/tools (raw URLs `.../src/core/tools/read.ts`, `edit.ts`, `bash.ts`, `truncate.ts`).
- read: `path` (string, required), `offset` (number, 1 indexed), `limit` (number). Text plus images (jpg, png, gif, webp, bmp). Default truncation 2000 lines or 50 KB, whichever comes first. Output ends with a notice giving the shown range and the next offset to use. A first line larger than the byte limit yields a notice pointing at a `sed | head -c` bash fallback.
- write: `path`, `content`. Creates parent directories (blog post).
- edit (current main): `path` plus `edits[]` where each item has `oldText` and `newText`. Exact text replacement. Each `oldText` must be unique in the original file and must not overlap other edits in the same call. All edits match against the original file in one pass. BOM is stripped and restored, text is normalized to LF for matching and original line endings are kept. Success text: "Successfully replaced {n} block(s) in {path}." Argument normalization accepts `edits` as a JSON string, a single object, or the legacy top level `oldText`/`newText`. Fuzzy matching and the uniqueness error text live in `edit-diff.ts`, which was not read (unverified). The 2025 blog describes the older single replacement form.
- bash: `command` (string), `timeout` (number of seconds, optional, "no default timeout"). On timeout the process tree is killed ("Command timed out after N seconds"). Output limit: last 2000 lines or 50 KB (tail kept), full output saved to a temp file with prefix `pi-bash`, path shown in the result. Non zero exit marks the result as an error with "Command exited with code N". Abort kills the whole process group.
- Truncation constants: `DEFAULT_MAX_LINES` 2000, `DEFAULT_MAX_BYTES` 51,200, `GREP_MAX_LINE_LENGTH` 500 (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/src/core/tools/truncate.ts). Head truncation for read, tail truncation for bash.
- Exact tool description sizes in tokens: unverified; not measured. Descriptions are short paragraphs.
- Default tool set `["read","bash","edit","write"]`, configurable with `defaultTools` and `-t/--tools`, `-nt`, `-nbt`, `-xt` (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/cli.md).
- The settings page also lists a `codemode` setting (`mode` "on" or "only", `inlineBudget` 3000). What it does was not researched (unverified) (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/settings.md).

### 1.4 Agent loop
Source: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/agent/README.md and https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/how-pi-works.md
- A turn is one LLM call plus the tool calls it makes. Sequence: `prepareRequest`, provider response, tool results, `finishTurn`, `turn_end`, then continue or `agent_end`.
- Tool calls in one assistant message run in parallel by default (preflight is sequential), with a `sequential` mode. Tool results are persisted in the assistant's source order.
- Thrown tool errors are reported to the model as tool results with `isError: true`. Behavior for invalid arguments was not stated in the page (unverified), though arguments are validated against the schema before `beforeToolCall`.
- The loop ends when there are no tool calls and no queued steering or follow up messages. Error and aborted responses are hard exits.
- Steering messages are injected after the current assistant message's tool calls. Follow up messages run only when nothing else is pending.
- No loop or stall detector is described in the docs read (unverified; absence not confirmed).

### 1.5 Compaction
Sources: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/compaction.md, https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/src/core/compaction/compaction.ts, https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/settings.md
- Trigger: `contextTokens > contextWindow - reserveTokens`. Defaults: `reserveTokens` 16384, `keepRecentTokens` 20000, `enabled` true. Per model overrides under `compaction.modelOverrides`.
- Checked after tool results are appended (before the next assistant response), before a new user prompt, and after a run ends. A provider context overflow error triggers one compact and retry attempt. Manual: `/compact [instructions]`.
- Token estimate is characters divided by 4, rounded up. An image counts as 4,800 characters. When provider usage exists, pi uses the last assistant usage plus estimates for later messages (compaction.ts).
- Cut point: walk backward summing estimates until `keepRecentTokens` is reached. Valid cut points are user, assistant, bash execution and custom messages. A cut never lands on a tool result. If a single user turn is larger than the keep budget, the cut falls inside it, pi writes a history summary and a turn prefix summary and merges them.
- Summary input: messages serialized to labeled text, thinking blocks dropped, each tool result cut to 2000 characters.
- Summary prompt (paraphrase; exact text is in compaction.ts and not fully quoted): "The messages above are a conversation to summarize. Create a structured context checkpoint summary" with sections Goal, Constraints & Preferences, Progress (Done, In Progress, Blocked), Key Decisions, Next Steps, Critical Context. It asks for exact file paths, function names and error messages. A second prompt ("The messages above are NEW conversation messages to incorporate into the existing summary") updates the previous summary on repeated compaction. A third, for split turns, uses Original Request, Progress So Far, Context Needed to Continue.
- The summary has no explicit max token cap; it uses the model's output limit. A summary that stops on the length limit is treated as a failure.
- File tracking: `<read-files>` and `<modified-files>` blocks accumulate across compactions.
- Result: a `compaction` entry (`summary`, `firstKeptEntryId`, `tokensBefore`, optional `usage`) is appended. Rebuilt context is summary plus messages from `firstKeptEntryId` on. Original entries stay in the file.
- Branch summaries (`/tree`): find the deepest common ancestor of the old and new leaf, collect entries from the old leaf back to it, newest first within a token budget, generate a structured summary (same sections except Critical Context), append a `branch_summary` entry. `branchSummary.reserveTokens` default 16384.
- A new entry type `context_edit` (append only edit of an earlier entry's context, fields `targetId`, `replacement`) exists in the session format. Its use is not documented in the pages read (unverified).

### 1.6 Session storage
Source: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/session-format.md and `.../docs/sessions.md`
- JSONL, one object per line. Line 1 is a header: `{"type":"session","version":3,"id":"uuid","timestamp":"...","cwd":"..."}`, optional `parentSession`.
- Every other entry has `type`, `id` (8 hex characters, UUID fallback), `parentId` (null for the root) and `timestamp`. The file is a tree. Branching appends a child to an earlier entry. The "leaf" marks the current position.
- Entry types: `message`, `model_change`, `thinking_level_change`, `usage`, `compaction`, `context_edit`, `branch_summary`, `custom`, `custom_message`, `label`, `session_info`.
- Roles in `message`: user, assistant, toolResult, system, custom. Assistant messages have `api`, `provider`, `model`, `usage`, `stopReason`, `timestamp`, `thinkingLevel`.
- Default location `~/.pi/agent/sessions/` grouped by cwd. Overrides: `--session-dir`, env `PI_CODING_AGENT_SESSION_DIR`, setting `sessionDir`.
- Commands and flags: `--continue` (latest in cwd), `--resume`, `--session <id|path>`, `--fork`, `--name`, `--no-session` (in memory, cannot resume), `/tree`, `/fork` (new session from an earlier user message), `/clone` (copy active branch).

### 1.4b Prompt caching behavior
- For the OpenAI completions API, `compat` fields `promptCacheKeyMode`, `sendSessionAffinityHeaders`, `cacheControlFormat` control cache hints. Defaults are auto. `PI_CACHE_RETENTION=long` enables extended retention on providers that support it (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/models-configuration.md, https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/environment-variables.md).
- The how-pi-works page does not discuss caching (unverified whether pi keeps the system prompt byte stable between turns). The agent core documents `diffSystemPromptSections`, which suggests the prompt can change when sections change (system-prompt.ts). For llama.cpp the cache works by prefix match on the server, so pi's history being append only matters. Pi's history rebuild after compaction replaces old messages by a summary, which invalidates the prefix at that point (inference).
- A user supplied `thinkingTokenBudgetField: "thinking_budget_tokens"` setting exists for llama.cpp.

### 1.5b Retry
Source: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/settings.md
- Defaults: `retry.enabled` true, `maxRetries` 3, `baseDelayMs` 2000, `maxAgentDelayMs` 60000, `retry.provider.timeoutMs` = `httpIdleTimeoutMs` (300000), provider `maxRetries` 0.
- Events `auto_retry_start` and `auto_retry_end` are emitted (json.md). Retry is for provider errors. Retry on a malformed model tool call is not described (unverified).

### 1.7 Models with no native tool calling
- Not found in the docs read. Pi uses the OpenAI tools API through its `openai-completions` adapter. Compat flags exist (`requiresThinkingAsText`, `supportsStrictMode`, `requiresToolsWithToolHistory`), but no text based tool protocol fallback was found (unverified; absence not confirmed). The blog says "self hosted models that work with tool calling" are supported, which implies tool calling is required.

### 1.8 Thinking levels
- `--thinking off|minimal|low|medium|high|xhigh|max`, clamped to the model's capability, also as a suffix `--model id:high`. Default `defaultThinkingLevel` "medium". `thinkingFormat` compat maps levels to request fields. For Qwen: `qwen` (top level `enable_thinking`), `chat-template`/`qwen-chat-template` (via `chatTemplateKwargs`), or `thinkingTokenBudgetField` (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/models-configuration.md).
- A blog about pi with llama.cpp says the llama.cpp extension maps thinking levels to `reasoning_budget_tokens` (https://blob42.xyz/post/building-a-local-agent-stack-with-llama.cpp-and-pi.dev/). Secondary source.

### 1.9 Headless modes and the exact commands
Sources: https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/cli.md, json.md, rpc.md, rpc-commands.md
- `-p`/`--print`: run the prompts, write the final assistant text to stdout, exit. Piped stdin is prepended to the first prompt.
- `--mode json`: run the prompts, write JSONL events to stdout, exit. The first line is the session header. Stdout carries only protocol records.
- `--mode rpc`: JSONL commands on stdin, events and responses on stdout. LF framing only. Commands include `prompt`, `steer`, `follow_up`, `abort`, `get_state`, `get_messages`, `get_session_stats`, `compact`, `set_auto_compaction`, `set_model`, `set_thinking_level`, `new_session`, `fork`, `bash`. `prompt` response `data.disposition` is `started`, `queued` or `handled`; wait for the `agent_settled` event.
- `--no-session`: in memory session.
- Model flags: `--provider <name>`, `--model <pattern>` (id, fuzzy name, `provider/id`, optional `:thinking`), `--models`, `--api-key`, `--list-models`.
- Tool flags: `-t`, `-nt`, `-nbt`, `-xt`.
- Env: `PI_CODING_AGENT_DIR` (default `~/.pi/agent`), `PI_OFFLINE=1`, `PI_SKIP_VERSION_CHECK`, `PI_TELEMETRY=0`, `PI_CACHE_RETENTION`.

`~/.pi/agent/models.json` for a llama.cpp server (pattern from https://huggingface.co/docs/hub/main/agents-local.md with compat fields from models-configuration.md; the compat and sampling values are my recommendation, not copied from a doc example):
```json
{
  "$schema": "https://pi.dev/schemas/models.schema.json",
  "providers": {
    "llamacpp-local": {
      "baseUrl": "http://127.0.0.1:PORT/v1",
      "api": "openai-completions",
      "apiKey": "none",
      "compat": {
        "supportsDeveloperRole": false,
        "supportsReasoningEffort": false,
        "supportsUsageInStreaming": true,
        "maxTokensField": "max_tokens"
      },
      "models": [
        {
          "id": "qwen",
          "reasoning": false,
          "contextWindow": 65536,
          "maxTokens": 8192,
          "samplingParams": { "temperature": 0.7, "top_p": 0.8, "top_k": 20, "min_p": 0.0 }
        }
      ]
    }
  }
}
```
Notes on the file. The `id` is sent as the model name. llama-server accepts any name unless `--alias` is set. Field names `supportsDeveloperRole`, `supportsReasoningEffort`, `supportsUsageInStreaming`, `maxTokensField`, `samplingParams`, `contextWindow` (default 128000) and `maxTokens` (default 16384) are in models-configuration.md. Without `contextWindow` set, pi assumes 128000 and would compact too late.

Non interactive run with machine readable usage (the flag combination is assembled from cli.md; the final prompt as a positional argument is inferred from the print mode example `git diff | pi --print "Review this change"`, not verified for `--mode json`):
```
PI_OFFLINE=1 PI_SKIP_VERSION_CHECK=1 PI_TELEMETRY=0 pi --mode json --no-session --provider llamacpp-local --model qwen --thinking off "TASK TEXT" > run.jsonl
```
Usage per turn: parse the `turn_end` and `message_end` events. The `message` there is an assistant message with a `usage` object `{input, output, cacheRead, cacheWrite, totalTokens, cost{input,output,cacheRead,cacheWrite,total}}` (session-format.md). `message_update` also carries a cumulative `usage` that may stay zero until completion (json.md). The doc does not state which field of `turn_end` holds usage (unverified); check one real run. For whole run totals use RPC `get_session_stats`, which returns `tokens.input/output/cacheRead/cacheWrite/total` and `cost` (rpc-commands.md). Pi's own token tracking is "best effort" (blog). llama.cpp reports `usage.prompt_tokens_details.cached_tokens` and `timings.cache_n`, so a proxy log on the llama.cpp side is a second source of truth.

The llama-cpp.md page in the pi docs describes a built in provider for a "llama serve" router with `/login llama.cpp` and `/llama`, default URL `http://127.0.0.1:9931`, env `LLAMA_BASE_URL`, `LLAMA_API_KEY` (https://raw.githubusercontent.com/badlogic/pi-mono/main/packages/coding-agent/docs/llama-cpp.md). The page content returned by the fetch tool also contained odd material about classifier models, so treat that page as unverified. A custom provider named `llamacpp-local` as above avoids it. A secondary source says the llama.cpp extension applies to providers whose name starts with `llama` (https://blob42.xyz/post/building-a-local-agent-stack-with-llama.cpp-and-pi.dev/); choose a provider name that does not start with `llama` to avoid the extension (unverified side effect).

---

## 2. Published benchmark results for pi

- Terminal-Bench 2.0 with Claude Opus 4.5, five trials per task, compared against Codex, Cursor and Windsurf with their native models. The blog shows the standings only as images, so no numbers could be extracted (https://mariozechner.at/posts/2025-11-30-pi-coding-agent/). A second run restricted to European time hours was in progress.
- Harbor adapter repo: https://github.com/badlogic/pi-terminal-bench. Command: `harbor run -d terminal-bench@2.0 --agent-import-path pi_terminal_bench:PiAgent -m anthropic/claude-sonnet-4-5 -n 4`. Leaderboard run uses `--k 5`. No scores in the README.
- Current pi score on tbench.ai: not found (the leaderboard page returned no rows to the fetch tool). Unverified. https://www.tbench.ai/leaderboard/terminal-bench/2.0
- What the author credits (blog): full control over context, full visibility into every tool call, small stable system prompt and toolset, no hidden injections, cross provider handoff, Terminus 2 (a tmux only agent) as evidence that minimal designs compete.
- Note: Hugging Face's guide says pi is the agent behind OpenClaw (https://huggingface.co/docs/hub/main/agents-local.md).

---

## 3. What other harnesses do that measurably helps

### Edit formats
- Aider unified diff format cut "lazy coding": GPT-4 Turbo 20% with SEARCH/REPLACE to 61% with unified diffs on an 89 task refactoring benchmark, lazy comments in 12 tasks down to 4 (https://aider.chat/docs/unified-diffs.html). Old, GPT-4 specific.
- Aider polyglot with Qwen3 (225 Exercism exercises, six languages), whole versus diff (https://aider.chat/2025/05/08/qwen3.html):
  - Qwen3 235B A22B, vLLM bf16, no think: whole 65.3% (100% well formed), diff 61.3% (94.7% well formed).
  - Qwen3 32B, vLLM bf16, no think: whole 45.8% (100%), diff 41.3% (94.2%).
  - Qwen3 235B Q5_K_M on llama.cpp, whole: 59.1% (100%).
  - Qwen3 32B via OpenRouter with thinking, diff: 40.0% (83.6% well formed).
  - Thinking mode scored lower than no think for the same model. Recommended non thinking sampling: temperature 0.7, top_p 0.8, top_k 20, min_p 0.
  - Malformed diffs were a common error source.
- A local test with Aider and Ollama: qwen3-coder:30b scored 0/100 with diff and 100/100 with whole on a small task, and the author attributes it to format mismatch (https://lostechies.com/erichexter/2026/06/03/local-llm-bench-part-4-harness-optimization/). Single author, small tasks. The same post found that a default Ollama context of 2048 tokens caused looping, and that a 32768 context fixed it.
- Aider format descriptions: whole, diff, diff-fenced (path inside the fence, for Gemini), udiff, editor-diff and editor-whole for architect mode (https://aider.chat/docs/more/edit-formats.html).
- SWE-agent ACI paper: removing the dedicated edit action (with linting) lowered the SWE-bench Lite solve rate from 18.0% to 10.3% in one summary; a 100 line viewer window performed best (secondary summary, https://arxiv.org/pdf/2405.15793.pdf; unverified against the paper tables).
- Codex CLI uses a freeform `apply_patch` custom tool with add, delete and update file hunks, parsed and verified before write (https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/apply_patch.rs). No benchmark delta found.
- Pi uses exact match edits with batched disjoint edits. No published reliability numbers found.
- Implication for Umbra: a whole file fallback for small files and a patch with a linter or syntax check guard are the two options with measured evidence on local Qwen sized models. Exact match replace with uniqueness errors is what pi, opencode and Claude Code use; its reliability on local Qwen is unmeasured in the sources found.

### Repo map
- Aider builds a tree-sitter map of definitions, ranks files with a graph algorithm, and fits it into `--map-tokens` (default 1k) (https://aider.chat/2023/10/22/repomap.html). The local test above turned the map off (`use_repo_map: false`) for qwen3-coder. No ablation numbers found.

### Tool output truncation
- pi: 2000 lines or 50 KB, tail for bash, head for read, full output in a temp file.
- opencode: 2000 lines or 50 KB, full output saved to disk, model gets a head and tail preview (secondary sources: https://gh.hhj520.top/anomalyco/opencode/pull/9693, search results; unverified against source).
- pi compaction cuts serialized tool results to 2000 characters inside the summarizer input.

### Compaction and context editing
- Anthropic context editing (`clear_tool_uses_20250919`): clears old tool results past a trigger (the docs example uses 30,000 input tokens, keep 3 tool uses). Reported: 84% fewer tokens in a 100 turn web search evaluation, 29% better than baseline alone, 39% with the memory tool, on an internal agentic search evaluation with no task count or model stated (https://claude.com/blog/context-management, https://docs.claude.com/en/docs/build-with-claude/context-editing).
- Observation masking versus LLM summarization (JetBrains and TUM), SWE-agent on SWE-bench Verified, five model configurations: masking halves cost versus the raw agent and matches or slightly exceeds the solve rate of LLM summarization. A hybrid cuts cost a further 7% (versus masking) and 11% (versus summarization). On OpenHands the SWE-agent masking window degraded results until retuned, tested on 50 instances only. Example: Qwen3 Coder 480B 53.8% to 54.8% with masking (secondary summary) (https://arxiv.org/abs/2508.21433).
- opencode prunes old tool outputs in place to "[Old tool result content cleared]" with constants PRUNE_MINIMUM 20,000 and PRUNE_PROTECT 40,000 (secondary community deep dive, unverified against source) (https://forums.basehub.com/anomalyco/opencode/20).
- Claude Code auto compact threshold: sources disagree (about 167k on a 200k window by one reverse engineered account, about 95% by another, buffer 45K reduced to 33K by a third). Not from official docs; unverified (https://www.morphllm.com/claude-code-auto-compact).
- OpenHands LLMSummarizingCondenser details: not obtained from a primary source (unverified).

### Prompt cache preservation
- Anthropic and OpenAI caching rules (stable prefix, append only) are well documented; for llama.cpp see section 4. Context editing that rewrites old tool results invalidates the cache from the first changed token, so clear in large batches (Anthropic's `clear_at_least` option exists for this reason; https://docs.claude.com/en/docs/build-with-claude/context-editing).
- Qwen3.6 template option `preserve_thinking: true` keeps thinking from earlier turns; the model card says this "can reduce token usage and improve KV cache use" in agent workflows (https://huggingface.co/Qwen/Qwen3.6-27B). By default only the latest user turn's thinking is kept, so the prompt prefix changes between turns, which breaks cache reuse. This matters directly for Umbra.

### System prompt and tool schema size
- pi under 1000 tokens combined (author claim). Playwright MCP 21 tools at 13.7k tokens (same blog). mini-swe-agent has bash only and no tool calling interface, about 100 lines of agent code, and "scores >74%" on SWE-bench Verified, with models unnamed on the page (https://github.com/SWE-agent/mini-swe-agent). Terminal-Bench's Terminus 2 uses only a tmux session.
- llama.cpp function calling guide: generic tool format "may consume more tokens and be less efficient"; "Beware of extreme KV quantizations" such as `-ctk q4_0` can hurt tool calling (https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md).

### Loop and stall detection
- Gemini CLI has a `LoopDetectionService` with a tool call repeat threshold of 5 (secondary: https://tessl.io/registry/tessl/npm-google--gemini-cli-core/files/docs/services.md and a third party crate). Unverified against source.
- Goose repetition inspector, opencode doom loop detection: no primary source found. Unverified.
- Pi: none documented.

### Retry on malformed tool calls, verification before finishing
- No primary source with numbers found for either. Unverified. mini-swe-agent's design (one bash command per turn, format errors returned to the model) is described in its docs but the page read did not cover it (unverified).

---

## 4. Qwen on llama.cpp

### Tool call parsing
- OpenAI style tool calls require `--jinja` (enabled by default in current llama-server, `--no-jinja` disables it). The server needs a tool aware template; check `/props`. `--chat-template-file` overrides. Generic handler is the fallback and costs more tokens. Parallel tool calls are off by default (`"parallel_tool_calls": true` in the request) (https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md, https://raw.githubusercontent.com/ggml-org/llama.cpp/master/tools/server/README.md).
- Qwen2.5 and Qwen2.5 Coder are native formats; many Qwen templates use the Hermes 2 Pro handler (function-calling.md).
- Qwen3 Coder uses an XML style tool call body. PR #16932 added a generalized XML tool call parser covering Qwen3 Coder (https://gh.hhj520.top/ggml-org/llama.cpp/pull/16932). Open reports: invalid JSON tool calls for Qwen3 Coder Next when writing files (https://github.com/ggml-org/llama.cpp/issues/19382); text before `<tool_call>` failing the peg native parser (issue #20260, reported in a search snippet); Qwen3 Instruct 2507 misdetected as a thinking model so tool calls land in `reasoning_content` (https://github.com/ggml-org/llama.cpp/issues/20809). Status of each depends on the build. Unverified whether fixed in the version Umbra uses.
- A user report: with opencode and Qwen, "more than half the tool calls failed" in an earlier attempt, anecdotal (https://blob42.xyz/post/building-a-local-agent-stack-with-llama.cpp-and-pi.dev/).

### Thinking
- Qwen3.6 27B: thinking on by default; disable with `chat_template_kwargs: {"enable_thinking": false}`. The Qwen3 `/think` and `/no_think` soft switches are not supported on this model (https://huggingface.co/Qwen/Qwen3.6-27B). llama.cpp: `--chat-template-kwargs '{"enable_thinking":false}'`, `--reasoning-budget N` (-1 unlimited, 0 ends thinking immediately), `--reasoning-format` (server README).
- Local test: models with thinking hit a 300 second timeout with zero turns because reasoning used the budget (lostechies post above).

### Sampling and limits (Qwen3.6 27B model card)
- Thinking, precise coding: temperature 0.6, top_p 0.95, top_k 20, min_p 0, presence_penalty 0, repetition_penalty 1.0.
- Thinking, general: temperature 1.0, top_p 0.95, top_k 20.
- Non thinking: temperature 0.7, top_p 0.8, top_k 20, min_p 0, presence_penalty 1.5.
- Context 262,144 native. The card advises keeping at least 128K tokens to preserve thinking capability. Output 32,768 for most queries, 81,920 for hard programming.
- Local servers rarely run 128K; a smaller window with thinking is a risk the card does not quantify.

### Prompt cache behavior
- `cache_prompt` defaults to true. A request reuses a slot's KV cache for the longest matching prefix and reprocesses from the first differing token. `--slot-prompt-similarity` default 0.10 picks the slot. `-np` sets the slot count (default auto), and the context is shared across slots. `--cache-reuse N` (default 0) lets the server shift and reuse chunks after a changed region. `--ctx-checkpoints` default 32 per slot. Slots can be saved and restored with `--slot-save-path` and `POST /slots/{id}?action=save|restore|erase`. Responses report `timings.cache_n` (tokens reused) and `usage.prompt_tokens_details.cached_tokens` (server README).
- Qwen3.5 and Qwen3.6 are hybrid models (Gated DeltaNet plus attention). The recurrent state cannot be trimmed to an arbitrary prefix, so a changed prefix can force full prompt reprocessing; checkpoints mitigate but did not fully fix it in 2026 builds. Issue #22384 reports full reprocessing every turn on Qwen3.5 27B and Qwen3.6 27B and a fix that cut a turn 2 prefill from about 12,146 tokens to 31; a later build (b9310) reportedly reduced reprocessing; other reports show hangs (https://github.com/ggml-org/llama.cpp/issues/22384, https://github.com/ggml-org/llama.cpp/issues/22450). One report attributed a cache miss to a changing attribution header in the client's system prompt (https://zenn.dev/syoyo/articles/188508eb6ab389). Pi's llama.cpp doc recommends `--ctx-checkpoints 32 --checkpoint-min-step 0` for hybrid models (unverified because that doc page was garbled).
- Design rule that follows: keep the system prompt and tool list byte identical across turns, never edit earlier messages in place, put volatile data (date, git status) at the end or not at all, compact in rare large steps.

---

## 5. How harnesses are evaluated

- Terminal-Bench 2.0 with Harbor: official harness, Docker per task, `harbor run -d terminal-bench@2.0 --agent-import-path ... -m <model> -n <concurrency>`; start with 10 to 20 tasks. Leaderboard columns: resolution rate with a 95% confidence interval, cost, tokens (https://www.tbench.ai/leaderboard/terminal-bench/2.0, https://github.com/badlogic/pi-terminal-bench, https://mintlify.com/harbor-framework/harbor/examples/terminal-bench). One survey puts open weight models around 15 to 36% (secondary: https://arxiv.org/pdf/2606.20683, unverified). Harbor needs Docker; on Windows this means WSL.
- SWE-bench Verified Mini: random 50 task subset with a similar difficulty distribution, 5 GB instead of 130 GB; EvalScope can run it against any OpenAI compatible endpoint, and has an agentic mode mirroring the mini-swe-agent config with a "backticks" text protocol for models without function calling (https://hal.cs.princeton.edu/swebench_verified_mini, https://evalscope.readthedocs.io/en/latest/benchmarks/swe_bench_verified_mini_agentic.html). Noted as noisier than the full set.
- mini-swe-agent: bash only harness used for a fixed model comparison on SWE-bench Verified (https://github.com/SWE-agent/mini-swe-agent).
- Aider polyglot: 225 exercises, two attempts, reports pass rate on attempt 2, percent well formed edits, cost (https://aider.chat/docs/leaderboards/). Runs per exercise are fast with a local model, and the benchmark measures edit format reliability directly.
- Cline publishes open evals for open weight agents (https://cline.bot/blog/open-sourcing-evals-for-open-weight-agents); not read in detail (unverified content).
- Metrics reported across these sources: pass or resolution rate with confidence interval, total tokens, cost, edit well formedness, wall time per task, number of turns. Cached tokens are reported by pi (`cacheRead`) and by llama.cpp (`cache_n`).
- Practical advice from sources: rerun to rule out flakiness (Cline post); pass rate varies with API provider settings and quantization, so use the same server for both harnesses (https://aider.chat/2025/05/08/qwen3.html).
- Suggested local suite (my recommendation, not from a source): 15 to 25 small tasks, each with a pytest or script verifier, run three times per harness, against one llama-server instance with `--parallel 1`, recording pass rate, input, output and cached tokens per turn, turn count and wall time. Per run log both the harness's own usage events and the llama.cpp `timings` for cross checking.

---

## Items I could not verify from a primary source
- Exact token count of pi's current system prompt and tool schemas.
- Pi's Terminal-Bench 2.0 numbers (blog shows images only; leaderboard fetch returned no rows).
- Whether pi keeps the system prompt byte stable between turns, and the exact meaning of `codemode` and `context_edit`.
- Which `turn_end` field carries per turn usage in `--mode json` (assistant message `usage` is documented; confirm with one real run).
- Whether `--mode json` takes the prompt as a positional argument.
- Pi's handling of malformed tool call arguments and of models without tool calling.
- OpenHands condenser internals, Goose and opencode loop detection, Claude Code compaction threshold, Crush behavior (not researched), opencode prune constants.
- SWE-agent linter ablation number against the paper tables.
