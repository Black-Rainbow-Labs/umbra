# Modes: confirm, yolo and human

This page explains the three modes, how to refuse a command, which commands can run without asking,
and how to run every command in a Docker container.

| Mode | Flag | What happens |
|---|---|---|
| Confirm | the default | Umbra shows each command and waits. Press Enter to run it, or type a reason to refuse it. The reason is sent to the model. |
| Yolo | `-y` or `--yolo` | Commands run without asking. Use it in a container (`--docker <image>`) or a checkout you can throw away. |
| Human | `--mode human` | You type the commands. The model sees them when you switch back with `/c` or `/y`. |

`mode = "yolo"` in `~/.umbra/config.toml` sets the mode every session starts in.

## Confirm mode

```text
$ cat cart.py
Run? [Enter] yes, /y yolo, /u human mode, /h help, or type why not:
```

The command waits on the input line. What you type decides what happens:

| You type | What happens |
|---|---|
| Enter | The command runs in a new shell, and the model gets its output |
| A reason, then Enter | The command does not run. The model is told it was refused, with your reason, and tries again |
| `/y` | Switches to yolo mode and runs this command |
| `/u` | Switches to human mode, so you type the commands |
| `/h` | Shows the help for these short commands |
| Ctrl+C | The command does not run. Umbra asks for a comment. See [Sessions, Ctrl+C and images](sessions.md#ctrlc) |

A reply that asks for several commands asks once for all of them.

## Commands that run without asking

`whitelist_actions` is a list of regular expressions matched at the start of a command. A command
that matches runs without asking in confirm mode.

```toml
whitelist_actions = ["ls", "cat ", "git (status|diff|log)", "python -m pytest"]
```

A reply runs without asking only when every command in it matches. In an editor, **Always allow**
on a permission request adds the program for the rest of that session. See
[Editors and ACP](acp.md#what-a-turn-looks-like-to-the-editor).

## Yolo mode

```bash
umbra -y "run the tests and fix what fails"
```

Every command runs as soon as the model asks for it. The command is still shown in the transcript
with its output, and Ctrl+C still stops it.

Without a terminal on stdin, as in a script or a pipe, Umbra refuses to start in confirm mode, so it
never waits for an answer nobody can give:

```text
umbra: stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking
```

## Human mode

In human mode the input line takes shell commands. Each one runs the way a model's command would,
and its output joins the conversation. `/c` gives control back to the model in confirm mode, and
`/y` gives it back in yolo mode. The model then sees every command you ran and its output.

## Switching mode during a session

| How | Where |
|---|---|
| `/mode`, or `/mode confirm`, `/mode yolo`, `/mode human` | The input line of the fullscreen view |
| `/y`, `/u`, `/c` | The confirm prompt, the human prompt and the Ctrl+C comment prompt |
| `umbra session mode yolo` | A command the agent runs, or another terminal with `--session <id>`. See [Command line](command-line.md#umbra-session) |
| The editor's mode picker | ACP offers `confirm` and `yolo` |

The header's `mode` row always shows the current mode.

## Running commands in Docker

```bash
umbra --docker python:3.12 -y "write a script that prints the Python version, and run it"
```

Umbra starts one container with `docker run -d <image> sleep 2h`, runs each command in it with
`docker exec -w <folder> <container> bash -lc`, and removes the container at the end of the run.

Umbra does not mount your project into the container. The commands run in the image's own file
system, in `/` unless `environment.cwd` names another folder. To work on a checkout, mount it with
`run_args` and point `environment.cwd` at the mount:

```toml
[environment]
kind = "docker"
image = "python:3.12"
run_args = ["--rm", "-v", "/home/me/shop:/work"]
cwd = "/work"
```

| Setting | What it does |
|---|---|
| `environment.kind = "docker"` | Use Docker for every session |
| `environment.image` | The image, the same as `--docker <image>` |
| `environment.cwd` | The folder the commands run in inside the container. Empty means `/` |
| `environment.run_args` | Extra arguments for `docker run`, `["--rm"]` by default |
| `environment.forward_env` | Host environment variables copied into the container |

## Limits

Every run has limits, and each one ends the turn when it is reached. 0 means no limit.

| Setting | Flag | Default |
|---|---|---|
| `step_limit` | `--step-limit <n>` | 0 |
| `cost_limit` | `-l <dollars>` | 0 |
| `wall_time_limit_seconds` | none | 0 |
| `environment.timeout` | none | 30 seconds for one command |

In a terminal, Umbra asks whether to raise the cost limit when a run reaches it.
