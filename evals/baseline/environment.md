# Environment report for the Umbra vs pi eval harness (task-2614)

Date of checks: 2026-11-10. All commands below were run and worked unless marked otherwise.

## 1. The local model server

The server on port 8080 is NOT llama.cpp. It is NInfer, a separate inference engine, behind a gateway.
The Service Manager entry "Llama.cpp Server" is stopped (desiredStatus stopped, no port). Its command file
documents the old llama.cpp launch line, but that service shares the GPU and the port 8080 with the gateway,
so it is not what answers requests today.

What runs (Service Manager, http://localhost:4000/api/services):

| Service | Port | Command |
|---|---|---|
| NInfer Gateway (id 05dd4f70f6984081a8bddd8b4ad4b51e) | 8080 | `node C:\jason\dev\ai-service\tools\ninfer\ninfer-gateway.cjs --port 8080 --upstream-port 8086 --merged-adapters "loras\ui-agent-v9-coletti-adapter-f16.gguf:1"` |
| NInfer (Qwen 3.8 27B) (id cmt1moh8d0023kixvvou3lohy) | 8086 | `wsl -d Ubuntu bash -lc "cd $HOME/ninfer && CUDA_VISIBLE_DEVICES=1 exec ./build/apps/ninfer-serve $HOME/ninfer-models/ninfer-qwen3.8-27b-coletti-v9.ninfer --model-id ninfer-qwen3.8-27b-coletti-v9 --host 0.0.0.0 --port 8086 --device 0 --max-context 262144 --kv-capacity auto --max-concurrency 1 --kv-dtype int8 --spec mtp --draft-tokens 3 --lm-head-draft --vision --log-stats-interval-ms 0"` |

- Model name for requests: `ninfer-qwen3.8-27b-coletti-v9` (abliterated Qwen 3.8 27B, JonathanColetti, ui-agent-v9 merged into the weights, about 5.3 bit groupwise int quantization, INT8 KV cache, MTP speculative decoding with 3 draft tokens, vision on).
- Model file: `~/ninfer-models/ninfer-qwen3.8-27b-coletti-v9.ninfer` inside WSL Ubuntu.
- Context: `n_ctx` 262144 (`/props` and `max_model_len` in `/v1/models`).
- Parallel slots: `total_slots` 1, `--max-concurrency 1`. One caller at a time.
- GPU: the card is chosen with `CUDA_VISIBLE_DEVICES=1`, the second RTX 5090. nvidia-smi: GPU 0 19202 of 32607 MiB used (ComfyUI), GPU 1 28007 of 32607 MiB used (NInfer).
- `--jinja`: not applicable. NInfer is not llama.cpp. The chat template (reasoning effort defaults to low) is stored in the model artifact.
- Sampling defaults: no `--temp`/`--top-p`/`--top-k` flags on the NInfer command, so the defaults live inside the NInfer artifact and are not visible from here. The old llama.cpp line used temp 0.6, top-p 0.95, top-k 20. Set sampling per request if the harness needs it fixed.
- Reasoning: the model reasons by default. Responses carry `reasoning_content` and `usage.completion_tokens_details.reasoning_tokens`.

Query results:

```
curl -s http://localhost:8080/v1/models
{"data":[{"created":1794371218,"id":"ninfer-qwen3.8-27b-coletti-v9","max_model_len":262144,"object":"model","owned_by":"ninfer"}],"object":"list"}

curl -s http://localhost:8080/props
{"default_generation_settings":{"n_ctx":262144},"n_ctx":262144,"total_slots":1,"model_path":"ninfer-qwen3.8-27b-coletti-v9","engine":"ninfer"}

curl -s http://localhost:8080/health
{"status":"ok"}

curl -s http://localhost:8080/slots
(empty body; the gateway and NInfer do not serve /slots, and the old llama.cpp line used --no-slots too)
```

Busy check: there is no slot endpoint, so busy state cannot be read directly. Other agents on this machine (the Pi, Umbra, Codex and Opencode terminal daemons are running) also use port 8080. Before each eval run, send a tiny request and check the latency. Do not run two callers at once.

Tiny request (max_tokens 16), the response includes both `usage` and `timings`:

```
curl -s http://localhost:8080/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"ninfer-qwen3.8-27b-coletti-v9","messages":[{"role":"user","content":"Say hi"}],"max_tokens":16}'
```
```
{"choices":[{"finish_reason":"length","index":0,"logprobs":null,"message":{"content":"Hi!","reasoning_content":"The user wants me to say hi. Simple greeting.\n","refusal":null,"role":"assistant"}}],"created":1794371239,"id":"chatcmpl-f772b9be55a8afd1","model":"ninfer-qwen3.8-27b-coletti-v9","object":"chat.completion","timings":{"cache_n":0,"draft_n":15,"draft_n_accepted":9,"predicted_ms":87.846852,"predicted_n":16,"predicted_per_second":170.75170775612995,"predicted_per_token_ms":5.8564568,"prompt_ms":1606.7116740000001,"prompt_n":42,"prompt_per_second":26.140346572225127,"prompt_per_token_ms":38.25503985714286},"usage":{"completion_tokens":16,"completion_tokens_details":{"reasoning_tokens":13},"prompt_tokens":42,"prompt_tokens_details":{"cached_tokens":0},"total_tokens":58}}
```

So: `usage.prompt_tokens`, `usage.completion_tokens`, `usage.prompt_tokens_details.cached_tokens`,
`usage.completion_tokens_details.reasoning_tokens` and a `timings` object (`cache_n`, `prompt_n`, `prompt_per_second`,
`predicted_n`, `predicted_per_second`, `draft_n`, `draft_n_accepted`) are all present. The prompt cache works: in the Umbra run the
second request reported `cached_tokens` 18172 of 18320 prompt tokens.

## 2. Umbra

- `umbra --version` prints `umbra 0.5.2`. Binary: `C:\Users\jason\.cargo\bin\umbra.exe`, built 2026-11-10 19:34.
- Git commit of the binary: the binary does not print one. Source checkout `C:/jason/dev/umbra` has tag `v0.5.2` at commit `25ae972` (Release 0.5.2). HEAD of main is `e563f7d` (a TDD commit after the release). Assume the installed build is v0.5.2 / 25ae972 unless a later reinstall happened.
- Config: `C:\Users\jason\.umbra\config.toml`. Entire content (it has no keys):
  ```
  model = "anthropic/claude-haiku-5-5"
  ```
  Everything else comes from built in defaults (`umbra config` prints the merged config). The built in provider `local` is
  `kind = "openai"`, `base_url = "http://localhost:8080/v1"`, with no key needed. So the local model is selected on the command line as
  `-m local/ninfer-qwen3.8-27b-coletti-v9`. Other built in providers: anthropic, ollama, openai, openrouter, replay.
- Other folders under `C:\Users\jason\.umbra`: `sessions` (one small JSON index per run), `trajectories` (default trajectory output), `logs/mcp`, `task-sessions`.
- Non interactive mode: yes. `umbra [OPTIONS] TASK_WORDS...` with `--yolo` (run every action without asking), `--exit-immediately` (exit when the agent finishes), `-o <file>` (trajectory JSON), `--cwd <dir>`, `--step-limit`, `--cost-limit`, `--no-mcp`, `--no-skills`, `--no-instructions`, `--no-source <harness>`, `--docker <image>`, `--finish reply|submit`, `--text`.
- Important for fairness: by default Umbra loads `C:\Users\jason\.claude\CLAUDE.md` and 43 skills into the system prompt. The first request was 18,177 prompt tokens. pi's first request was 9,750 tokens. Use `--no-skills --no-instructions --no-mcp` (or `--no-source claude,agents,...`) for a controlled comparison. `umbra prompt "task"` prints the exact system prompt and first user message without running anything.
- MCP: an MCP server named nikaya failed to start (30 second timeout) on the first run, which added 30 seconds. Use `--no-mcp`.

Smoke run, with MCP (first run):

```
umbra -m local/ninfer-qwen3.8-27b-coletti-v9 --yolo --exit-immediately --cwd C:/jason/dev/umbra/.claude/worktrees/task-2614/_agent_output/task-2614/smoke/umbra -o C:/jason/dev/umbra/.claude/worktrees/task-2614/_agent_output/task-2614/smoke/umbra-traj.json "Create a file hello.txt containing the word hello, then stop."
```
Worked. hello.txt created with `hello` (5 bytes). Wall time 46 s (30 s waiting on the nikaya MCP server, agent time 12 s, 2 steps).

Smoke run, without MCP (second run, 13.1 s wall, 10 s agent time, 2 steps, exit code 0):

```
umbra -m local/ninfer-qwen3.8-27b-coletti-v9 --yolo --exit-immediately --no-mcp --cwd <smoke/umbra> -o <smoke/umbra-traj2.json> "Create a file hello2.txt containing the word hello, then stop."
```

Where output ends up:
- Trajectory JSON (the file named with `-o`): `_agent_output/task-2614/smoke/umbra-traj2.json`. Without `-o` it goes to `C:\Users\jason\.umbra\trajectories\<session id>.traj.json`.
- Session index: `C:\Users\jason\.umbra\sessions\<session id>.json` (id, trajectory path, cwd, model, task, api_calls, cost, exit_status, message count).
- Token usage: `info.model_stats` in the trajectory: `{"instance_cost":0,"api_calls":2,"input_tokens":36497,"output_tokens":469,"compactions":0}`. Per call usage is in `messages[i].extra.response.usage`, for example
  `{"completion_tokens":45,"completion_tokens_details":{"reasoning_tokens":30},"prompt_tokens":18320,"prompt_tokens_details":{"cached_tokens":18172},"total_tokens":18365}`.
  The trajectory also has `info.exit_status` ("Replied"), `info.submission`, and `info.umbra_version`.
- Console output ends with "Done, 2 steps, 12s".

## 3. pi

- The installed pi is `@earendil-works/pi-coding-agent` 1.3.0, global, at `C:\nvm4w\nodejs\node_modules\@earendil-works\pi-coding-agent` (`pi --version` prints `1.3.0`, binary `C:\nvm4w\nodejs\pi`). npm latest of that package is 1.5.2.
- `@mariozechner/pi-coding-agent` is the old name. npm marks it deprecated ("please use @earendil-works/pi-coding-agent instead going forward"), its last version is 0.73.1. I did NOT install it. The harness should use the `@earendil-works` package (the Pi Terminal Daemon service uses the installed one). If you want the 1.5.2 release, install locally with `npm install --prefix <dir> @earendil-works/pi-coding-agent`.
- Global npm packages: pi-coding-agent 1.3.0, @nestjs/cli 11.0.10, @openai/codex 0.153.4, @tauri-apps/cli 2.11.1, concurrently 9.2.1, corepack 0.36.0, deno 2.7.9, npm 11.19.1, opencode-ai 1.19.4, pnpm 9.15.9.
- Isolation: the environment variable is `PI_CODING_AGENT_DIR` (docs/environment-variables.md: "Override the config directory; default is ~/.pi/agent"). Sessions are stored under `<agent-dir>/sessions/<encoded cwd>/`. `--session-dir <dir>` and `--no-session` also exist. `--offline` (or `PI_OFFLINE=1`) disables startup network calls.
- The user's own config at `C:\Users\jason\.pi\agent` already has a `local-qwen` provider pointing at the same server. I copied its structure.

Config files written under `C:/jason/dev/umbra/.claude/worktrees/task-2614/_agent_output/task-2614/pi-home`:

`models.json`:
```json
{
  "providers": {
    "local-qwen": {
      "baseUrl": "http://localhost:8080/v1",
      "api": "openai-completions",
      "apiKey": "local",
      "models": [
        {
          "id": "ninfer-qwen3.8-27b-coletti-v9",
          "name": "Qwen 3.8 27B (local)",
          "reasoning": true,
          "contextWindow": 262144,
          "maxTokens": 16000,
          "compat": { "supportsDeveloperRole": false, "thinkingFormat": "qwen-chat-template", "supportsStore": false }
        }
      ]
    }
  }
}
```
(`apiKey` is a dummy value, the server needs none.)

`settings.json`:
```json
{
  "defaultProvider": "local-qwen",
  "defaultModel": "ninfer-qwen3.8-27b-coletti-v9"
}
```
pi also created `auth.json`, `models-store.json` and `sessions/` in that folder on first run.

Smoke command (PowerShell, because the working folder must be set and `cd` is not allowed):

```powershell
$W='C:\jason\dev\umbra\.claude\worktrees\task-2614\_agent_output\task-2614'
$env:PI_CODING_AGENT_DIR="$W\pi-home"
Start-Process -FilePath pi.cmd -ArgumentList '-p','--mode','json','--no-mcp','--no-context-files','--offline','"Create a file hello.txt containing the word hello, then stop."' -WorkingDirectory "$W\smoke\pi" -RedirectStandardOutput "$W\smoke\pi-events.jsonl" -RedirectStandardError "$W\smoke\pi-stderr.txt" -Wait -PassThru -NoNewWindow
```
Worked: exit 0, 4.4 s wall, hello.txt created containing `hello`, stderr empty. pi has no `--cwd` flag, it uses the process working directory, so the harness must set the working directory when it spawns pi. (My first attempt ran in the wrong folder and created a stray hello.txt in the worktree root. I deleted that file.)

JSON event stream (`smoke/pi-events.jsonl`, 49 lines): `session` header, then `agent_start`, `turn_start`, `message_start/end` (user), assistant `message_start`, many `message_update` (thinking_delta, text_delta, toolcall_*), `message_end`, `tool_execution_start/end`, tool result message, `turn_end`, a second turn, `agent_end`, `agent_settled`. Session header example:
`{"type":"session","version":3,"id":"01a1c8dd-...","timestamp":"2026-11-11T04:28:48.269Z","cwd":"C:\\jason\\dev\\umbra\\...\\smoke\\pi"}`.

Token usage is in `turn_end.message.usage` (and `message_end.message.usage`, and a running `usage` on each `message_update`). One usage record (turn 2):
```json
{"input":53,"output":28,"cacheRead":9840,"cacheWrite":0,"reasoning":15,"totalTokens":9921,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
```
Turn 1: `{"input":9750,"output":91,"cacheRead":0,"cacheWrite":0,"reasoning":20,"totalTokens":9841,...}`. `turn_end.message` also has `model`, `provider`, `stopReason` (`toolUse`, then `stop`). Note pi's `input` excludes cached tokens (`cacheRead` is separate), unlike the server's `prompt_tokens` and Umbra's `input_tokens`, which include them. Sum `input + cacheRead` to compare. Tool timing is in `tool_execution_end.durationMs`. pi's own session file is under `pi-home\sessions\--<encoded cwd>--\`.

## 4. Versions

- Umbra 0.5.2 (tag v0.5.2, commit 25ae972 in the source checkout; the binary itself has no commit string).
- pi 1.3.0 (`@earendil-works/pi-coding-agent`).

## 5. Tooling

- Node v24.22.1 (nvm4w). npm 11.19.1.
- Python 3.13.11 as `python`. `python3` is NOT on PATH in Git Bash (use `python`).
- Docker: client 29.4.0, API 1.54 (`docker version` printed the Client section, the Server section was cut off by my `head -5`, so verify with `docker info` that the daemon is running before depending on it). Umbra supports `--docker <image>`.
- Git Bash exists at `C:\Program Files\Git\bin\bash.exe` and is what Umbra uses for its shell.

## Notes for the harness

1. The server is single slot. Run one agent at a time. Both agents showed good prompt cache reuse across turns of the same run.
2. Reasoning tokens are reported separately by all three (server, Umbra, pi).
3. Equalize the prompts: Umbra's default prompt is about twice pi's because of CLAUDE.md and skills. Use `--no-skills --no-instructions --no-mcp` for Umbra and `--no-context-files --no-skills --no-mcp` for pi.
4. Both agents write hello.txt in about 10 to 13 s (Umbra) and 4.4 s (pi) on this trivial task. The difference is mostly the prompt size and Umbra's startup, not a quality measurement.
