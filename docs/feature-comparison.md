# Feature comparison

This page compares what Umbra 0.8.0 does with four other coding agents, feature by feature. The
Umbra column is what this release does, checked against the code and the tests. The other columns
come from each project's own documentation as of November 2026. The last column, pi 1.3.0, holds only
what the suite in `evals/` measured, and says `not measured` for every other row.

| Feature | Umbra | mini-swe-agent | Claude Code | Codex CLI | opencode | pi 1.3.0 |
|---|---|---|---|---|---|---|
| Native tool calling | yes | yes | yes | yes | yes | not measured |
| Text actions for models without tool calling | yes (`--text`) | yes | no | no | no | not measured |
| The shell keeps its folder and variables between commands | yes, by default (`environment.persistent_shell`) | no | yes | yes | yes | not measured |
| Dedicated file read and edit tools | no, the model uses bash | no | yes | yes | yes | not measured |
| Run commands in Docker | yes (`--docker`) | yes | sandboxing | sandboxing | no | not measured |
| Confirm before each command | yes | yes | yes | yes | yes | not measured |
| Human mode (you type the commands) | yes | yes | no | no | no | not measured |
| Step, cost and time limits | yes | yes | turn limit | not documented | not documented | not measured |
| Time limit for one command | yes, 120 seconds, and the model can ask for up to 600 | not documented here | not documented here | not documented here | not documented here | no limit on a command: one reference run was lost to a hung `find /` (ledger E9) |
| Cost tracking | yes | yes | yes | yes | yes | not measured |
| Saved record of every run | yes, a journal, and a trajectory JSON in mini-swe-agent format from `umbra export` or `-o` | yes | transcripts | session logs | yes | not measured |
| Reads mini-swe-agent YAML configs | yes | yes | no | no | no | not measured |
| Anthropic models | yes | yes, through LiteLLM | yes | no | yes | not measured |
| OpenAI compatible servers (llama.cpp, Ollama, vLLM) | yes | yes, through LiteLLM | no | yes | yes | not measured |
| Prompt caching | yes, Anthropic | yes | yes | yes | yes | cache hit rate 82.4% in the 61 task run |
| Retries with backoff | yes | yes | yes | yes | yes | not measured |
| CLAUDE.md, AGENTS.md, GEMINI.md and rule files | yes, all of them | no | CLAUDE.md, AGENTS.md | AGENTS.md | AGENTS.md, CLAUDE.md | not measured |
| `@path` imports in instruction files | yes | no | yes | no | no | not measured |
| Agent Skills (SKILL.md) | yes, from every tool's folders | no | yes | yes | yes | not measured |
| Custom slash commands | yes, from every tool's folders | no | yes | yes | yes | not measured |
| MCP over stdio | yes | no | yes | yes | yes | not measured |
| MCP over HTTP and SSE | yes | no | yes | yes | yes | not measured |
| MCP 2026-07-28 (no handshake) | yes | no | not documented | not documented | optional | not measured |
| MCP config from other tools' files | yes | no | no | no | no | not measured |
| MCP OAuth sign in | no | no | yes | yes | yes | not measured |
| Rules that apply only to some file paths | no, only rules for every file | no | yes | no | no | not measured |
| Context compaction | yes: mask old outputs, then summarize | no | yes | yes | yes | compaction tasks passed in 44.4% of runs |
| Subagents | no | no | yes | yes | yes | not measured |
| Hooks | no | no | yes | yes | yes | not measured |
| Plan mode | no | no | yes | not documented | yes | not measured |
| Todo list tool | no | no | yes | yes | yes | not measured |
| Web fetch and web search | no | no | yes | yes | yes | not measured |
| Image input | yes (`-i`, typed paths, MCP images) | optional | yes | yes | yes | not measured |
| Paste an image from the clipboard | no | no | yes | yes | yes | not measured |
| Resume a past session | yes (`--resume`, `--continue`, `--fork`, `--at`) | no | yes | yes | yes | session tasks passed in 95.2% of runs |
| Ctrl+C stops the current step and asks for a comment | yes | yes | yes, with Esc | yes, with Esc | yes, with Esc | not measured |
| Full screen terminal interface | yes | no | yes | yes | yes | not measured |
| Agent Client Protocol, for editors | yes (`umbra acp`) | no | through an adapter | through an adapter | yes | not measured |
| Git checkpoints and undo | no | no | yes | not documented | yes | not measured |

## Why some features are left out

The rows marked "no" for Umbra are left out on purpose, to keep the loop as small and as easy to
follow as mini-swe-agent's. An MCP server can supply several of them, such as web search or a
browser.

| Left out | What to use instead |
|---|---|
| Dedicated file read and edit tools | The model reads with `cat` and `sed -n`, and edits with `sed` or a heredoc. Each edit is a command you can read and run again |
| Subagents | Run a second Umbra in another terminal, or have the agent run `umbra -q -t "<task>" --exit-immediately` as a command |
| Hooks | `whitelist_actions` for commands that may run without asking, and confirm mode for the rest |
| Plan mode and a todo list tool | Ask for a plan in the task. The model writes it in its reply and the turn ends, so you can answer it |
| Web fetch and web search | An MCP server that offers them |
| Rules that apply only to some file paths | Umbra reads the rule files that apply to every file. See [Where files are found](discovery.md#rules-for-instruction-files) |
| MCP OAuth sign in | A server that takes a token in a header, such as `headers = { Authorization = "Bearer ${TOKEN}" }` |

## How Umbra measures against another agent

The eval suite in `evals/` gives Umbra and pi the same 61 tasks on the same local model and grades
each result with a hidden verifier. [The eval status](../evals/STATUS.md) has the current numbers,
the metrics where Umbra is ahead and the ones where it is not yet, and [the ledger](../evals/LEDGER.md)
records every experiment.
