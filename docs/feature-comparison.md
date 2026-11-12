# Feature comparison

This page compares what Umbra 0.7.2 does with four other coding agents, feature by feature. The
Umbra column is what this release does, checked against the code and the tests. The other columns
come from each project's own documentation as of November 2026.

| Feature | Umbra | mini-swe-agent | Claude Code | Codex CLI | opencode |
|---|---|---|---|---|---|
| Native tool calling | yes | yes | yes | yes | yes |
| Text actions for models without tool calling | yes (`--text`) | yes | no | no | no |
| Fresh shell for every command | yes | yes | no | no | no |
| Dedicated file read and edit tools | no, the model uses bash | no | yes | yes | yes |
| Run commands in Docker | yes (`--docker`) | yes | sandboxing | sandboxing | no |
| Confirm before each command | yes | yes | yes | yes | yes |
| Human mode (you type the commands) | yes | yes | no | no | no |
| Step, cost and time limits | yes | yes | turn limit | not documented | not documented |
| Cost tracking | yes | yes | yes | yes | yes |
| Trajectory file of every run | yes, mini-swe-agent format | yes | transcripts | session logs | yes |
| Reads mini-swe-agent YAML configs | yes | yes | no | no | no |
| Anthropic models | yes | yes, through LiteLLM | yes | no | yes |
| OpenAI compatible servers (llama.cpp, Ollama, vLLM) | yes | yes, through LiteLLM | no | yes | yes |
| Prompt caching | yes, Anthropic | yes | yes | yes | yes |
| Retries with backoff | yes | yes | yes | yes | yes |
| CLAUDE.md, AGENTS.md, GEMINI.md and rule files | yes, all of them | no | CLAUDE.md, AGENTS.md | AGENTS.md | AGENTS.md, CLAUDE.md |
| `@path` imports in instruction files | yes | no | yes | no | no |
| Agent Skills (SKILL.md) | yes, from every tool's folders | no | yes | yes | yes |
| Custom slash commands | yes, from every tool's folders | no | yes | yes | yes |
| MCP over stdio | yes | no | yes | yes | yes |
| MCP over HTTP and SSE | yes | no | yes | yes | yes |
| MCP 2026-07-28 (no handshake) | yes | no | not documented | not documented | optional |
| MCP config from other tools' files | yes | no | no | no | no |
| MCP OAuth sign in | no | no | yes | yes | yes |
| Rules that apply only to some file paths | no, only rules for every file | no | yes | no | no |
| Context compaction | yes: mask old outputs, then summarize | no | yes | yes | yes |
| Subagents | no | no | yes | yes | yes |
| Hooks | no | no | yes | yes | yes |
| Plan mode | no | no | yes | not documented | yes |
| Todo list tool | no | no | yes | yes | yes |
| Web fetch and web search | no | no | yes | yes | yes |
| Image input | yes (`-i`, typed paths, MCP images) | optional | yes | yes | yes |
| Paste an image from the clipboard | no | no | yes | yes | yes |
| Resume a past session | yes (`--resume`, `--continue`, `--fork`) | no | yes | yes | yes |
| Ctrl+C stops the current step and asks for a comment | yes | yes | yes, with Esc | yes, with Esc | yes, with Esc |
| Full screen terminal interface | yes | no | yes | yes | yes |
| Agent Client Protocol, for editors | yes (`umbra acp`) | no | through an adapter | through an adapter | yes |
| Git checkpoints and undo | no | no | yes | not documented | yes |

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
