# Umbra

Umbra is a coding agent for the terminal, written in Rust. You give it a task, it asks a model what
to do, runs the commands the model asks for, shows the model what happened, and repeats until the
model replies without a command. Your next message continues the same conversation.

It works with any model: Claude, OpenAI, OpenRouter, or a local model on llama.cpp, Ollama or vLLM.
It reads the instruction files, skills, slash commands and MCP servers you already set up for Claude
Code, Codex, Gemini CLI, opencode, Cursor, Copilot, Windsurf and Cline, so it knows your project on
the first run. It shows every command before it runs, lets you steer it while it works, and saves
every session so it can be picked up later, in the terminal or in an editor that speaks the
[Agent Client Protocol](https://agentclientprotocol.com).

```text
$ umbra -m anthropic/claude-haiku-5-5 "two tests fail. find out why and fix it"
```

**Documentation:** [unluminous.com/umbra/docs](https://unluminous.com/umbra/docs) has every
chapter with pictures and clips of the real program. The reference pages are in [docs/](docs/README.md).

## Install

**macOS and Linux**

```sh
curl -fsSL https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.sh | sh
```

**Windows**, in PowerShell or Command Prompt

```bat
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.ps1 | iex"
```

Both scripts download the build for your machine from the
[latest GitHub release](https://github.com/Black-Rainbow-Labs/umbra/releases/latest), check it
against the release's `SHA256SUMS`, and install it into your home folder (`~/.local/bin`, or
`%LOCALAPPDATA%\Programs\umbra` on Windows). Neither needs administrator rights. On Windows, Umbra
runs commands with Git Bash, so install [Git for Windows](https://gitforwindows.org) too.

| Other ways | Command |
|---|---|
| Homebrew, macOS and Linux | `brew install black-rainbow-labs/umbra/umbra` |
| cargo, from source | `cargo install --git https://github.com/Black-Rainbow-Labs/umbra --locked` |
| A download | the `.tar.gz` or `.zip` for your machine from the [releases page](https://github.com/Black-Rainbow-Labs/umbra/releases/latest) |
| An editor | pick Umbra from the editor's ACP agent list. See [Editors and ACP](docs/acp.md) |

**To have a coding agent install it for you**, paste this to the agent:

```text
Install Umbra, the coding agent from https://github.com/Black-Rainbow-Labs/umbra.
On macOS or Linux run: curl -fsSL https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.sh | sh
On Windows run in PowerShell: irm https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.ps1 | iex
On Windows, check that Git for Windows is installed, because Umbra runs commands with Git Bash.
Then run `umbra --version` to check it is on the PATH, and run
`umbra setup --model anthropic/claude-haiku-5-5 --no-test` (or another provider/model I name) to save the model.
To add it to an editor that speaks ACP, the command is `umbra` with the argument `acp`.
```

## Quick start

```bash
umbra setup                                    # choose a provider and model, saved in ~/.umbra/config.toml
umbra "create sum.py that prints the sum of 1 to 100, run it, and tell me the result"
umbra -m local/qwen "create sum.py that prints the sum of 1 to 100"   # a local model on llama-server, port 8080
```

If you are signed in to Claude Code, Umbra uses that login for Claude. Otherwise set
`ANTHROPIC_API_KEY`, or the key of the provider you chose.

Umbra shows each command before it runs and waits for Enter. Type a reason instead to refuse it, or
`/y` to run the rest without asking. Add `-y` to start in that mode. [Getting started](docs/getting-started.md)
walks through a first session.

See what Umbra found on your machine before you run anything:

```bash
umbra instructions    # instruction files, in the order the model reads them
umbra skills          # every skill and where it came from
umbra mcp --connect   # MCP servers and their tools
umbra prompt "fix the failing test"   # the exact prompt a task would get
```

## What it does

| | |
|---|---|
| **One tool, one list** | The model has one tool, `bash`, and every command runs in a new shell. The whole conversation is one list of messages, saved as a trajectory file in mini-swe-agent's format. [How it works](docs/how-it-works.md) |
| **Your setup** | `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, Cursor, Copilot, Windsurf and Cline rules, skills from every tool's folders, slash commands, and MCP servers from `.mcp.json`, Claude Desktop, Codex, Gemini, opencode, Cursor and VS Code. [Where files are found](docs/discovery.md) |
| **Confirm, yolo and human** | See each command before it runs, run them all, or type them yourself. `--docker <image>` runs every command in a container. [Modes](docs/modes.md) |
| **The fullscreen view** | A transcript with collapsed command groups and syntax colors, an input line that is always there, pickers, an MCP panel, a configuration modal, and a status bar with the context use, agent time, cost and weekly usage. [The terminal view](docs/terminal.md) |
| **Ctrl+C and steering** | Ctrl+C stops the command or model call and asks for a comment. A line typed while the agent works reaches it before its next call. [Sessions, Ctrl+C and images](docs/sessions.md) |
| **Sessions** | Saved after every step. `umbra resume` opens a picker with a title and a summary for each, and `--fork` continues a copy. [Sessions](docs/sessions.md#sessions) |
| **Long sessions** | Near 80% of the context window, old outputs are masked, then the middle is summarized. Large MCP tool sets are sent by name behind `tool_search`. [Architecture](docs/architecture.md#compaction) |
| **Images** | `-i shot.png`, an image path typed in a task, and images an MCP tool returns. [Images](docs/sessions.md#images) |
| **Editors** | `umbra acp` runs in Zed, the JetBrains IDEs and Unluminous, with modes, a model picker, sessions shared with the terminal, and Stop. [Editors and ACP](docs/acp.md) |
| **Its own command line** | `umbra config set`, `umbra mcp disable`, `umbra session model` and `umbra update`, which the agent can run to change its own settings and session. [Command line](docs/command-line.md) |

## Choosing a model

| Provider | Example | What it talks to | Credential |
|---|---|---|---|
| `anthropic` | `anthropic/claude-haiku-5-5` | Anthropic Messages API | `ANTHROPIC_API_KEY`, or your Claude Code login |
| `local` | `local/qwen` | `http://localhost:8080/v1` (llama.cpp) | none |
| `ollama` | `ollama/llama3` | `http://localhost:11434/v1` | none |
| `openai` | `openai/gpt-5` | OpenAI | `OPENAI_API_KEY` |
| `openrouter` | `openrouter/anthropic/claude-sonnet-5-5` | OpenRouter | `OPENROUTER_API_KEY` |

Any other OpenAI compatible server is one entry in the config. See [Models](docs/providers.md).

## What it leaves out

Subagents, hooks, plan mode, a todo list tool, web search and dedicated file editing tools are not
in this version, to keep the loop as small and easy to follow as mini-swe-agent's. An MCP server can
supply several of them. [Feature comparison](docs/feature-comparison.md) has the full table against
mini-swe-agent, Claude Code, Codex CLI and opencode.

## Working on Umbra

```bash
cargo build
cargo test
```

[Repository and building](docs/repository.md) lists what is where and what each test checks, and
[`AGENTS.md`](AGENTS.md) has the rules for a change, including the documentation every change
updates. Umbra is MIT licensed.
