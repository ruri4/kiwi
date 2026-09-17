# kiwi

A minimal Rust terminal client for OpenAI-compatible streaming chat APIs, with a `ratatui` UI, YAML endpoint profiles, model discovery, and persistent sessions.

## setup

Requires Rust 1.89+ and an OpenAI-compatible API exposing `GET /models` and `POST /chat/completions`.

Copy `config.example.yaml` to `$XDG_CONFIG_HOME/kiwi/config.yaml` (or `~/.config/kiwi/config.yaml`), or pass a path with `--config PATH` / the `KIWI_CONFIG` environment variable. The config file is required.

```yaml
default_endpoint: local
reasoning_effort: auto   # auto | none | low | medium | high
endpoints:
  - name: local
    base_url: http://localhost:8000/v1
    default_model: example-model

  - name: hosted
    base_url: https://api.example.invalid/v1/
    api_key_env: HOSTED_LLM_API_KEY
    default_model: hosted-model
```

`base_url` is the OpenAI API base, not the full chat URL. API keys are never stored in YAML: `api_key_env` names an environment variable (or a private `.env` entry) holding the bearer token. Omit it for unauthenticated endpoints.

```sh
HOSTED_LLM_API_KEY='replace-with-your-token' cargo run --release
```

## keys

| key | action |
| --- | --- |
| `Enter` | Send message |
| `Alt+Enter` | Insert newline |
| `PageUp` / `PageDown` | Scroll transcript |
| `Ctrl+S` | Session browser |
| `Ctrl+E` / `Ctrl+M` | Endpoint picker / model picker (retry discovery) |
| `Ctrl+R` | Cycle reasoning effort |
| `Ctrl+C` / `Ctrl+Q` | Exit |

In the session browser: `n` new session, `r` rename, `d` delete (confirm with `y`), `Enter` switch, `Escape` cancel/close.

## notes

- Models are discovered via `GET <base_url>/models`; `Ctrl+M` retries discovery.
- `AGENTS.md` files from the launch directory and its ancestors are injected into each request's user content; the explicit prompt always takes precedence.
- Sessions persist as versioned JSON in `$XDG_DATA_HOME/kiwi/sessions.json` (or `~/.local/share/kiwi/sessions.json`). Transcripts are stored; base URLs and API keys never are.
- Reasoning output is transcript-only and never sent back to the model.
