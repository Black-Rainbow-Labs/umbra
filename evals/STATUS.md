# Umbra against pi: where we stand

This page explains what the eval suite measures, how Umbra compares with pi today, and what is
left to do. The numbers are from 2026-11-14. The run by run record of every experiment is
[LEDGER.md](LEDGER.md), and how to run the suite is in [README.md](README.md).

## What the evals are

The suite gives two terminal coding agents the same 61 tasks and checks the result of each one
with a hidden verifier the agent never sees.

- **Umbra** is this repository's agent.
- **pi** (1.3.0, run exactly as shipped) is the agent we compare against.
- **The model** is the same for both: a local Qwen model, `ninfer-qwen3.8-27b-coletti-v9`, served
  on port 8080 with one request slot.
- **A measuring proxy** sits between each agent and the model. Every token count and every timing
  in this page comes from the model server's own responses, and the proxy gives both agents the
  same sampling settings (temperature 0.6, top_p 0.95, top_k 20, thinking on).
- **Each task runs 3 times** for each agent, one task at a time, because the model has one slot.
  A machine wide lock stops two eval runs from sharing the model.

### The task categories

| Category | Tasks | What it tests |
|---|---|---|
| hard | 20 | Long multi part tasks that pi passes about 70% of the time: specs with many requirements, migrations, bugs whose cause is far from the symptom |
| compaction | 9 | Work too long to fit in the context window, so the agent has to compact its history and keep going |
| session | 7 | Resume, fork, branch from an earlier message, resume after a crash, resume a 300 step session |
| bugfix | 6 | Find and fix a bug from a failing test |
| edit-precision | 5 | Edit files without touching bytes outside the change: CRLF endings, BOMs, tabs, non ASCII text |
| navigate | 4 | Find things in a code base |
| create | 4 | Write a small program from a description |
| large-output | 3 | Commands that print far more output than the context can hold |
| instructions | 3 | Follow precise output instructions |

### How a result is judged

- **Pass** means the verifier's checks all passed. **Score** is the share of checks passed.
- **pi's noise band** for a metric is the gap between pi's best and worst of its 3 reps. A
  difference smaller than that band is counted as a tie, because pi alone varies that much.
- An Umbra change is kept only if its target metric improves by more than the noise band and no
  other metric gets worse by more than its band. Otherwise it is reverted.
- **The goal (G1)** is a full 3 rep run where Umbra is strictly better than pi on every metric in
  the scorecard.

## Where Umbra stands

These numbers are from the final 3 rep run of all 61 tasks on 2026-11-14 (run `final-8df86c3`,
task-2643). That build has every kept change: E1, E2, E4, E8, E9, E10 (the persistent shell, on by
default), E11, E13 to E23, E25 and E26. E3, E5, E6, E7, E24 and E27 are off or reverted. The eval
counts every file Umbra writes (Umbra runs with its default storage, without `-o`). pi is the
reference run `pi-1.3.0-ref`.

### Head to head

| Metric | Umbra | pi | pi noise band | Better |
|---|---|---|---|---|
| Pass rate | 91.3% | 80.3% | 6.6 points | Umbra |
| Mean score | 98.7% | 90.7% | 5.4 points | Umbra |
| Hard tier pass rate | 78.3% | 66.7% | 10.0 points | Umbra |
| Hard tier mean score | 98.1% | 82.0% | 10.0 points | Umbra |
| Compaction pass rate | 92.6% | 44.4% | 22.2 points | Umbra |
| Compaction mean score | 99.3% | 77.7% | 16.9 points | Umbra |
| Prompt tokens, total | 19.6 million | 21.9 million | 0.7 million | Umbra, 10% fewer |
| Uncached prompt tokens, total | 815,800 | 3,863,170 | 133,361 | Umbra, 79% fewer |
| Completion tokens, total | 606,332 | 848,601 | 17,840 | Umbra, 29% fewer |
| Requests, total | 1,327 | 1,631 | 41 | Umbra, 19% fewer |
| Prompt tokens per passed task | 353,094 | 447,300 | 22,849 | Umbra, 21% fewer |
| Uncached prompt tokens per passed task | 14,694 | 78,944 | 7,646 | Umbra, 81% fewer |
| Completion tokens per passed task | 10,897 | 17,343 | 1,780 | Umbra, 37% fewer |
| Requests per passed task | 24 | 33 | 3 | Umbra, 27% fewer |
| First request size (fixed overhead) | 1,835 tokens | 2,154 tokens | 0 | Umbra, 15% smaller |
| Prompt cache hit rate | 95.8% | 82.4% | 1.1 points | Umbra |
| Wall time per task | 69.4 s | 117.5 s | 22.1 s | Umbra, 69% faster |
| Time to the first reply text | 382 ms | 1,095 ms | 27 ms | Umbra, 187% faster |
| Total model seconds | 3,841 | 6,092 | 130 | Umbra, 59% faster |
| Cache hit rate just after a compaction | 74.2% | 32.4% | 1.7 points | Umbra |
| Peak prompt tokens on window tasks | 45,705 | 48,212 | 1,294 | Umbra |
| Startup when resuming a long session | 36 ms | 498 ms | 248 ms | Umbra |
| Session store bytes per step | 763 | 1,094 | 29 | Umbra, 30% fewer |
| Reasoning tokens, total | 338,858 | 315,434 | 25,289 | tie inside the band |
| Runs that hit the step or time limit | 0 | 1 | 1 | Umbra at 0, inside the band |
| Runs with a nonzero exit | 0 | 1 | 1 | Umbra at 0, inside the band |
| Session: resume after a crash | 3 of 3 | 2 of 3 | 100 points | Umbra at 100%, inside the band |
| Runs that reached the eval's own task files | 1 of 183 | 0 of 183 | 0 | pi |

A paired sign test over the 61 tasks finds Umbra better on 15, pi better on 5 and 41 tied
(p = 0.041). Every other row (no tool call, HTTP errors, malformed tool calls, requests over the
window, the other six session scenarios) is 0 or 100% for both.

### Pass rate by category (Umbra / pi)

| Category | Umbra | pi |
|---|---|---|
| hard | 78.3% | 66.7% |
| compaction | 92.6% | 44.4% |
| session | 100% | 95.2% |
| edit-precision | 93.3% | 100% |
| bugfix, create, instructions, large-output, navigate | 100% | 100% |

### What is not strictly better, and why

- **Reasoning tokens.** Inside pi's band. Two prompt changes aimed at it (E25, E27) did not move it
  beyond the band. The eval sets thinking on for both agents, so the thinking setting (E12) cannot
  be used in this comparison; its arms are reported separately.
- **Runs that reached the eval's own task files.** One hard tier run of json-cleanup-30 found the
  reference solution under `evals/tasks` in the checkout and ran it to check its own output. The
  eval fails that cell. The same task earlier sent runs across the disk looking for the original
  data after a buggy script overwrote it; E26 stopped that, and this run went looking for a way to
  verify instead. The eval's task files are readable from the workspace because the runner and the
  agent share one disk.
- **Three rows where Umbra is at the best possible value** (0 limit hits, 0 error exits, 100% on
  resume after a crash) are listed because pi's noise band is as wide as the whole gap, so no
  change to Umbra can make them strictly better.

## Experiments

The run by run numbers are in [LEDGER.md](LEDGER.md).

| Experiment | Change | Result |
|---|---|---|
| E1 | Send the streamed reasoning back | Kept, released in 0.7.2 |
| E2 | Umbra records its own timings and cache counts | Kept, released in 0.7.2 |
| E3 | Ask the model to check its work before the final reply | Reverted (setting `finish_check`, off) |
| E4 | System prompt lines about line endings, encodings and Windows paths | Kept. edit-precision 15 of 15 |
| E5 | An `edit` tool beside `bash` | Not adopted (setting `edit_tool`, off) |
| E6 | Save very long command output to a file | Reverted (setting `output_spill`, off). Prompt tokens per passed task rose 32% |
| E7 | Stall message, tool call repair, rejected reply kept | Reverted. The error exits it targeted were replies cut off at the output limit, which E13 fixed |
| E8 | Compaction settings for a smaller working window | Kept, with no cap by default |
| E9 | A 120 second default command timeout | Kept |
| E10 | The shell state carries from one command to the next | Kept, on by default. Requests and completion tokens per passed task fell beyond pi's band |
| E11 | Append only session journal and `--at` to branch | Kept. branch and cut-resume 3 of 3 |
| E12 | A thinking setting for local models | Not adopted, setting unset. Thinking off: hard tier 40.0% against 81.7%, 8 times the requests per passed task. Thinking on the first request only: 37.1% |
| E13 | Ask a local server for 16,000 reply tokens | Kept. Hard tier 58.3% to 80.0%, error exits 3 to 0 |
| E14 to E16 | Session index at turn ends, server timings kept once, no second trajectory write | Kept. 6,601 to 1,922 bytes per step |
| E17 to E22 | The journal is the session store; compact journal records; stats as differences | Kept. 763 bytes per step in the final run |
| E23 | Keep at most 1 MB from each end of a command's output | Kept. One run had kept 420 MB |
| E24 | Stay inside the working directory | Reverted |
| E25 | Short thinking on routine steps | Kept |
| E26 | Copy the originals before a script edits many files in place | Kept |
| E27 | A firmer short thinking paragraph | Reverted |

The eval itself changed in task-2643: the store bytes counter reads the final write count after the
process exits (the old poll missed pi's last 259 KB write and reported pi at 190 bytes per step),
Umbra runs without `-o` so every file it writes counts, and long-300-resume counts tool calls as
steps.

The PC crashed once during the work (2026-11-14 04:13Z, bugcheck 0x20001, with corrected PCIe errors
on the root port above GPU 1 throughout). The eval was restarted after the reboot; finished cells
were kept.
