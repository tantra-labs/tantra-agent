# Provider config file (`tantra.jsonc`)

Tantra reaches every LLM through an OpenAI-compatible endpoint listed in one
opencode-style config file. It replaces all LLM env vars and the removed Candle GGUF provider.

## Lookup order (lowest to highest precedence)
1. User: `$XDG_CONFIG_HOME/tantra/tantra.jsonc` (else `$HOME/.config/tantra/tantra.jsonc`); `.json` also accepted.
2. Project: first `tantra.jsonc` / `tantra.json` found walking up from the current directory.
3. `--config PATH` (CLI/binaries): uses only that file.

User and project files merge: project `model` wins; providers merge by key; within a shared
provider, project `options` override field by field and models merge by key (project wins).
No file found gives `ConfigError::NotFound` listing the searched paths.
Env is read only to locate the standard config directory.

## Schema
JSONC (comments and trailing commas allowed). Unknown keys (e.g. opencode's `npm`) are ignored.

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "model": "local/qwen3-30b-a3b",          // default, "provider/model"
  "provider": {
    "local": {
      "name": "llama.cpp",
      "options": { "baseURL": "http://127.0.0.1:8080/v1" },
      "models": {
        "qwen3-30b-a3b": { "name": "Qwen3 30B A3B", "tool_call": true,
                           "limit": { "context": 16384, "output": 4096 } }
      }
    },
    "gw": {
      "options": { "baseURL": "{env:GW_URL}", "apiKey": "{file:~/.secrets/gw}", "timeout": 5000 },
      "models": { "ds": { "id": "deepseek-v4-flash" } }   // id = upstream model name
    }
  }
}
```

- `baseURL` is required per provider with models; `apiKey` optional; `timeout` in ms (default 120000).
- Catalog ids are `"{provider}/{model_key}"`; the default is `model` if set (must exist), else the first.
- `{env:VAR}`: env value (unset gives `MissingEnv`). `{file:path}`: trimmed file contents; `~/` uses `HOME`,
  relative paths resolve against the directory of the highest-precedence file. Applies to `baseURL` and `apiKey`.
- Secrets never appear in `Debug`, errors or logs; errors name the variable or file only.

## API (`tantra-agent-llm`)
`parse_config`, `resolve_value`, `merge`, `config_paths`, `load_config` (startup-only sync IO: call before the
async runtime / at startup), `ConfigCatalog::build(&LoadedConfig, env) -> StaticCatalog`.
