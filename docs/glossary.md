# Glossary

This page explains the words the Umbra documentation uses. Each entry is one or two plain
sentences. The terms are grouped by topic, and each group is in alphabetical order.

If a word on another page is not here and should be, add it.

| Group | What it covers |
|---|---|
| [The agent](#the-agent) | the loop, the model and what a turn is |
| [Your setup](#your-setup) | the files Umbra reads from other tools |
| [Sessions and context](#sessions-and-context) | what is saved, and how a long session fits in the model's window |
| [The screen](#the-screen) | the parts of the fullscreen view |
| [Protocols](#protocols) | MCP, ACP and the APIs Umbra talks to |

## The agent

| Term | What it means |
|---|---|
| **Action** | One command the model asks Umbra to run, or one MCP tool call. |
| **Action format** | How the model asks for an action. In tool mode, the default, it makes a native tool call to `bash`. In text mode (`--text`) it writes one fenced `umbra_bash` block. |
| **Confirm mode** | The default mode. Umbra shows each action and waits for Enter before it runs it. |
| **Exit status** | How a turn or a run ended: `Replied`, `Submitted`, `Interrupted`, `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError` or `ModelError`. [Command line](command-line.md#exit-codes) maps each one to an exit code. |
| **Finish style** | How a turn ends. With `finish = "reply"`, the default, a reply with no command ends the turn. With `finish = "submit"`, the mini-swe-agent behavior, every reply must run a command until one prints `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT`. |
| **Format error** | A reply Umbra cannot use, such as text mode with two fenced blocks. Umbra sends it back with what was wrong. Three in a row end the run with `RepeatedFormatError`. |
| **Human mode** | A mode in which you type the commands and the model sees them. `/c` or `/y` gives control back to the model. |
| **Mode** | `confirm`, `yolo` or `human`. `/mode` and `umbra session mode` switch it during a session. |
| **Model call** | One request to the model. The step limit counts model calls. |
| **Observation** | What the model is shown after an action: the output, the return code, and a note when the command timed out or was interrupted. |
| **Provider** | A named way to reach a model, such as `anthropic`, `local` or `openrouter`. `-m provider/model` picks one. |
| **Step** | One model call and the actions it asked for. |
| **Turn** | Everything from one message you send to the reply that ends it. A session is one or more turns. |
| **Yolo mode** | A mode in which actions run without asking. `-y` starts in it and `/y` switches to it. |

## Your setup

| Term | What it means |
|---|---|
| **`@path` import** | A line in a Claude Code or Gemini instruction file that pulls another file into it. Umbra follows imports up to four levels deep. |
| **Instruction file** | A Markdown file that tells an agent how to work, such as `~/.claude/CLAUDE.md`, a project's `AGENTS.md`, or a Cursor rule. Umbra adds them to the system prompt, the most general first. |
| **Project root** | The nearest folder at or above the working folder that holds `.git`. With no `.git`, the working folder itself. |
| **Skill** | A folder with a `SKILL.md` file whose frontmatter gives a name and a description. Umbra lists each skill's name, description and path in the system prompt, and the model reads the file when a task matches. |
| **Slash command** | A task that starts with `/name`. Umbra replaces it with that command's prompt from a file such as `.claude/commands/name.md`, or with a skill of that name. |
| **Source** | One other tool whose files Umbra reads, such as `claude` or `codex`. `--no-source <name>` ignores one. |
| **System prompt** | The first message of every session. It holds Umbra's instructions to the model, your instruction files and the list of your skills. `umbra prompt "<task>"` prints it. |

## Sessions and context

| Term | What it means |
|---|---|
| **Compaction** | Making the history smaller when it nears the model's context window. Umbra first replaces old command outputs with a short placeholder (the mask pass), and when that is not enough asks the model to summarize the middle of the session (the summary pass). |
| **Context window** | The most tokens a model can read in one request. Umbra asks the server for it, or uses a built in table. |
| **Control folder** | `~/.umbra/control/<session id>/`, which a running terminal session keeps so that `umbra session` can change its model, mode or MCP servers. |
| **Deferred tools** | MCP tools sent to the model by name only, with a `tool_search` tool that loads their definitions. Umbra does this when the definitions would take more than a tenth of the context window. |
| **Fork** | Continuing a copy of a session and leaving the original as it was. `--fork` does it. |
| **Prompt cache** | A provider's store of a request it has already read, so the next request with the same start costs less and answers sooner. Umbra keeps earlier messages unchanged so the cache keeps matching. |
| **Session** | One conversation with Umbra, saved after every step, with an id that is its start time in milliseconds. `umbra sessions` lists them. |
| **Token** | The unit a model reads and writes text in. About four characters of English. |
| **Trajectory** | The JSON file of one session, in mini-swe-agent's format. Its `messages` are exactly what the model was sent on the last call. |

## The screen

| Term | What it means |
|---|---|
| **Command group** | Consecutive shell commands shown as one collapsed row, `Ran 3 shell commands · 0:42`. A click opens it. |
| **Configuration modal** | The settings editor `/config` opens, with sections on the left and fields on the right. |
| **Fullscreen view** | Umbra's screen in a terminal. It uses the alternate screen, the way `vim` does, so your shell comes back as it was when Umbra exits. |
| **Header** | The first block of the transcript: the logo, the version, and the model, folder, mode, skills and MCP rows. |
| **Inline view** | Printing below the shell prompt with no fullscreen view. `--no-fullscreen` asks for it, and a pipe always gets it. |
| **Input line** | The line at the bottom where you type. It is there while the agent works, so a message can be sent at any time. |
| **Picker** | A list above the input line to choose from: models, modes, sessions or skills. |
| **Status bar** | The last row of the fullscreen view: the context bar, the agent time, the cost, the model, the weekly usage and any MCP error. |
| **Transcript** | The scrolling record of the session: your prompts, the replies, the command groups and the notices. |

## Protocols

| Term | What it means |
|---|---|
| **ACP** | The [Agent Client Protocol](https://agentclientprotocol.com). An editor starts `umbra acp` and talks to it in JSON-RPC on stdin and stdout. |
| **Anthropic Messages API** | The API Umbra uses for Claude, `POST /v1/messages`. |
| **MCP** | The [Model Context Protocol](https://modelcontextprotocol.io). A program, the MCP server, offers tools, and Umbra offers them to the model next to `bash`. |
| **MCP transport** | How Umbra reaches an MCP server: stdio for a program it starts, Streamable HTTP or the older HTTP and SSE for a URL. |
| **OpenAI chat API** | `POST /chat/completions`, which llama.cpp, Ollama, vLLM, LM Studio, OpenAI and OpenRouter all accept. Umbra calls these the `openai` kind. |
| **Replay provider** | A provider that answers from a JSON file of scripted replies. The tests and demos use it. |
