# Getting started

This page installs Umbra, chooses a model, runs a first task, and shows how to check what Umbra
found on your machine. It takes about five minutes.

The output on this page was printed by Umbra 0.7.2 on Windows, in a small Python project called
`shop` with a `CLAUDE.md`, a Cursor rule, an `AGENTS.md`, three skills from three tools and one MCP
server.

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
against the release's `SHA256SUMS`, and install it into your home folder. Neither needs
administrator rights.

| Script | Installs into | Variables it reads |
|---|---|---|
| `install.sh` | `~/.local/bin` | `UMBRA_VERSION=0.7.2` for one version, `UMBRA_INSTALL_DIR=<folder>` for another folder |
| `install.ps1` | `%LOCALAPPDATA%\Programs\umbra`, added to your user `PATH` | the same two |

On Windows, Umbra runs commands with Git Bash, so install [Git for Windows](https://gitforwindows.org)
too. Umbra never uses `C:\Windows\System32\bash.exe`, which would run the command inside WSL.

**Other ways to install**

| How | Command |
|---|---|
| Homebrew, macOS and Linux | `brew install black-rainbow-labs/umbra/umbra` |
| cargo, from source | `cargo install --git https://github.com/Black-Rainbow-Labs/umbra --locked` |
| A download | the `.tar.gz` or `.zip` for your machine from the [releases page](https://github.com/Black-Rainbow-Labs/umbra/releases/latest) |
| From a checkout | `cargo install --path .`, with Rust 1.85 or newer |
| An editor | pick Umbra from the editor's ACP agent list. See [Editors and ACP](acp.md) |

Check that it is on your `PATH`:

```text
$ umbra --version
umbra 0.7.2
```

**To have another coding agent install Umbra for you**, paste this to it:

```text
Install Umbra, the coding agent from https://github.com/Black-Rainbow-Labs/umbra.
On macOS or Linux run: curl -fsSL https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.sh | sh
On Windows run in PowerShell: irm https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.ps1 | iex
On Windows, check that Git for Windows is installed, because Umbra runs commands with Git Bash.
Then run `umbra --version` to check it is on the PATH, and run
`umbra setup --model anthropic/claude-haiku-5-5 --no-test` (or another provider/model I name) to save the model.
To add it to an editor that speaks ACP, the command is `umbra` with the argument `acp`.
```

## Choose a model

`umbra setup` asks which provider and model to use, saves the answer as `model` in
`~/.umbra/config.toml`, and sends one short request to check that the model answers.

```text
$ umbra setup
Which model should Umbra use?
  1. Anthropic Claude (an API key, or your Claude Code login)
  2. A local llama.cpp server on http://localhost:8080
  3. Ollama on http://localhost:11434
  4. OpenAI (an API key)
  5. OpenRouter (an API key)
Number [1]:
Model name [claude-sonnet-5-5]:
```

| Option | What it does |
|---|---|
| `--model provider/model` | Saves this model without asking, such as `umbra setup --model local/qwen` |
| `--no-test` | Saves the model without sending the test request |

If you are signed in to Claude Code, Umbra uses that login for Claude and you need no key.
Otherwise set `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` or `OPENROUTER_API_KEY`, or paste the key when
`umbra setup` asks for it. A local llama.cpp or Ollama server needs no key. [Models](providers.md)
covers every provider.

You can skip `umbra setup` and pass the model on every run with `-m provider/model`, or set the
`UMBRA_MODEL` environment variable.

## Run a first task

```bash
umbra -m anthropic/claude-haiku-5-5 "create sum.py that prints the sum of 1 to 100, run it, and tell me the result"
```

Umbra opens its fullscreen view, sends the task to the model, and shows each command the model asks
for on the input line with `Run? [Enter]`:

- **Enter** runs the command in a new shell, and the model gets the output.
- **Type a reason and press Enter** to refuse it. The command does not run, and the model gets
  your reason.
- **`/y`** switches to yolo mode, so later commands run without asking.

When the model replies without asking for a command, the turn ends and the screen says
`Type a follow up task, or press Enter to quit`. Type your next message to continue the same
conversation. Press Enter on the empty input line, press Ctrl+D, or type `/quit` to end the
session. Your shell comes back as it was, with the path of the session's trajectory file
printed under it.

The same task on a local model served by llama.cpp's `llama-server` on port 8080:

```bash
umbra -m local/qwen "create sum.py that prints the sum of 1 to 100, run it, and tell me the result"
```

Add `-y` to run every command without asking. Use it in a container (`--docker <image>`) or a
checkout you can throw away. [Modes](modes.md) explains the three modes.

## See what Umbra found

Umbra reads the files you already set up for other agents. Four commands list what it found,
before you run anything.

```text
$ umbra instructions
3 instruction files, 447 bytes

claude     User      160 bytes  C:/Users/jason/home\.claude\CLAUDE.md
cursor     Project       94 bytes  C:\Users\jason\shop\.cursor\rules\money.mdc
agents     Project      193 bytes  C:\Users\jason\shop\AGENTS.md
```

The files are listed in the order the model reads them, the most general first, so a project's
file comes after your own and wins when they disagree.

```text
$ umbra skills
3 skills

sql-style                    agents         User
    How SQL is written in my projects. Use when writing or changing a SQL query or migration.
    C:/Users/jason/home\.agents\skills\sql-style\SKILL.md
release-notes                claude         User
    Write release notes for the changes since the last tag. Use when asked for release notes or a changelog entry.
    C:/Users/jason/home\.claude\skills\release-notes\SKILL.md
security-review              codex          User
    Review a change for injection, secrets in code and unsafe file handling. Use when asked for a security review.
    C:/Users/jason/home\.codex\skills\security-review\SKILL.md
```

```text
$ umbra mcp --connect
termoil              connected, protocol 2025-06-18, 39 tools

mcp__termoil__termoil_new_window                   Opens a new window with one tab.
mcp__termoil__termoil_new_tab                      Opens a new tab in the focused window, in the given directory or the focused pane's.
mcp__termoil__termoil_split                        Splits the pane, starting a new shell in the new half.
```

| Command | What it lists |
|---|---|
| `umbra instructions` | The instruction files, in prompt order, with their sizes |
| `umbra skills` | Every skill, the tool whose folder it came from, and its path |
| `umbra commands` | The slash commands |
| `umbra mcp` | The MCP servers, `on` or `off`, and the file each came from. `--connect` starts them and lists their tools |
| `umbra prompt "<task>"` | The exact system prompt and first message a task would get |

Every listing command takes `--json`. [Where files are found](discovery.md) lists every place Umbra
looks.

## Keep it up to date

In the fullscreen view Umbra checks for a newer release once a day. When there is one, a prompt
above the input line offers **Install and restart**, **Remind me tomorrow** and **Skip this
version**. Install checks the download against the release's `SHA256SUMS`, replaces the program,
and starts the new version on the same session.

```bash
umbra update --check     # say whether a newer release exists
umbra update             # install it now
```

A Homebrew install is updated with `brew upgrade`, which `umbra update` runs for you.

## Where to go next

- [How it works](how-it-works.md): the loop, and what happens at each step.
- [The terminal view](terminal.md): the header, the commands that start with `/`, the keys and the status bar.
- [Sessions, Ctrl+C and images](sessions.md): stop a run, steer it, resume it later.
- [Editors and ACP](acp.md): run Umbra in Zed, a JetBrains IDE or Unluminous.
