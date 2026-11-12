# Sessions, Ctrl+C and images

This page explains how to stop Umbra and steer it while it works, how to give the model images, and
how sessions are saved, listed, resumed and copied.

## Ctrl+C

Ctrl+C stops what Umbra is doing and asks what to do next, the way mini-swe-agent does:

```text
Interrupted. Type a comment/command (/h for available commands)
>
```

| Umbra was | What Ctrl+C does |
|---|---|
| Waiting for the model | Stops waiting. A reply streaming from a local server is dropped, which closes the connection, so the server stops generating. Nothing from that reply is kept |
| Running a command | Stops the command and everything it started. The model gets the output so far and `The user interrupted this command (Ctrl+C) after N seconds` |
| Calling an MCP tool | Stops waiting and tells the server the call is cancelled. The server keeps running |
| Asking you to confirm | The commands are not run |

What you type next goes to the model as `Interrupted by user: <your text>`, and the run carries on
with everything it had. Enter alone carries on with `Temporary interruption caught.`.

| At the comment prompt | What it does |
|---|---|
| Text, then Enter | Sends the text to the model and carries on |
| Enter | Carries on |
| `/y`, `/c`, `/u` | Switches to yolo, confirm or human mode and carries on |
| `/compact` | Compacts the history |
| `/q`, or a second Ctrl+C | Ends the run with exit code 130. The session is saved |

Commands and MCP servers run in their own process group, so the key press reaches Umbra and not
them. Without a terminal, Ctrl+C ends the run at once. If Umbra ever stops responding, a third
Ctrl+C exits.

## Typing while the agent works

A line you type and send with Enter while Umbra works is queued. Before its next model call Umbra
adds it to the conversation as `The user sent this message while you were working: <your text>` and
shows it under a `message from the user` heading. The agent sees it without the run having to
finish.

To have it read at once, press Ctrl+C after sending it. The current command or model call stops, and
the queued line becomes the comment, `Interrupted by user: <your text>`.

## Sessions for another program to type into

`--keep-open` is for a session another program types into, such as a task board that sends an
agent its comments. An empty line or a Ctrl+C at the task prompt or the follow up prompt asks again
instead of ending the run, and a second Ctrl+C at the comment prompt carries on instead of ending
it. Only `/q`, the end of input, or a third Ctrl+C that Umbra has not handled ends such a run.

## Images

```bash
umbra -m anthropic/claude-haiku-5-5 -i screenshot.png "why does this dialog cut off its last button?"
umbra -m local/qwen "compare @before.png with @after.png"
```

| How an image reaches the model | Example |
|---|---|
| `-i` or `--image`, repeatable | `umbra -i a.png -i b.png "what changed"` |
| A path typed in a task, a follow up task or a comment, with or without `@` | `look at @shot.png`, or a file dragged into the terminal |
| An image an MCP tool returns, such as a browser screenshot | it reaches the model as an image |
| An image block an editor sends over ACP | see [Editors and ACP](acp.md) |

PNG, JPEG, GIF and WebP files are read. An image over 1,568 pixels on its long edge is scaled down.

| Setting | Default | What it does |
|---|---|---|
| `images.max_long_edge` | 1568 | Images whose long edge is bigger are scaled down to it. 0 never scales |
| `images.max_bytes` | 10485760 | An image whose base64 text is bigger is refused |
| `images.detect_paths` | true | Attach image files whose paths are typed in a task, a follow up task or a comment |

A local model needs a vision projector loaded to read images, such as llama.cpp's `--mmproj`. A
server that cannot read images answers with an error, and the run ends with `ModelError`.

## Sessions

A session is one conversation. Umbra saves it after every step, so any run can be continued,
including one that was interrupted or crashed.

```bash
umbra sessions                       # this folder's sessions, newest first
umbra resume                         # pick one from a list with a title and a summary
umbra resume 17941855                # resume one directly, by its id or the start of it
umbra --continue "now add a test"    # the newest session in this folder, with a new task
umbra --resume 1794185509287 "and update the docs"
umbra --resume 17941855 --fork "try it a different way"   # continue a copy, leave the original alone
```

| Command | What it does |
|---|---|
| `umbra sessions` | Lists this folder's sessions, newest first. `--all` lists every folder's |
| `umbra resume` | Opens the session picker. Without a terminal, it prints the list the way `umbra sessions` does and asks for an id. No model needs to be configured: the chosen session runs on the model it recorded |
| `umbra resume <id>` | Resumes that session |
| `umbra resume --all` | The picker with every folder's sessions |
| `-r`, `--resume [session]` | Resumes a session by its id, the start of its id, or a trajectory file. With no value, opens the picker |
| `--continue` | Resumes the newest session in this folder |
| `--fork` | With `--resume` or `--continue`: copies the history into a new session and leaves the old one as it was |
| `/resume` | Opens the session picker inside the fullscreen view |
| `/clear` | Saves this session and starts a new one with an empty history |

**The session picker** shows two rows for each session: its title with the age, the model and the
number of model calls, and under it a one sentence summary. Up and Down move, Enter resumes, typing
filters and Esc cancels.

**Titles and summaries.** The title of a session starts as the first line of its first prompt.
After the first turn, in a terminal, Umbra makes one small request to the session's model for a
title of at most six words and a one sentence summary. `sessions.auto_title = false` turns that
request off. A compaction summary replaces the session summary.

**What a resume keeps.** A resumed session continues in its own folder, with its own model unless
`-m` says otherwise, and in the same trajectory file. This holds when no model is set in
`~/.umbra/config.toml` or `UMBRA_MODEL`, for `umbra resume`, `umbra resume <id>`, `--resume <id>`
and `--continue`. A session that recorded no model stops with the error for no model set. The model sees the whole history, the images
included, and the next task arrives the way a follow up task does. The MCP servers turned off in the
session are turned off again. The step and cost limits count from the resume. After a resume the
fullscreen view shows the earlier prompts, replies and command groups.

**Editors and the terminal share sessions.** A session started in Zed or Unluminous over ACP can be
continued with `umbra --resume <id>`, and the other way round.

Umbra does not print a resume command during a session or when it exits. In quiet mode an
interrupted run still prints `umbra: interrupted. Resume with: umbra --resume <id>` on stderr,
because a script has no other way to learn the id.

## Where sessions are kept

| File | What it holds |
|---|---|
| `~/.umbra/trajectories/<time>.traj.json` | The whole session in mini-swe-agent's trajectory format: every message exactly as the model was sent it. `-o <file>` or `output` chooses another path |
| `~/.umbra/sessions/<id>.json` | A small index entry: the folder, model, first task, title, summary, status, number of calls, cost, the MCP servers turned off and the agent time |
| `~/.umbra/control/<id>/` | Only while a terminal session runs: what `umbra session` reads and writes. See [Command line](command-line.md#umbra-session) |

[Architecture](architecture.md#the-trajectory-file) describes the trajectory file field by field.

## Where to go next

- [Command line](command-line.md): every command and option, and `umbra session` for a running session.
- [Configuration](configuration.md): the settings this page names.
