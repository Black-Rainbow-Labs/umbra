# Models

Umbra talks to two kinds of API, plus a scripted one for tests.

| Kind | API | Used for |
|---|---|---|
| `anthropic` | Anthropic Messages API, `POST /v1/messages` | Claude |
| `openai` | OpenAI Chat Completions, `POST /chat/completions` | llama.cpp, Ollama, vLLM, LM Studio, OpenAI, OpenRouter, and anything else with that API |
| `replay` | none | Returns replies from a JSON file, for tests and demos |

Choose a model with `-m provider/model`. The provider is a name from the table below or from your
config.

| Provider | Kind | Base URL | Key read from |
|---|---|---|---|
| `anthropic` | anthropic | `https://api.anthropic.com` | `ANTHROPIC_API_KEY`, then your Claude Code login |
| `openai` | openai | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| `openrouter` | openai | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| `local` | openai | `http://localhost:8080/v1` | `LOCAL_API_KEY` (usually unset) |
| `ollama` | openai | `http://localhost:11434/v1` | `OLLAMA_API_KEY` (usually unset) |
| `replay` | replay | none | none; the model name is the path to the script |

## Claude

```bash
umbra -m anthropic/claude-haiku-5-5 "..."
umbra -m claude-sonnet-5-5 "..."      # a bare claude- name means the anthropic provider
```

Umbra finds a credential in this order:

1. `api_key` or the variable named by `api_key_env` in the provider config (`ANTHROPIC_API_KEY`
   by default), sent as `x-api-key`.
2. `ANTHROPIC_AUTH_TOKEN`, sent as a bearer token.
3. The login Claude Code saved in `~/.claude/.credentials.json`, sent as a bearer token with the
   `oauth-2025-04-20` beta header. Requests made with this login must start their system prompt
   with Claude Code's identity line, and Umbra adds it. Umbra only reads this file. It never
   refreshes the token, because a refresh replaces the refresh token Claude Code depends on. When
   the token has expired, run `claude` once to refresh it. On macOS, Claude Code keeps its login
   in the Keychain instead of this file, so set `ANTHROPIC_API_KEY` there.

Set `auth = "api_key"` to use only a key, or `auth = "claude_code"` to use only the Claude Code
login.

What Umbra sends:

- `max_tokens` 16,000 unless `max_tokens` is set.
- Top level `cache_control: {"type": "ephemeral"}`, so the API caches the conversation so far on
  every call. The trajectory shows it working: the second call of a run reads the first call's
  tokens from the cache.
- No `temperature` unless you set one. Current Claude models reject any value other than the
  default.
- `output_config.effort` when `effort` is set.
- Each assistant reply goes back exactly as the API returned it, thinking blocks included,
  because current models reject a changed thinking block.

## Local models with llama.cpp

Start `llama-server` with tool calling turned on (`--jinja`), then:

```bash
umbra -m local/qwen "..."
```

The model name after `local/` is sent as `model`. llama.cpp serves one model and ignores the
name; some servers in front of it need the exact name from `GET /v1/models`. If the model does not
do tool calling well, use text mode:

```bash
umbra -m local/qwen --text "..."
```

To point at a different server, add a provider:

```toml
[providers.box]
kind = "openai"
base_url = "http://192.168.1.20:8080/v1"
max_tokens = 8192
extra_body = { chat_template_kwargs = { enable_thinking = false } }
```

`extra_body` fields are added to every request, which is how to pass server specific options.

## Provider settings

| Setting | Meaning |
|---|---|
| `kind` | `anthropic`, `openai` or `replay` |
| `base_url` | The API's base URL |
| `api_key_env` | The environment variable holding the key |
| `api_key` | The key itself. Prefer `api_key_env`, so the key is not stored in a file. |
| `auth` | Anthropic only: `auto`, `api_key` or `claude_code` |
| `headers` | Extra request headers |
| `max_tokens` | The reply length limit |
| `temperature` | Only sent when set |
| `effort` | Anthropic only: `low`, `medium`, `high`, `xhigh` or `max` |
| `input_cost_per_mtok`, `output_cost_per_mtok` | Dollars per million tokens, for a model Umbra has no price for |
| `extra_body` | Fields added to every request body |
| `max_retries` | Retries after the first attempt (default 9) |
| `stream` | OpenAI compatible servers only: stream replies (default true). Set it to false for a server that rejects `stream` or `stream_options`. |
| `replay_file` | Replay only: the script file |

## Streaming and Ctrl+C

Every request runs on a worker thread, and Umbra checks for Ctrl+C every 100 ms while it waits, so
an interrupt never waits for the model to finish.

OpenAI compatible servers are streamed by default (`"stream": true` with
`"stream_options": {"include_usage": true}`). Umbra joins the chunks, tool call deltas included, into
the same reply a whole response would have given, so the trajectory looks the same either way. When
you press Ctrl+C, Umbra drops the stream, which closes the connection at the next chunk, and
llama.cpp and NInfer stop generating. Without streaming the server would keep generating the
abandoned reply, and a server with one slot would make the next request wait for it.

Anthropic requests are not streamed. On Ctrl+C Umbra stops waiting at once, but the API finishes the
abandoned reply, up to `max_tokens`, and its cost is not added to the run's total.

## Images

| Provider | A user message with images | A tool result with images |
|---|---|---|
| `anthropic` | `image` blocks with a base64 `source`, then the text | `tool_result.content` holds the text block and the image blocks |
| `openai` | `image_url` parts with `data:` URLs, then the text | The `tool` message keeps the text. A user message with every image of that step, starting `Images returned by the tools above:`, follows the last tool message, because a `tool` message can only hold text. |

A llama.cpp server reads images only when it runs with a vision projector (`--mmproj`). NInfer's
gateway on this machine reads them. A server that cannot read images answers the request with an
error, and the run ends with `ModelError`.

## Retries

A call that fails with HTTP 408, 409, 429 or 5xx, or never reaches the server, is retried after 4,
8, 16, 32 seconds and then every 60 seconds, up to 10 attempts in all. These are mini-swe-agent's
numbers. Ctrl+C during a wait stops the retries. Authentication errors, bad requests and unknown models fail at once. A run whose model
call still fails ends with the exit status `ModelError`.

## Cost

Cost is worked out from the token counts in each reply and a price table, in dollars per million
tokens:

| Model | Input | Output | Cache read | Cache write |
|---|---|---|---|---|
| `claude-fable-5-1` | 10.00 | 50.00 | 0.25 | 12.50 |
| `claude-opus-5-5` | 4.00 | 20.00 | 0.20 | 5.00 |
| `claude-sonnet-5-5` | 2.00 | 10.00 | 0.20 | 2.50 |
| `claude-haiku-5-5` | 0.10 | 0.50 | 0.01 | 0.125 |

Older Claude models are in the table too (`src/services/providers/pricing.rs`). A model with no
price, such as a local one, costs 0. Set `input_cost_per_mtok` and `output_cost_per_mtok` to track
the cost of any other paid model.

## Replay scripts

A replay script is a JSON array. Each entry is the model's next reply: a string for text mode, or
an object with `content` and `tool_calls`.

```json
[
  { "content": "Looking around.", "tool_calls": [{ "name": "bash", "arguments": { "command": "ls" } }] },
  { "content": "Done.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
]
```

```bash
umbra -m replay/script.json -y "anything"
```

An object can also have `"sleep_ms"`, which makes that call wait like a slow model; Ctrl+C ends the
wait. `{"error": {"status": 400, "body": "prompt is too long"}}` makes the call fail the way a server
would. With `UMBRA_REPLAY_RECORD=<file>` every request is added to the file as one JSON line, with
each message's images listed by type, size and source instead of their data.
