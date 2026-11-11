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

## How a change is kept

From the TDD's hill climb protocol: one change per commit, built with
`cargo build --release` in the worktree and passed as `UMBRA_BIN`. The target categories run 3
reps. A change whose target metric does not improve by more than pi's noise band for that metric
is reverted. A change that improves its target is kept if no row of the full scorecard gets worse
by more than its noise band.

## Experiments

| Id | Commit | Hypothesis | Target metric | Before | After | Result | Run ids |
|---|---|---|---|---|---|---|---|
