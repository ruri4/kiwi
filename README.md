# kiwi

Minimal Rust TUI client for OpenAI-compatible streaming chat APIs.

## setup

Requires Rust 1.89+.

```sh
cp config.example.yaml ~/.config/kiwi/config.yaml
HOSTED_LLM_API_KEY='your-token' cargo run --release
```

```yaml
default_endpoint: local
endpoints:
  - name: local
    base_url: http://localhost:8000/v1
    default_model: example-model
```

`base_url` is the API root. `api_key_env` names an env variable (or `.env` entry) holding the bearer token; omit it for unauthenticated endpoints. Pass a custom config with `--config PATH` or `KIWI_CONFIG`.

## keys

| key | action |
| --- | --- |
| `Enter` / `Alt+Enter` | Send / newline |
| `Ctrl+S` | Sessions |
| `Ctrl+E` / `Ctrl+M` | Endpoint / model picker |
| `Ctrl+R` | Reasoning effort |
| `Ctrl+C` | Exit |
