# MCP

The [Model Context Protocol](https://modelcontextprotocol.io) lets a program offer tools to a
model. Umbra is an MCP client: it starts the servers it finds, asks each for its tools, offers
those tools to the model next to `bash`, and runs a tool when the model calls it.

```mermaid
sequenceDiagram
    participant U as Umbra
    participant S as MCP server
    U->>S: start the process, or open the URL
    U->>S: initialize (protocol 2025-11-25)
    alt the server answers
        S-->>U: protocol version, capabilities, instructions
        U->>S: notifications/initialized
    else the server refuses initialize
        Note over U,S: 2026-07-28 server: no handshake,<br/>metadata in every request
        U->>S: server/discover
    end
    U->>S: tools/list (every page)
    S-->>U: tools
    Note over U: tools are offered to the model as mcp__server__tool
    U->>S: tools/call when the model asks
    S-->>U: content, isError
```

## Setting up a server

You do not need to set up anything for servers you already use in Claude Code, Codex, Gemini CLI,
opencode, Cursor, VS Code or Claude Desktop: Umbra reads their files. To add a server only for
Umbra, put it in `~/.umbra/config.toml` or `<project>/.umbra/config.toml`:

```toml
[mcp_servers.files]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/me/projects"]

[mcp_servers.tickets]
type = "http"
url = "https://mcp.example.com/mcp"
headers = { Authorization = "Bearer ${TICKETS_TOKEN}" }
```

or in `<project>/.umbra/mcp.json` or `~/.umbra/mcp.json`, in Claude Code's format:

```json
{
  "mcpServers": {
    "files": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "."] }
  }
}
```

Check what was found and whether it starts:

```bash
umbra mcp             # the servers and which file each came from
umbra mcp --connect   # start them and list their tools
```

## Formats Umbra reads

| Tool | Where the servers are | Shape |
|---|---|---|
| Claude Code, Claude Desktop, Cursor, Gemini CLI, Umbra's `mcp.json` | `mcpServers` | `{"name": {"command", "args", "env"}}` or `{"type": "http", "url", "headers"}` |
| Claude Code local scope | `~/.claude.json` → `projects["/abs/path"].mcpServers` | same |
| VS Code | `servers` | same, with `type` |
| Codex | `[mcp_servers.<name>]` in TOML | `command`, `args`, `env`, `env_vars`, `cwd`, `url`, `http_headers`, `env_http_headers`, `bearer_token_env_var`, `enabled` |
| opencode | `mcp` (older layout) or `mcp.servers` (newer layout) | `{"type": "local", "command": ["program", "arg"], "environment"}` or `{"type": "remote", "url", "headers"}` |
| Umbra's config | `[mcp_servers.<name>]` | the Claude Code shape, in TOML |

How the transport is chosen:

| The entry has | Transport |
|---|---|
| `command` | stdio |
| `httpUrl` (Gemini) | Streamable HTTP |
| `url` with `type = "sse"`, or a Gemini `url` | the older HTTP+SSE transport |
| `url` with any other type | Streamable HTTP |

An entry with `"enabled": false` or `"disabled": true` is skipped.

### Variables in config values

Every string in a server entry has its variables expanded, in every syntax the other tools use:

| Syntax | Used by | Becomes |
|---|---|---|
| `${VAR}` | Claude Code, Cursor, Gemini | the variable, or empty |
| `${VAR:-default}` | Claude Code | the variable, or `default` |
| `${env:VAR}` | Cursor, VS Code | the variable |
| `{env:VAR}` | opencode | the variable |
| `${workspaceFolder}`, `${CLAUDE_PROJECT_DIR}` | VS Code, Cursor, Claude Code | the project root |
| `${userHome}` | Cursor | the home folder |

Gemini's bare `$VAR` and `%VAR%` forms and VS Code's `${input:...}` prompts are not supported.

## Transports

| Transport | How Umbra uses it |
|---|---|
| stdio | Starts the program and sends one JSON message per line. On Windows, a program that is a `.cmd` or `.bat` file (such as `npx`) runs through `cmd /c`. The server's stderr goes to `~/.umbra/logs/mcp/<name>.log`. When the run ends, Umbra waits half a second for the server to exit, then stops it. |
| Streamable HTTP | Each message is a POST with `Accept: application/json, text/event-stream`. Replies can be JSON or an event stream. Umbra keeps the `Mcp-Session-Id` the server hands out, sends `MCP-Protocol-Version` after the handshake, and sends a DELETE at the end. |
| HTTP+SSE (older) | Opens the event stream with GET, waits for the `endpoint` event, then POSTs messages to that endpoint and reads replies from the stream. |

## Protocol versions

There are two kinds of MCP server today.

- **2025-11-25 and earlier** (almost every server in use): the client sends `initialize`, then
  `notifications/initialized`, then normal requests.
- **2026-07-28**: there is no handshake. Every request carries
  `_meta.io.modelcontextprotocol/protocolVersion`, `clientInfo` and `clientCapabilities`, and HTTP
  requests also carry `Mcp-Method` and `Mcp-Name` headers.

Umbra sends `initialize` first. If the server answers with a JSON-RPC error, Umbra treats it as a
2026-07-28 server from then on and adds the metadata to every request.

## Tool names and results

The model sees each tool as `mcp__<server>__<tool>`, the convention Claude Code uses, with any
character other than letters, digits, `_` and `-` replaced by `_` and the name cut to 64
characters. In text mode the model writes:

````text
```umbra_mcp
{"tool": "mcp__files__read_file", "arguments": {"path": "README.md"}}
```
````

A result's text items are joined into the output the model sees. Images and audio are described
by type and size instead of being pasted in. A result with `isError: true` is shown with return
code 1, so the model knows the call failed and can try again.

The text a server sends in `instructions` during the handshake is added to the system prompt.

## Limits and failures

| Situation | What happens |
|---|---|
| A server does not start or does not answer within 30 seconds | It is listed as failed on stderr, and the run goes on without it. `mcp.startup_timeout_seconds` changes the limit. |
| A tool call takes more than 120 seconds | The model sees an error. On stdio Umbra also sends `notifications/cancelled`. `mcp.call_timeout_seconds` changes the limit. |
| The server sends a request to Umbra | `ping` gets an empty result. Anything else gets "method not found", because Umbra offers no client features such as sampling. |
| The server prints text before a message on the same line | The message is still read. |

## Not supported

OAuth sign in for remote servers, resources, prompts, sampling, elicitation, roots, and
`notifications/tools/list_changed` (the tool list is read once at the start of a run).
