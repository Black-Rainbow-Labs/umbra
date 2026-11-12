# Editors and the Agent Client Protocol

`umbra acp` runs Umbra as an agent for an editor that speaks the
[Agent Client Protocol](https://agentclientprotocol.com) (ACP). The editor starts the program and
sends JSON-RPC 2.0 messages on its stdin, one per line. Umbra answers on stdout and writes its own
log lines to stderr. Nothing else is ever written to stdout.

The design, and the reasons for it, are in
[the ACP and distribution design](../tasks/umbra-acp-distribution-tdd.md).

## Adding Umbra to an editor

| Editor | Setting |
|---|---|
| Zed | `settings.json`: `"agent_servers": { "Umbra": { "type": "custom", "command": "umbra", "args": ["acp"] } }` |
| JetBrains IDEs | `~/.jetbrains/acp.json`: `{ "agent_servers": { "Umbra": { "command": "umbra", "args": ["acp"] } } }` |
| Unluminous | `settings acp-add umbra` adds it from the ACP registry. Or add an endpoint with wire `acp`, command `umbra` and the argument `acp`. |
| Others | The command `umbra`, the argument `acp`. |

Use the full path to `umbra` if the editor does not see the same `PATH` as your shell, which is
common for an editor started from the macOS Dock.

The options of `umbra acp` go after `acp`:

| Option | Meaning |
|---|---|
| `-m provider/model` | The model, instead of the one in `~/.umbra/config.toml` |
| `--yolo`, `--mode confirm\|yolo` | The mode a new session starts in. The default is `confirm`, or the config's `mode`. |
| `-l <dollars>`, `--step-limit <n>` | Limits for each turn |
| `-c <file>`, `--no-mcp`, `--mcp a,b`, `--no-skills`, `--no-instructions`, `--no-source` | The same as for a terminal run |
| `--setup` | Runs `umbra setup` in a terminal instead of serving ACP |

## Choosing a model

A session needs a model and a credential for it. Umbra looks for them the way a terminal run does:
`-m`, then `model` in `~/.umbra/config.toml` or the project's `.umbra/config.toml`, then
`UMBRA_MODEL`. When there is none, `session/new` answers error `-32000`, which the editor shows as
a request to sign in. The sign in methods Umbra offers are:

| Method | Type | What it does |
|---|---|---|
| `umbra-config` | `agent` | Checks the configured model again, after you set it yourself |
| `umbra-setup` | `terminal` | The editor runs `umbra acp --setup` in a terminal: pick a provider and model, and it is saved in `~/.umbra/config.toml` |

`umbra-setup` is offered only to an editor that says it can run a terminal sign in
(`clientCapabilities.auth.terminal`).

The editor's model picker lists the session's model and every `provider/model` in the config's
`models` list:

```toml
model = "anthropic/claude-sonnet-5-5"
models = ["anthropic/claude-haiku-5-5", "local/qwen", "openrouter/openai/gpt-5"]
```

## What a turn looks like to the editor

Each `session/prompt` is one turn. Umbra runs the agent loop until the model replies without asking
for a command, a limit is reached, or the editor cancels. With `finish = "submit"` the turn ends
when a command prints the submit line instead.

| What happens | What Umbra sends |
|---|---|
| The first prompt of a session | `session_info_update` with the title |
| The model replies | `usage_update` (`used`, `size`, `cost`), then `agent_message_chunk` with its text |
| The model asks for a command | `tool_call` with `kind: execute`, the command as `title` and `rawInput.command` |
| The model asks for an MCP tool | `tool_call` with `kind: other` and `server.tool` as `title` |
| Confirm mode | `session/request_permission` for each action, with Allow, Always allow and Reject |
| The action starts | `tool_call_update` with `status: in_progress` |
| The action ends | `tool_call_update` with `completed` or `failed`, the output (at most 20,000 characters) and `rawOutput` |
| An action is not run | `tool_call_update` with `failed` and the reason |
| A compaction, a format error | `notice` when the editor supports notices, otherwise `agent_thought_chunk` |
| The model replies with no command | The reply has already gone as `agent_message_chunk`, and the `session/prompt` answer is `end_turn` |
| A command prints the submit line (`finish = "submit"`) | `agent_message_chunk` with the submitted output, then `end_turn` |

| How the turn ended | `stopReason` |
|---|---|
| The model replied, or a command submitted | `end_turn` |
| The step, cost or time limit | `max_turn_requests` |
| `session/cancel` | `cancelled` |
| The model's replies could not be read three times in a row | `end_turn`, with a message saying so |
| The model call failed | an error answer, `-32603`, with the provider's message |

**Always allow** remembers the program the command starts (its first word, after any
`cd folder &&`), or the MCP tool, for the rest of the session. `whitelist_actions` in the config
applies too.

**Cancel** stops the running command and everything it started, or the model call, the same way
Ctrl+C does in a terminal. The next prompt carries on in the same session.

## Sessions

Every ACP session is an ordinary Umbra session, saved after every step in
`~/.umbra/trajectories/` and listed in `~/.umbra/sessions/`.

| Request | What Umbra does |
|---|---|
| `session/new` | Starts a session in `cwd`. Instructions, skills, commands and MCP servers are found for that folder, and the editor's `mcpServers` are added (an editor server replaces a found one of the same name). |
| `session/load` | Opens a saved session and sends its history: your messages, the model's text, each action with its output, each final answer |
| `session/resume` | Opens a saved session without sending the history |
| `session/list` | The saved sessions, for `cwd` when given, with the first task as the title |
| `session/close` | Cancels the session's turn and stops its MCP servers |
| `session/set_mode`, `session/set_config_option` | Change the mode (`confirm`, `yolo`) or the model |

A session started in an editor can be continued in a terminal with `umbra --resume <id>`, and the
other way round.

## What Umbra does not use

Umbra runs every command itself in a fresh shell, and edits files with those commands. It does not
use the editor's file system methods (`fs/read_text_file`, `fs/write_text_file`) or its terminals
(`terminal/create`). It sends no `plan` updates, because it keeps no todo list, and a model reply is
sent whole, when it arrives.

## Trying it by hand

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}' | umbra acp
```

The integration tests in [tests/acp.rs](../tests/acp.rs) drive every request above against the
real binary with a scripted model.
