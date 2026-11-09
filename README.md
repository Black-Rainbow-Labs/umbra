# Umbra

Umbra is a small coding agent for the terminal, written in Rust. You give it a task, it asks a
model what to do, runs the commands the model asks for, shows the model what happened, and repeats
until the model says it is finished.

Its loop is a port of [mini-swe-agent](https://github.com/swe-agent/mini-swe-agent): one shell
tool, a new shell for every command, and one plain list of messages. Umbra adds the things a coding
agent needs on a real machine. It reads the instruction files, skills, slash commands and MCP
servers you already set up for Claude Code, Codex, Gemini CLI, opencode, Cursor, Copilot, Windsurf
and Cline, so it works with your existing setup on the first run.

```text
$ umbra -m anthropic/claude-haiku-5-5 "add a --verbose flag to the CLI and test it"
```

## Contents

- [Quick start](#quick-start)
- [How it works](#how-it-works)
- [Choosing a model](#choosing-a-model)
- [What Umbra reads from other tools](#what-umbra-reads-from-other-tools)
- [Modes: confirm, yolo and human](#modes-confirm-yolo-and-human)
- [Ctrl+C, images and resuming a session](#ctrlc-images-and-resuming-a-session)
- [Features: what is included and what is not](#features-what-is-included-and-what-is-not)
- [Commands](#commands)
- [Project layout](#project-layout)
- More detail: [architecture](docs/architecture.md), [configuration](docs/configuration.md),
  [where files are found](docs/discovery.md), [MCP](docs/mcp.md), [models](docs/providers.md),
  [design document](tasks/umbra-harness-tdd.md), [compaction design](tasks/umbra-compaction-tdd.md),
  [images, resume and Ctrl+C design](tasks/umbra-images-resume-interrupt-tdd.md)

## Quick start

You need Rust 1.85 or newer. On Windows you also need [Git for Windows](https://gitforwindows.org),
because Umbra runs commands with its `bash.exe`.

```bash
cargo install --path .
```

Run a task with Claude. If you are signed in to Claude Code, Umbra uses that login. Otherwise set
`ANTHROPIC_API_KEY`.

```bash
umbra -m anthropic/claude-haiku-5-5 "create sum.py that prints the sum of 1 to 100, run it, and tell me the result"
```

Run the same task with a local model served by llama.cpp's `llama-server` on port 8080:

```bash
umbra -m local/qwen "create sum.py that prints the sum of 1 to 100, run it, and tell me the result"
```

Umbra shows each command before it runs and waits for you to press Enter. Add `-y` to run
commands without asking. Each run's full record is saved to `~/.umbra/trajectories/`.

See what Umbra found on your machine before you run anything:

```bash
umbra instructions    # instruction files, in the order the model reads them
umbra skills          # every skill and where it came from
umbra mcp --connect   # MCP servers and their tools
umbra prompt "fix the failing test"   # the exact prompt a task would get
```

## How it works

```mermaid
flowchart TD
    T[Your task] --> P[Build the first two messages:<br/>system prompt and task]
    P --> Q[Ask the model]
    Q --> R{Did the reply ask for an action<br/>in the right format?}
    R -- no --> E[Tell the model what was wrong] --> Q
    R -- yes --> C{Confirm mode?}
    C -- yes --> Y{You press Enter?}
    Y -- no, you type a reason --> X[Tell the model the command was refused] --> Q
    Y -- yes --> RUN
    C -- no --> RUN[Run each command in a new shell,<br/>or call the MCP tool]
    RUN --> D{Did a command print<br/>COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT?}
    D -- no --> O[Add the output to the messages] --> Q
    D -- yes --> F[Finished. Everything printed after<br/>that line is the final answer]
```

A few rules shape everything else:

- **The model has one tool: `bash`.** It reads files with `cat`, edits them with `sed` or a
  heredoc, and runs tests the same way you would. MCP tools are added next to it.
- **Every command runs in a new shell.** `cd` and `export` do not carry over to the next command,
  so the model writes `cd src && make` in one command. This keeps each step simple to repeat.
- **The history is one list.** What you see in the trajectory file is exactly what the model saw.
- **Long runs are compacted.** Near 80% of the context window, old command outputs are replaced
  by a short placeholder, and if that is not enough the model summarizes the middle of the session.
  Every compaction is recorded in the trajectory with the messages it removed. See
  [compaction](docs/architecture.md#compaction).
- **The run ends** when a command prints `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT` as its first line
  and exits with code 0, or when a step limit, cost limit or time limit is reached.

There are two ways for the model to ask for a command:

| | Tool mode (the default) | Text mode (`--text`) |
|---|---|---|
| How the model asks | A native tool call: `bash` with a `command` | A fenced block: ```` ```umbra_bash ```` |
| Best for | Current models with tool calling | Any model, including ones without tool calling |
| Commands per reply | One or more | Exactly one |

## Choosing a model

Pass `-m provider/model`, or set `model` in `~/.umbra/config.toml`.

| Provider | Example | What it talks to | Credential |
|---|---|---|---|
| `anthropic` | `anthropic/claude-haiku-5-5` | Anthropic Messages API | `ANTHROPIC_API_KEY`, or your Claude Code login |
| `local` | `local/qwen` | `http://localhost:8080/v1` (llama.cpp) | none |
| `ollama` | `ollama/llama3` | `http://localhost:11434/v1` | none |
| `openai` | `openai/gpt-5` | OpenAI | `OPENAI_API_KEY` |
| `openrouter` | `openrouter/anthropic/claude-sonnet-5-5` | OpenRouter | `OPENROUTER_API_KEY` |

Any other OpenAI compatible server can be added in the config. See [models](docs/providers.md).

## What Umbra reads from other tools

Umbra looks in the places each tool keeps its files. You can see the result with the listing
commands above, and turn any tool off with `--no-source <name>`.

```mermaid
flowchart LR
    subgraph User level
      U1[~/.claude] --- U2[~/.codex] --- U3[~/.gemini] --- U4[~/.config/opencode] --- U5[~/.agents] --- U6[~/.cursor] --- U7[Claude Desktop]
    end
    subgraph Project level
      P1[CLAUDE.md, AGENTS.md, GEMINI.md] --- P2[.claude, .cursor, .github, .windsurf, .clinerules] --- P3[.mcp.json, opencode.json, .vscode/mcp.json]
    end
    U1 & U2 & U3 & U4 & U5 & U6 & U7 --> M[Umbra]
    P1 & P2 & P3 --> M
    M --> I[Instructions in the system prompt]
    M --> S[Skill list in the system prompt]
    M --> C[Slash commands for the task]
    M --> T[MCP tools next to bash]
```

| What | Examples of where Umbra looks | How it is used |
|---|---|---|
| Instruction files | `~/.claude/CLAUDE.md`, `AGENTS.md`, `CLAUDE.md`, `CLAUDE.local.md`, `GEMINI.md`, `.cursor/rules/*.mdc`, `.github/copilot-instructions.md`, `.windsurfrules`, `.clinerules` | Added to the system prompt, most general first. `@path` imports are followed. |
| Skills | `~/.claude/skills`, installed Claude Code plugins, `~/.codex/skills`, `~/.agents/skills`, `.claude/skills`, `.agents/skills`, `.opencode/skills` | The name, description and path go into the system prompt. The model reads the SKILL.md when a task matches. |
| Slash commands | `.claude/commands/*.md`, `~/.codex/prompts/*.md`, `.gemini/commands/*.toml`, opencode's `command/*.md` | A task that starts with `/name` is replaced by that command's prompt. `/skill-name` works too. |
| MCP servers | `~/.claude.json`, `.mcp.json`, Claude Desktop, `~/.codex/config.toml`, `~/.gemini/settings.json`, `opencode.json`, `.cursor/mcp.json`, `.vscode/mcp.json` | Started when the run begins. Their tools are offered to the model. |

The full list, the order, and the rules for duplicates are in [where files are found](docs/discovery.md).

## Modes: confirm, yolo and human

| Mode | Flag | What happens |
|---|---|---|
| Confirm | default | Umbra shows each command and waits. Press Enter to run it, or type a reason to refuse it; the reason is sent to the model. |
| Yolo | `-y` | Commands run without asking. Use it in a container (`--docker <image>`) or a throwaway checkout. |
| Human | `--mode human` | You type the commands. The model sees them when you switch back with `/c` or `/y`. |

At the confirm prompt, `/y` switches to yolo, `/u` to human mode and `/h` shows help. When the
agent finishes in a terminal, Umbra asks for a follow up task; `--exit-immediately` turns that off.
At that prompt, `/compact` or `/compact <what to keep>` summarizes the session before the next task.
If stdin is not a terminal, Umbra refuses to start in confirm mode and asks for `-y`, so it never
waits for an answer nobody can give.

## Ctrl+C, images and resuming a session

**Ctrl+C** stops what Umbra is doing and asks what to do next, the way mini-swe-agent does:

```text
Interrupted. Type a comment/command (/h for available commands)
>
```

| Umbra was | What Ctrl+C does |
|---|---|
| waiting for the model | Stops waiting. A streamed reply from a local server is dropped, which closes the connection, so the server stops generating. Nothing from that reply is kept. |
| running a command | Kills the command and everything it started. The model gets the output so far and `The user interrupted this command (Ctrl+C) after N seconds`. |
| calling an MCP tool | Stops waiting and tells the server the call is cancelled. The server keeps running. |
| asking you to confirm | The commands are not run. |

What you type next goes to the model as `Interrupted by user: <your text>` and the run carries on.
Enter alone carries on with `Temporary interruption caught.`. `/y`, `/c` and `/u` switch the
mode, `/compact` compacts the history, and `/q` or a second Ctrl+C ends the run with exit code
130. Commands and MCP servers run in their own process group, so the key press reaches Umbra and
not them. Without a terminal, Ctrl+C ends the run straight away. If Umbra ever stops responding,
a third Ctrl+C exits.

**Typing while the agent works.** A line you type and send with Enter while Umbra is working is
queued. Before its next model call Umbra adds it to the conversation as `The user sent this
message while you were working: <your text>` and shows it under a `message from the user`
heading, so the agent sees it without the run having to finish. To have it read at once, press
Ctrl+C after sending it: the current command or model call stops, and the queued line becomes the
comment (`Interrupted by user: <your text>`). This is how the Tasks board in AI Service delivers a
comment to an Umbra agent that is in the middle of a task.

**`--keep-open`**, for a session another program types into: an empty line or a Ctrl+C at the
task prompt or the follow up prompt asks again instead of ending the run, and a second Ctrl+C at
the comment prompt carries on instead of ending it. Only `/q`, the end of input or a third
unhandled Ctrl+C ends such a run.

**The status bar** sits under the output in a terminal and is drawn again after every line Umbra
prints, so it always shows the current numbers:

```text
 ● thinking │ context ████████░░░░░░░░░░░┊░░░░  33% · 42.6k / 128k │ 59.8k left before compaction at 80% │ ↻ 2 compactions (1 summary) │ step 14 │ 12m04s │ qwen-3.8
```

| Part | What it shows |
|---|---|
| `● thinking` | What the agent is doing: `starting`, `thinking` (a model call), `running <program>`, `compacting`, `waiting for you` (a prompt) or `finished`. |
| context bar | How full the context window is. The bar fills in eighths of a cell, and `┊` marks where compaction starts. It is green while the request is under 60% of the compaction point, yellow under 85%, and red past that. |
| `33% · 42.6k / 128k` | The estimated size of the next request, as a share of the window and in tokens. This is the number compaction is decided on. |
| `59.8k left before compaction at 80%` | The tokens left before the next compaction runs. `compaction off` with `--no-compact`. |
| `↻ 2 compactions (1 summary)` | The compactions this session has had, counted across `--resume`, and how many of them were summaries. The rest replaced old command outputs. |
| `step 14`, `$0.43`, `12m04s`, model | Model calls in the session, the cost (left out for a free local model), the time since this run started, and the model. |

When the terminal is narrow, the parts lower in the table shorten or are left out first; the bar
and the percentage stay. The bar is erased before each line of output and drawn again under it, so
the scrollback never keeps old copies. When the run ends, it is printed once more as the last
line. Prompts are written on the line above the bar, and what you type echoes there. It is only
drawn when stdout is a terminal and the run is not `--quiet`. `--no-status-bar`, `status_bar =
false` in the config, or `UMBRA_STATUS_BAR=0` turns it off; `UMBRA_STATUS_BAR=1` draws it even
into a pipe, which is how the tests read it.

**Images** go to the model with the task:

```bash
umbra -m anthropic/claude-haiku-5-5 -i screenshot.png "why does this dialog cut off its last button?"
umbra -m local/qwen "compare @before.png with @after.png"
```

`-i` / `--image` attaches a file. A PNG, JPEG, GIF or WebP path typed in a task, a follow up task or
a comment is attached too, with or without `@`, so you can drag a file into the terminal. Images
over 1568 pixels on the long edge are scaled down. Images that MCP tools return, such as a browser
screenshot, reach the model as images. A local model needs a vision projector loaded to read them.

**Sessions** are saved after every step, so any run can be continued, including one that was
interrupted or crashed:

```bash
umbra sessions                       # this folder's sessions, newest first
umbra --continue "now add a test"    # the newest session in this folder
umbra --resume 1794185509287 "and update the docs"
umbra --resume                       # pick one from a list
umbra --resume 17941855 --fork "try it a different way"   # copy it, leave the original alone
```

A resumed session continues in its own folder, with its own model unless `-m` says otherwise, and
in the same trajectory file. The model sees the whole history, the images included, and the next task
arrives the way a follow up task does. The step and cost limits count from the resume.

## Features: what is included and what is not

"Umbra" is what this version does. The other columns are for comparison and come from each
project's documentation as of November 2026.

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
| Full screen terminal interface | no | no | yes | yes | yes |
| Git checkpoints and undo | no | no | yes | not documented | yes |

The items marked "no" for Umbra are left out on purpose for this version, to keep the loop as
small and easy to follow as mini-swe-agent's. MCP servers can supply several of them, such as web
search or a browser.

## Commands

| Command | What it does |
|---|---|
| `umbra [options] <task>` | Runs a task. `umbra run` is the same. |
| `umbra prompt <task>` | Prints the system prompt and first message the task would get. |
| `umbra instructions` | Lists the instruction files in prompt order, with sizes. |
| `umbra skills` | Lists the skills and where each came from. |
| `umbra commands` | Lists the slash commands. |
| `umbra mcp [--connect]` | Lists MCP servers; with `--connect` starts them and lists their tools. |
| `umbra config` | Prints the merged configuration. |
| `umbra sessions [--all]` | Lists saved sessions, newest first, for `--resume`. |

The listing commands take `--json`. The options used most:

| Option | Meaning |
|---|---|
| `-m, --model provider/model` | Which model to use |
| `-y, --yolo` | Run commands without asking |
| `--text` | Text mode instead of tool calls |
| `-l, --cost-limit <dollars>` | Stop at this cost (default 3.00; 0 means no limit) |
| `--step-limit <n>` | Stop after this many model calls |
| `--cwd <folder>` | Work in this folder |
| `--docker <image>` | Run every command in a new container |
| `-c, --config <file>` | Another config file, TOML or mini-swe-agent YAML |
| `--no-mcp`, `--mcp a,b` | No MCP servers, or only these |
| `--no-skills`, `--no-instructions` | Leave skills or instructions out |
| `--no-source claude,codex,...` | Ignore one tool's files |
| `-o, --output <file>` | Where to write the trajectory |
| `--exit-immediately` | Do not ask for a follow up task |
| `-q, --quiet` | Print only the final answer |
| `--no-status-bar` | Do not draw the status bar under the output |
| `--no-compact` | Never compact the history |
| `--context-window <tokens>` | The model's context window, for compaction |
| `-i, --image <file>` | Attach an image to the task. Repeatable. |
| `-r, --resume [session]` | Continue a saved session: its id, the start of its id, or a trajectory file |
| `--continue` | Continue the newest session in this folder |
| `--fork` | With `--resume` or `--continue`: continue a copy of the session |

On Windows, Git Bash rewrites an argument that starts with `/` into a Windows path. To run a slash
command from Git Bash, write `MSYS_NO_PATHCONV=1 umbra /review`. PowerShell and cmd do not do this.

## Project layout

```text
src/
  main.rs, cli.rs        command line
  models/                plain data: messages, actions, config, context, trajectory
  repositories/          reading and writing files: config, instructions, skills,
                         commands, MCP config, trajectories
  services/              the agent loop, model providers, environments, MCP client,
                         prompt templates, the confirm prompt
  templates/             the Jinja prompt templates
  utils/                 paths, frontmatter, variable expansion, HTTP, terminal output, status bar
tests/                   integration tests that run the real binary
docs/                    the documents linked above
tasks/                   the design document
```

Run the tests with `cargo test`. The integration tests use a fake home folder and never read your
real configuration. Two more sets are left out of a plain `cargo test` and run with `--ignored`:

```bash
cargo test --test live_local -- --ignored --test-threads 1       # against the model on localhost:8080
cargo test --test console_windows -- --ignored --test-threads 1  # a real Ctrl+C in a console window
```

`UMBRA_LIVE_MODEL` picks the model for the live tests, and `UMBRA_LIVE_EVIDENCE=<folder>` keeps their
trajectories.
