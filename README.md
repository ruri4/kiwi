# llm-chat

A minimal Rust terminal chat client for a Modal-hosted, OpenAI-compatible streaming endpoint. It sends the full successful conversation with each request and renders incoming answer and reasoning SSE deltas live in a `ratatui` UI.

## prerequisites

- Rust with Rust 2024 edition support (Rust 1.85 or newer)
- a terminal supported by `crossterm`
- the exact URL of an OpenAI-compatible chat-completions endpoint

Modal itself permits custom web payloads. This client specifically assumes the contract used by current Modal LLM/vLLM endpoints: OpenAI-compatible `POST /v1/chat/completions` requests with streaming SSE responses. It does not implement other Modal-specific protocols.

## build and run

```sh
cargo build --release

LLM_CHAT_ENDPOINT='https://example--my-app.modal.run/v1/chat/completions' \
  cargo run --release
```

Select a model on the command line or through the environment:

```sh
cargo run --release -- \
  --endpoint 'https://example--my-app.modal.run/v1/chat/completions' \
  --model 'my-model'
```

For an authenticated endpoint, keep the bearer token in the environment rather than shell history:

```sh
LLM_CHAT_ENDPOINT='https://example--my-app.modal.run/v1/chat/completions' \
LLM_CHAT_API_KEY='replace-with-your-token' \
  cargo run --release
```

You can also put configuration in an optional `.env` file:

```dotenv
LLM_CHAT_ENDPOINT=https://example--my-app.modal.run/v1/chat/completions
LLM_CHAT_MODEL=my-model
LLM_CHAT_API_KEY=replace-with-your-token
LLM_CHAT_REASONING_EFFORT=auto
```

Values already set in the process environment take precedence over `.env`. Explicit `--endpoint`, `--model`, and `--reasoning-effort` options take precedence over both environment sources.

## configuration

| option / variable | required | meaning |
| --- | --- | --- |
| `--endpoint URL` / `LLM_CHAT_ENDPOINT` | yes | Exact chat-completions URL. The option takes precedence over the environment fallback. |
| `--model MODEL` / `LLM_CHAT_MODEL` | no | Model identifier sent in the request. Defaults to `llm`; the option takes precedence. |
| `--reasoning-effort EFFORT` / `LLM_CHAT_REASONING_EFFORT` | no | `auto`, `none`, `low`, `medium`, or `high`. Defaults to `auto`; the option takes precedence. `auto` omits the request property so model/server defaults remain unchanged. |
| `LLM_CHAT_API_KEY` | no | Bearer token. There is deliberately no command-line equivalent, and the value is never displayed or logged. |

Run `cargo run -- --help` for concise command-line help.

## keys

| key | action |
| --- | --- |
| `Enter` | Send a non-empty message when no request is active |
| `Alt+Enter` | Insert a newline |
| `Left` / `Right` | Move by one Unicode scalar value |
| `Home` / `End` | Move to the start / end of the current input line |
| `Backspace` / `Delete` | Delete before / under the cursor without splitting UTF-8 |
| `PageUp` / `PageDown` | Scroll the transcript |
| `Ctrl+R` | Cycle reasoning effort while idle: `auto` → `none` → `low` → `medium` → `high` → `auto` |
| `Ctrl+C` / `Ctrl+Q` | Exit |

`Ctrl+R` is ignored while a request is active. Each request captures the effort shown in the header when it starts. Explicit efforts are sent as the top-level OpenAI/vLLM `reasoning_effort` property; endpoints and models differ in which effort levels they support.

The stream decoder reads final answer text from `choices[0].delta.content` and reasoning from `choices[0].delta.reasoning_content` or `choices[0].delta.reasoning`. Reasoning is shown as a muted, transcript-only section and is never included in later conversation history. Reasoning display depends on the endpoint and model emitting one of those supported delta fields.

`Alt+Enter` depends on the terminal emulator reporting that modified key distinctly. OpenAI tool calls, images, usage chunks, and Markdown rendering are intentionally outside the client's scope.
