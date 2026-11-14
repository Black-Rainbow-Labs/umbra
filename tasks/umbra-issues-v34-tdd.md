# Umbra issues v34 (task-2659)

## Introduction

This ticket asks for five changes to Umbra 0.7.2:

1. Scrolling to the top of the fullscreen transcript lags when the terminal draws with shaders. The
   logo repaints slowly, black boxes show for a moment, and the scroll stops answering for a few
   seconds.
2. A fenced `mermaid` block in a reply is drawn as a diagram.
3. LaTeX math in a reply is drawn as math.
4. Footnotes in a reply are drawn as footnotes. Today `[^1]` and the `[^1]: text` definition print
   as plain text.
5. `umbra resume` with no id fails with `no model set` when no model is configured, before the
   session picker opens, although every saved session records its own model.

## Goals

| # | Goal | How it is checked |
|---|---|---|
| G1 | A burst of wheel events is drawn as one frame, a frame that changes nothing writes nothing, and a frame writes only the rows that changed. The logo's bytes are at least halved. | Unit tests on the batched key handling, on the row diff (applied to a small screen model) and on the logo's byte count. |
| G2 | `graph`/`flowchart` (TD, TB, BT, LR, RL), `sequenceDiagram` and `pie` blocks are drawn with box characters. Any other diagram type, a parse failure, or a diagram wider than the reply shows the source as a code block, as today. | Golden files for each diagram type at 80 and 40 columns. A test that no row is wider than the width. |
| G3 | `$...$`, `\(...\)` are drawn inline as Unicode math. `$$...$$` and `\[...\]` are drawn as a block, with fractions stacked, roots, limits above and below big operators, and matrices in tall brackets. | Unit tests per construct. Golden file. |
| G4 | `[^name]` is drawn as `[1]`, numbered by first use, and the definitions are drawn at the end of the reply under a short rule, each starting with its number. | Golden file. |
| G5 | `umbra resume` with no model configured opens the picker, and the chosen session runs on the model it recorded. `umbra resume <id>` and `--resume <id>` work the same way. | Integration tests with a home folder that has a saved session and no `config.toml`. |

## Non goals

- No terminal image protocols (kitty, iTerm2, sixel). Diagrams and math are drawn with text, so
  they work in every terminal, in a pipe and in a copy.
- No full Mermaid or TeX implementation. Class, state, ER, gantt and other Mermaid diagrams fall
  back to the source. TeX commands Umbra does not know are printed by name.
- No change to the inline view's drawing, which prints each line once.

## 1. The scroll lag

### What happens today

- `console::keys_to_lines` calls `screen::handle_key` for every decoded key. Each call takes the
  view's lock, applies the key, renders the whole frame and writes it with a flush. A mouse wheel
  sends one report per notch, so a fast scroll is dozens of full frames.
- When the terminal is slow to draw (the neon glow shader with the glow pulse cursor), the write
  blocks until the terminal reads it. Reports pile up in the input buffer, and every one of them is
  then drawn as its own full frame. That is the few seconds of no response.
- Every frame repaints every row. The ticker draws a frame every 100 ms while anything animates (a
  spinner, the agent time, a starting MCP server), and most of those frames are identical to the
  one before, or differ in one row.
- The logo is about 60 by 11 cells of 24 bit color, and each cell is written with its own SGR code
  and its own reset: about 16 KB per frame for the logo alone. The header rebuilds all six steps of
  the logo reveal from pixels at every frame.

### The change

1. **Batch keys.** `screen::handle_keys(keys)` applies every key of one read under one lock and
   draws once. A key that returns effects (work that must run without the lock) draws, releases
   the lock, runs the effects, and the rest of the keys continue in the next batch. When the
   terminal falls behind, the next read returns every report that piled up, and they become one
   frame.
2. **Draw only what changed.** `screen.rs` keeps the rows and cursor of the last frame written.
   A new frame writes only the rows that differ, each with a cursor move and an erase, inside the
   same synchronized output block. A frame identical to the last one writes nothing, so a ticker
   frame that changes nothing costs nothing. A resize, entering the view, and `redraw` after a
   program Umbra ran draw the whole frame. `UMBRA_FORCE_FULLSCREEN=1`, which the tests use, keeps
   full frames, because the test harness reads the last full frame.
3. **Smaller logo.** `banner::logo_lines` writes a color code only when the color changes from the
   cell before, and one reset at the end of the row.
4. **Cached header.** `banner::render_banner_step` keeps the last result for the same info, width,
   color and step, so a frame does not rebuild the pixels.

## 2. Mermaid diagrams

`markdown::finish_code` sends a block whose language is `mermaid` to a new module,
`src/ui/mermaid.rs`, which returns rows or None. None draws the source as a code block.

### Flowcharts

- **Parsing.** Statements are split at line breaks and `;`. A statement is a chain of node
  references joined by links: `A[Text] -->|label| B{Choice} --> C`. `&` joins several nodes on one
  side. Node shapes `[ ]`, `( )`, `([ ])`, `[[ ]]`, `[( )]`, `(( ))`, `{ }`, `{{ }}`, `> ]` are read,
  with quoted text and `<br>`. Links `-->`, `---`, `-.->`, `==>`, `--o`, `--x`, `<-->` and longer
  forms are read, with a label in `|...|` or between the dashes (`-- yes -->`). `subgraph`, `end`,
  `style`, `classDef`, `class`, `linkStyle`, `click`, `direction` and `%%` comments are skipped.
- **Layout.** The layered method: cycles are broken by reversing the links that point back along a
  depth first search, each node gets the layer of its longest path from a source, a link longer than
  one layer gets a placeholder node in each layer it crosses, and the nodes of each layer are
  ordered by the average position of their neighbors, in four passes down and up.
- **Drawing.** Nodes are boxes: `┌┐└┘` for rectangles, `╭╮╰╯` for rounded shapes, `╱╲` corners
  for a decision. Between two layers is a channel. A link leaves the upper node at its center, runs
  down, turns along its own row of the channel when the two centers differ, and runs down to the
  lower node, where `▼` points into it. Lines are kept as connection bits per cell, so corners,
  tees and crossings get the right box character. A label sits on the last straight part of its
  link. `LR` and `RL` swap the axes. `BT` and `RL` reverse the order of the layers, and an arrow can
  point up or left.

### Sequence diagrams

Participants (`participant`, `actor`, with `as` aliases, or first named in a message) get a box at
the top and bottom and a lifeline. Each message is a label row and an arrow row between two
lifelines: `->>` and `-->>` with `▶`, `->` and `-->` with no head, `-x` with `x`, `-)` with `▷`,
dashed for the `--` forms. A message to itself is a loop on the right. Gaps between lifelines grow
to fit the longest label between them. `Note left of`, `right of` and `over` are boxes.
`loop`, `alt`, `else`, `opt`, `par`, `and`, `critical`, `break`, `rect` and `end` are dotted rows
with their text. `autonumber` numbers the messages.

### Pie charts

The title, then one row per slice: the label, a bar scaled to the largest slice, and the percent.
With color on, each bar gets one of the basic terminal colors.

When a diagram is wider than the reply, a flowchart in `LR` is tried as `TD` first, and if that is
also too wide the source is shown.

## 3. LaTeX math

`Options::ENABLE_MATH` makes the parser return `InlineMath` and `DisplayMath`. Before parsing,
`\(...\)` becomes `$...$` and `\[...\]` becomes `$$...$$`, outside code blocks and code spans,
because models write both forms.

A new module, `src/ui/math.rs`, parses the TeX into a tree: groups, `^` and `_`, `\frac`,
`\dfrac`, `\tfrac`, `\binom`, `\sqrt[n]{}`, `\left ... \right`, big operators (`\sum`, `\prod`,
`\int`, `\lim` and others), environments `matrix`, `pmatrix`, `bmatrix`, `vmatrix`, `cases`,
`aligned`, `align`, font commands (`\mathbb{R}` is `ℝ`, `\mathbf` and the others keep their
content), `\text{}`, accents (`\hat`, `\bar`, `\vec`, `\dot`, `\tilde` as combining marks) and
about 250 symbols.

- **Inline math** is drawn on one line: superscripts and subscripts as Unicode small characters when
  every character has one (`x²`, `aᵢ`), otherwise `^(...)` and `_(...)`. A fraction is `a/b`, with
  parentheses around a side that has more than one term. A root is `√x` or `√(x+1)`.
- **Display math** is drawn as a block of rows, centered in the reply: a fraction is stacked with a
  line between the parts, a root gets a line over its content, limits go above and below a big
  operator, a matrix sits in brackets as tall as its rows, and a superscript that has no Unicode
  form is raised one row. A block wider than the reply is drawn in the one line form, wrapped.

## 4. Footnotes

`Options::ENABLE_FOOTNOTES` (the GitHub rules). A reference gets the next number at its first use
and is drawn as `[1]`. A definition is laid out like a list item whose marker is `[1] `, and its
rows are moved out of the flow and kept. At the end of the reply comes a blank row, a dim rule of
up to 20 columns, and the kept definitions in number order.

## 5. `umbra resume` without a model

`run_service::run_task` calls `check_credentials(&config, &config.model)` before the fullscreen view
opens. `umbra resume` with no id sets `resume = Some("")`, and the session is only known after the
picker, so with no `model` in the config and no `UMBRA_MODEL`, `split_model_spec` fails with `no
model set`.

The change: when the run will show the fullscreen picker and the config has no model, the check
waits. After the picker returns a session, the session's model is taken when the command line named
none, its credentials are checked (a missing credential stops the run with that error), and the
header, the status bar and the usage poll get the model. A session with no recorded model still
fails with `no model set`, now after the picker.

`umbra resume <id>`, `--resume <id>` and `--continue` load the session before the config is read,
and `fill_from_session` already takes the session's model, so they work today. The integration tests
cover all four forms so a later change cannot break them without a test failing.

## Documentation

- `docs/terminal.md`: the reply section lists diagrams, math and footnotes.
- `docs/sessions.md`: says the picker opens with no model configured.
- The Terminal and Sessions chapters in `sites/unluminous/src/data/umbraDocumentation.ts`.
- `docs/README.md` lists this design document.

## Release

0.7.3, cut on the Windows PC with `pwsh packaging/release.ps1 -Part patch`, which needs `pwsh` and
`gh`. The Mac has neither, so the release is a linked ticket for the PC.
