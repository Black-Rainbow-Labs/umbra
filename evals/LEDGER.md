# Hill climb ledger (task-2617)

Every experiment from `tasks/umbra-hillclimb-tdd.md`, kept or reverted, with the scorecard rows
before and after and the run ids. Run folders are under `_agent_output/task-2617/eval-runs/` in
the worktree (gitignored). All runs use `ninfer-qwen3.8-27b-coletti-v9` behind the NInfer gateway
on port 8080, the sampling the proxy fixes (temperature 0.6, top_p 0.95, top_k 20, thinking on),
and pi 1.3.0 as shipped.

## Reference and baseline

| Run id | What | Reps | Tasks |
|---|---|---|---|
| `pi-1.3.0-ref` | pi 1.3.0, the fixed reference for every later run | 3 | all 61 |
| `baseline-0.6.2` (task-2614) | Umbra 0.6.2 and pi 1.3.0, the "before" for E1 | 1 | the first 33 |

The separate Umbra 0.6.2 run on the 61 task suite was not made: the task-2614 scorecard is the
"before" row for E1, and the E1 run is the baseline for every later experiment.

## Hard tier calibration

The first version of the 20 hard tasks was too easy. In `pi-1.3.0-ref` pi passed 48 of 60 cells
(80%), and passed 14 tasks 3 times out of 3: csv-report-eight-reqs, eight-bugs-suite,
esm-migration-25, git-bisect-regression, json-cleanup-30, multi-file-currency,
newline-safe-generator, pagination-five-handlers, paths-with-spaces, python-cache-staleness,
shared-mutable-config, symptom-elsewhere-dates, tabs-makefile-tsv and utf8-bom-config. Those 14
were made harder (more scale, more stated and checked requirements) before any Umbra run touched
the hard tier, and pi's cells for them were run again. The other 6 tasks were kept as they were:
callback-to-promise 1 of 3, crlf-python-multi-edit 0 of 3, markdown-table-formatter 1 of 3,
rate-limiter-spec 2 of 3, refactor-keep-40-tests 1 of 3, stack-vm-spec 1 of 3.

One verifier fix: crlf-python-multi-edit failed pi 3 of 3 on `format_sku behavior`, because
`hidden/checks.py` asserted `format_sku("zz", "15") == "ZZ-000015"`. The prompt never says the
number may be a digit string, so that assert was removed (the check is otherwise unchanged) and
pi's cells for that task were run again. The other failed checks on that task were confirmed to
follow from the prompt.

## Session scenario fix

interrupt-continue killed the agent 20 seconds into its first phase. Both agents now finish all ten
files in 18 to 27 seconds, so the kill often came after the work was done and the cell failed with
"first phase was not interrupted" (pi 1 of 3 passed, E1 0 of 3). The trigger is now
`killWhenFiles: { dir: "out", count: 4 }`: the runner interrupts the agent when the fourth output
file appears. The verifier is unchanged. The old cells for both agents are in
`eval-runs/interrupt-continue-killafter20/`, and the task was run again for pi and E1.

## How a change is kept

From the TDD's hill climb protocol: one change per commit, built with
`cargo build --release` in the worktree and passed as `UMBRA_BIN`. The target categories run 3
reps. A change whose target metric does not improve by more than pi's noise band for that metric
is reverted. A change that improves its target is kept if no row of the full scorecard gets worse
by more than its noise band.

## Experiments

| Id | Commit | Hypothesis | Target metric | Before | After | Result | Run ids |
|---|---|---|---|---|---|---|---|
| E1 | c89a679 | Sending the streamed reasoning back keeps the server's prompt cache valid from one request to the next | Cache hit rate, uncached prompt tokens per passed task, time to first content | 10.3%, 137,820, 4,153 ms (0.6.2, task-2614) | 95.7%, 14,608, 319 ms (pi: 82.3%, 78,944, 1,084 ms) | Kept. All three targets met. Pass rate 82.5% against pi's 80.3% (inside pi's 6.6% noise band), compaction pass rate 96.3% against 44.4%, wall seconds per task 61.4 against 116.9. Not yet better than pi: hard tier pass rate (60.0% against 66.7%), reasoning tokens (+8.4%), nonzero exits (6 against 1), session branch and cut-resume (need E11), session store bytes per step. E2 token totals checked against the proxy here: input and output totals match exactly in all 126 cells | `e1-c89a679`, `pi-1.3.0-ref` |
| E2 | 9adf309 | Umbra can report its own timings, reasoning tokens and cached tokens, so the proxy is not needed to see the E1 result | Umbra's model_stats against the proxy | no cached or reasoning totals | calls, input, output and cached token totals match the proxy exactly in all 87 E3 cells (0% difference) | Kept. No scorecard change expected or seen | `e3-500b00b` |
| E3 | 500b00b | A final reply right after a file change, with no command run since, gets one message asking for the check that proves the change | Hard tier pass rate and mean score | 60.0%, 76.5% (E1) | 63.3%, 84.5% (pi 66.7%, 82.0%; pi noise band 10.0 points for both) | Reverted (583a32d). Both gains are inside the noise band, and prompt tokens rose 30% on the hard tier and 42% on create. The code stays as the `finish_check` setting, off by default. Later binaries were built with it on, so their runs pass `UMBRA_EVAL_CONFIG="finish_check = false"` | `e3-500b00b`, `e1-c89a679` |
| E4 | 1b771b1 | Stating in the system prompt that files keep their line endings and encoding, that Python opens files with `newline=''`, and how Git Bash paths map to Windows paths removes the byte safety failures | `edit-precision` score 100% at 3 reps, first request tokens below pi's | edit-precision 14 of 15 (93%), first request 1,644 tokens (E1 on the same 29 tasks) | edit-precision 15 of 15 (100%), first request 1,753 tokens (pi 15 of 15, 2,478 tokens) | Kept. Both targets met. On the same 29 tasks (hard, edit-precision, create) pass rate went from 71.3% to 75.9% and hard tier pass rate from 60.0% to 65.0% (pi 77.0% and 66.7%; both changes are inside pi's noise band). No row got worse by more than its noise band except first request tokens, +109, which the target allows. Finished on task-2643: the 41 hard cells run on task-2617 before the GPU crashes were kept and the other 46 cells were run on 2026-11-13 | `e4-1b771b1`, `e1-c89a679`, `pi-1.3.0-ref` |
| E5 | 58f7320 | An `edit` tool beside `bash` (exact unique match, line endings and encoding kept outside the match) beats `bash` alone with E4 | Arm A must beat arm B on `edit-precision` and hard tier pass rate, with requests per passed task not rising | Arm B (E4, bash only): edit-precision 15 of 15, hard tier 65.0%, 18 requests per passed task, first request 1,791 tokens | Arm A (`edit_tool = true`): edit-precision 15 of 15, hard tier 66.7%, 18 requests per passed task, first request 1,998 tokens (pi 66.7%, 17) | Not adopted. edit-precision is a tie at 100% and the hard tier gain of 1.7 points is inside pi's 10 point noise band, while the tool definition adds 207 tokens to every first request and prompt tokens per passed task rose 7.6%. The code stays as the `edit_tool` setting, off by default, so no later binary changes. 51 hard cells first failed to start a process while the machine was low on memory; they were moved to `start-failed/` and run again | `e5a-58f7320`, `e4-1b771b1`, `pi-1.3.0-ref` |
