# llm-chat

A minimal Rust terminal client for OpenAI-compatible streaming chat APIs. It supports named YAML endpoint profiles, model discovery, persistent sessions, and live answer/reasoning SSE rendering in a `ratatui` UI.

## prerequisites

- Rust with Rust 2024 edition support (Rust 1.89 or newer)
- a terminal supported by `crossterm`
- an OpenAI-compatible API exposing `GET /models` and `POST /chat/completions`

## YAML configuration

Copy `config.example.yaml` to the platform configuration directory as `config.yaml`, or select a file with `--config PATH` / the process environment variable `LLM_CHAT_CONFIG`. On Linux the default is normally `$XDG_CONFIG_HOME/llm-chat/config.yaml`, or `~/.config/llm-chat/config.yaml` when `XDG_CONFIG_HOME` is unset. The selected YAML file is required.

```yaml
default_endpoint: local
reasoning_effort: auto
endpoints:
  - name: local
    base_url: http://localhost:8000/v1
    default_model: example-model

  - name: hosted
    base_url: https://api.example.invalid/v1/
    api_key_env: HOSTED_LLM_API_KEY
    default_model: hosted-model
```

Each `base_url` is the OpenAI API base, not the complete chat URL. A trailing slash is optional. The client safely derives `<base_url>/models` and `<base_url>/chat/completions`. Endpoint names must be unique and nonblank, `default_endpoint` must name a profile, and unknown YAML fields are rejected.

`reasoning_effort` accepts `auto`, `none`, `low`, `medium`, or `high`, and defaults to `auto` when omitted. Endpoint URLs, default models, and reasoning settings belong to YAML; they have no command-line or environment overrides.

`api_key_env` names an environment variable containing the bearer token. Tokens are never accepted in YAML or on the command line, displayed, logged, or persisted. Omit `api_key_env` for an unauthenticated endpoint. Names must be portable shell-style identifiers, starting with an ASCII letter or underscore and continuing with ASCII letters, digits, or underscores. A named variable that is missing, empty, or not valid Unicode is a startup error. Process environment values take precedence over values loaded from the optional `.env` file.

Run with the default configuration:

```sh
HOSTED_LLM_API_KEY='replace-with-your-token' cargo run --release
```

Or choose another file:

```sh
EXAMPLE_LLM_API_KEY='replace-with-your-token' \
  cargo run --release -- --config ./config.example.yaml
```

The optional `.env` file is read privately, without loading its values into the process environment. Only variables named by a YAML profile's `api_key_env` are used as credentials; endpoint, model, reasoning, and configuration entries there are ignored. Keys are sent to the selected API as Bearer authentication.

```dotenv
HOSTED_LLM_API_KEY=replace-with-your-token
```

Values already set in the process environment take precedence over `.env`. A missing `.env` is allowed, but a referenced key must be present and non-empty in one of those sources.

## options

| option / variable | required | meaning |
| --- | --- | --- |
| `--config PATH` / process `LLM_CHAT_CONFIG` | no | YAML configuration file. Defaults to the required platform `config.yaml`. |
| profile `api_key_env` variable | no | Bearer token for that YAML endpoint profile, read from the process environment or private `.env` data. |

Run `cargo run -- --help` for concise command-line help.

## discovery and selection

The active endpoint's models are discovered with an authenticated `GET <base_url>/models`. Startup discovery runs asynchronously so the terminal renders immediately. IDs from the standard `{ "data": [{ "id": "..." }] }` response are sorted and deduplicated.

At startup, a saved model or YAML default is retained when it appears in discovery; if neither is available, the first discovered model is selected. Discovery failure is shown in the header and does not erase a usable current/default model. Sending is disabled only when no model is available. Press `Ctrl+M` to retry discovery when the model list is unavailable.

Endpoint and model selectors are idle-only and only one modal can be open. Switching endpoints starts discovery for the selected profile.

## keys

| key | action |
| --- | --- |
| `Enter` | Send a non-empty message when no request/discovery is active and a model is available |
| `Alt+Enter` | Insert a newline |
| `Left` / `Right` | Move by one Unicode scalar value |
| `Home` / `End` | Move to the start / end of the current input line |
| `Backspace` / `Delete` | Delete before / under the cursor without splitting UTF-8 |
| `PageUp` / `PageDown` | Scroll the transcript |
| `Ctrl+S` | Open or close the session browser while no chat request is active |
| `Ctrl+E` | Open the endpoint picker while idle |
| `Ctrl+M` | Open the model picker, or retry discovery, while idle |
| `Ctrl+R` | Cycle reasoning effort: `auto` → `none` → `low` → `medium` → `high` → `auto` |
| `Ctrl+C` / `Ctrl+Q` | Exit |

The endpoint/model pickers use `Up` / `Down`, `Enter`, and `Escape`.

### session browser

| key | action |
| --- | --- |
| `Up` / `Down` | Move the selected session |
| `Enter` | Switch to the selected session |
| `n` | Create and switch to a new session |
| `r` | Rename the selected session; `Enter` saves a nonblank name and `Escape` cancels |
| `d` | Begin deleting the selected session; `y` confirms and `n` or `Escape` cancels |
| `Escape` | Cancel rename/deletion, or close the browser |

At least one session is always kept. New sessions are named `Untitled`; the first sent user message supplies an automatic title unless manually renamed first.

## persistence and security

Sessions are saved as versioned JSON in the platform-standard local data directory. On Linux this is `$XDG_DATA_HOME/llm-chat/sessions.json`, or `~/.local/share/llm-chat/sessions.json`. Parent directories use restrictive permissions and updates use an atomic sibling rename.

The store contains session names, displayed transcripts (including reasoning and errors), and the latest YAML endpoint profile name/model identifier. It never contains base URLs or API keys. Version 1 stores are migrated in memory and written as version 2 on the next save. Missing saved profiles/models fall back deterministically to the YAML default and discovery results. Malformed or unsupported stores stop startup rather than replacing data.

The stream decoder reads answer text from `choices[0].delta.content` and reasoning from `choices[0].delta.reasoning_content` or `choices[0].delta.reasoning`. Reasoning is transcript-only and never included in later model context. OpenAI tool calls, images, usage chunks, and Markdown rendering are outside the client's scope.
