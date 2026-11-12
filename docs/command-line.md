# Command line

This page lists every command and option of `umbra`, the commands that change a running session,
the environment variables Umbra reads, and the exit codes. `umbra --help` and
`umbra <command> --help` print the same information from the program itself.

## Commands

| Command | What it does |
|---|---|
| `umbra [options] <task>` | Runs a task. `umbra run` is the same. Words after the options are joined with spaces. A task that starts with `/name` runs that slash command or skill |
| `umbra prompt <task>` | Prints the system prompt and first message the task would get, without running it |
| `umbra instructions` | Lists the instruction files in prompt order, with sizes |
| `umbra skills` | Lists the skills and where each came from |
| `umbra commands` | Lists the slash commands |
| `umbra mcp [--connect]` | Lists the MCP servers with `on` or `off` for each. `--connect` starts them and lists their tools |
| `umbra mcp enable <name>`, `umbra mcp disable <name>` | Lets a server start in every session, or stops it. `--project` changes the project's config file |
| `umbra config` | Prints the merged configuration |
| `umbra config list\|get\|set\|unset\|path` | Reads and changes one setting. See [below](#umbra-config) |
| `umbra models` | Lists the default model, the named models and the known models |
| `umbra session <command>` | Controls a running session. See [below](#umbra-session) |
| `umbra update [--check] [--version <v>]` | Checks for a newer release and installs it |
| `umbra sessions [--all]` | Lists saved sessions, newest first |
| `umbra resume [id] [--all] [task]` | Opens the session picker, or resumes the session with that id or the start of its id |
| `umbra acp` | Runs as an ACP agent on stdin and stdout, for an editor. See [Editors and ACP](acp.md) |
| `umbra setup [--model <m>] [--no-test]` | Asks for a provider and model, saves it in `~/.umbra/config.toml`, and checks that the model answers |

The listing commands (`instructions`, `skills`, `commands`, `mcp`, `config`, `models`, `sessions`)
take `--json`.

## Options for a run

| Option | Meaning |
|---|---|
| `-t, --task <task>` | The task, as one argument |
| `-m, --model provider/model` | Which model to use, or a name from `[models.<name>]` |
| `-y, --yolo` | Run commands without asking |
| `--mode confirm\|yolo\|human` | The mode to start in |
| `--text` | Text mode instead of tool calls |
| `--finish reply\|submit` | How a turn ends: a reply with no command (the default), or the submit command |
| `-l, --cost-limit <dollars>` | Stop once the run has cost this much. 0, the default, means no limit |
| `--step-limit <n>` | Stop after this many model calls. 0 means no limit |
| `--cwd <folder>` | Work in this folder |
| `--docker <image>` | Run every command in a new container from this image |
| `-c, --config <file>` | Another config file, TOML or mini-swe-agent YAML. Repeatable, and later files win |
| `--no-mcp`, `--mcp a,b` | Start no MCP servers, or only these |
| `--no-skills`, `--no-instructions` | Leave skills or instruction files out |
| `--no-source claude,codex,...` | Ignore one tool's files. Repeatable or comma separated |
| `-o, --output <file>` | Where to write the trajectory |
| `--exit-immediately` | Exit when the turn ends instead of waiting for the next message |
| `--keep-open` | Never end the run because of an empty line. See [Sessions](sessions.md#sessions-for-another-program-to-type-into) |
| `-q, --quiet` | Print only the final answer and errors |
| `--no-fullscreen` | Print inline below the shell prompt instead of opening the fullscreen view |
| `--no-mouse` | Do not turn on mouse reporting in the fullscreen view |
| `--no-status-bar` | Do not draw the status bar |
| `--verbose` | Print every detail: step costs, whole command outputs, full format errors |
| `--no-compact` | Never compact the history |
| `--context-window <tokens>` | The model's context window, for compaction |
| `-i, --image <file>` | Attach an image to the task. Repeatable |
| `-r, --resume [session]` | Continue a saved session: its id, the start of its id, or a trajectory file |
| `--continue` | Continue the newest session in this folder |
| `--fork` | With `--resume` or `--continue`: continue a copy of the session |

`umbra -q -t "<question>" --exit-immediately` prints only the answer, the way `claude -p` does.

## umbra config

`umbra config` reads and changes any setting in [Configuration](configuration.md). It uses the same
parser and file writer as the configuration modal, so comments in the file are kept.

```text
$ umbra config get compaction.trigger_ratio
0.8

$ umbra config list compaction
compaction.enabled                     true                     default  Compact the history when it nears the context window.
compaction.context_window              0                        default  Tokens. 0 asks the provider, then a built in table.
compaction.trigger_ratio               80%                      default  Compact when the request passes this share of the window.
compaction.target_ratio                60%                      default  A mask pass that leaves more than this share is followed by a summary.
```

| Command | What it does |
|---|---|
| `umbra config` | Prints the merged config |
| `umbra config list [prefix]` | Every setting with its value, the file it comes from (`default`, `user`, `project`) and what it does |
| `umbra config get <key>` | One value |
| `umbra config set <key> <value>` | Sets a value in `~/.umbra/config.toml`. Lists are comma separated, maps are `k=v, k=v`, and a percent setting takes `70` for 70%. A value of the wrong kind is refused, and the file is left as it was |
| `umbra config unset <key>` | Removes the key, so the default or another file applies |
| `umbra config path` | The file `set` changes |

`--project` makes `set`, `unset`, `path` and `list` use `<project>/.umbra/config.toml`, and `--cwd`
names the folder. A change applies to the next session. `umbra session` changes a running one.

## umbra session

`umbra session` changes a session that is running in a terminal. A command the agent runs reaches
its own session, so you can ask the agent to switch its own model or mode. From another terminal,
name the session with `--session <id>`.

| Command | What it does |
|---|---|
| `umbra session status` | Prints the session's id, model, mode and MCP servers |
| `umbra session list` | Lists the running sessions |
| `umbra session model <name>` | Switches the model from the next model call. A name from `[models]` or a `provider/model` |
| `umbra session mode <confirm\|yolo\|human>` | Switches the mode |
| `umbra session mcp <on\|off> <name>` | Turns an MCP server on or off in this session only |
| `umbra session compact [focus]` | Summarizes the history before the next model call |
| `umbra session quit` | Ends the session after the current step |

```text
$ umbra session status
umbra: no Umbra session is running
```

While a terminal session runs it keeps `~/.umbra/control/<session id>/`. Each command the agent
runs gets `UMBRA_CONTROL_DIR`, `UMBRA_SESSION_ID` and `UMBRA_BIN` in its environment, which is how
`umbra session` finds its own session. An ACP session has no control folder, because one ACP
process serves many sessions. [Architecture](architecture.md#umbras-command-line-and-the-running-session)
explains how a request reaches the session.

## umbra update

```text
$ umbra update --check
umbra 0.7.2 is the newest release
```

| Command | What it does |
|---|---|
| `umbra update --check` | Says whether a newer release exists |
| `umbra update` | Downloads the newest release for this machine, checks it against `SHA256SUMS`, and replaces the program. A Homebrew install runs `brew upgrade` instead |
| `umbra update --version <v>` | Installs that version |

## Environment variables

| Variable | What it does |
|---|---|
| `UMBRA_MODEL` | The model, when `-m` is not given |
| `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` | The Claude credential. See [Models](providers.md#claude) |
| `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `LOCAL_API_KEY`, `OLLAMA_API_KEY` | The key for each built in provider |
| `UMBRA_FULLSCREEN=0` | Print inline instead of opening the fullscreen view |
| `UMBRA_MOUSE=0`, `UMBRA_MOUSE=1` | Turn mouse reporting off, or on in a classic Windows console |
| `UMBRA_STATUS_BAR=0`, `UMBRA_STATUS_BAR=1` | Turn the status bar off, or draw it into a pipe |
| `UMBRA_BANNER=0`, `UMBRA_BANNER=1` | Leave the header out, or draw it into a pipe |
| `UMBRA_BANNER_ANIMATION=0` | Show the finished logo at once |
| `UMBRA_UPDATE_CHECK=0` | Turn the daily update check off |
| `UMBRA_FORCE_INTERACTIVE=1` | Treat a piped stdin as a terminal, so a script or a test can type into Umbra's prompts |
| `UMBRA_HOME_OVERRIDE` | Use this folder as the home folder, for `~/.umbra` and every other tool's files. The tests and the documentation examples use it |
| `UMBRA_REPLAY_RECORD=<file>` | With the replay provider, add every request to the file as one JSON line |
| `CODEX_HOME` | Where Codex keeps its files, read the way Codex reads it |

Umbra sets `UMBRA_CONTROL_DIR`, `UMBRA_SESSION_ID` and `UMBRA_BIN` for the commands the agent runs.

## Exit codes

| Code | When |
|---|---|
| 0 | The run ended with `Replied` or `Submitted`, or a listing command succeeded |
| 1 | The run ended with any other exit status: `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError` or `ModelError` |
| 2 | Umbra could not start the command: a bad option, a missing model, a config file it cannot read, confirm mode with no terminal. The reason is printed on stderr after `umbra:` |
| 130 | The run was interrupted with Ctrl+C and then ended |

[How it works](how-it-works.md#how-a-turn-ends) explains each exit status.
