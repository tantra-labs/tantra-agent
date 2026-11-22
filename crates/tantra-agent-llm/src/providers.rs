//! Gateway provider configuration (opencode-style `provider` block).
//!
//! Every LLM is reached through an OpenAI-compatible endpoint listed in a
//! `tantra.jsonc` file. Parsing and merging are pure; only [`load_config`] touches
//! the filesystem and must be called at startup (before the async runtime or from
//! startup code), never on a request path.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::catalog::{ModelInfo, StaticCatalog};
use crate::openai_compat::OpenAiCompatibleProvider;

/// Configuration errors. Messages may name an env var or file, never a secret value.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// No config file found in any searched location.
    #[error("no tantra config found (searched: {})", display_paths(.searched))]
    NotFound {
        /// Locations searched.
        searched: Vec<PathBuf>,
    },
    /// JSONC parse failure.
    #[error("config parse error{}: {message}", path_suffix(.path))]
    Parse {
        /// File being parsed, if known.
        path: Option<PathBuf>,
        /// Parser message.
        message: String,
    },
    /// `{env:VAR}` referenced an unset variable.
    #[error("environment variable {0} is not set")]
    MissingEnv(String),
    /// `{file:path}` could not be read.
    #[error("cannot read {}: {source}", .path.display())]
    File {
        /// Path that failed.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Provider lacks `options.baseURL`.
    #[error("provider {0} has no options.baseURL")]
    MissingBaseUrl(String),
    /// Configured default model is not in the catalog.
    #[error("default model {0} is not defined by any provider")]
    UnknownDefault(String),
    /// No models configured.
    #[error("config defines no models")]
    Empty,
}

fn display_paths(p: &[PathBuf]) -> String {
    p.iter()
        .map(|x| x.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn path_suffix(p: &Option<PathBuf>) -> String {
    p.as_ref()
        .map(|x| format!(" in {}", x.display()))
        .unwrap_or_default()
}

/// Top-level config file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TantraConfig {
    /// Optional schema URL (ignored).
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    /// Default model id as `provider/model`.
    pub model: Option<String>,
    /// Providers by key.
    #[serde(default)]
    pub provider: BTreeMap<String, ProviderEntry>,
    /// Runtime REST endpoint used by clients (CLI, TUI, MCP).
    #[serde(default)]
    pub runtime: Option<RuntimeBlock>,
}

/// Raw `runtime` block; `token` is redacted in `Debug`.
#[derive(Clone, Default, Deserialize)]
pub struct RuntimeBlock {
    /// Runtime base URL (e.g. `http://127.0.0.1:8000`); may be an `{env:}`/`{file:}` reference.
    pub url: Option<String>,
    /// Bearer token (usually an `{env:VAR}` / `{file:path}` reference).
    pub token: Option<String>,
}

impl fmt::Debug for RuntimeBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeBlock")
            .field("url", &self.url)
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// A resolved runtime endpoint; `token` is redacted in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeEndpoint {
    /// Runtime base URL.
    pub url: String,
    /// Bearer token, if any.
    pub token: Option<String>,
}

impl fmt::Debug for RuntimeEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeEndpoint")
            .field("url", &self.url)
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// Resolve the `runtime` block (`{env:}`/`{file:}` against `loaded.base_dir`).
/// Returns `None` when there is no block or it has no `url`.
pub fn resolve_runtime(
    loaded: &LoadedConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<RuntimeEndpoint>, ConfigError> {
    let Some(block) = loaded.config.runtime.as_ref() else {
        return Ok(None);
    };
    let Some(raw_url) = block.url.as_deref() else {
        return Ok(None);
    };
    let url = resolve_value(raw_url, &loaded.base_dir, env)?
        .trim()
        .to_string();
    if url.is_empty() {
        return Ok(None);
    }
    let token = match block.token.as_deref() {
        Some(raw) => {
            let t = resolve_value(raw, &loaded.base_dir, env)?
                .trim()
                .to_string();
            if t.is_empty() { None } else { Some(t) }
        }
        None => None,
    };
    Ok(Some(RuntimeEndpoint { url, token }))
}

/// One provider entry.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProviderEntry {
    /// Display name.
    pub name: Option<String>,
    /// Connection options.
    #[serde(default)]
    pub options: ProviderOptions,
    /// Models by key.
    #[serde(default)]
    pub models: BTreeMap<String, ModelEntry>,
}

/// Connection options; `apiKey` is redacted in `Debug`.
#[derive(Clone, Default, Deserialize)]
pub struct ProviderOptions {
    /// OpenAI-compatible endpoint root.
    #[serde(rename = "baseURL")]
    pub base_url: Option<String>,
    /// Bearer key (usually an `{env:VAR}` / `{file:path}` reference).
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    /// Request timeout in milliseconds.
    pub timeout: Option<u64>,
}

impl fmt::Debug for ProviderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderOptions")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// One model entry.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelEntry {
    /// Display name.
    pub name: Option<String>,
    /// Upstream model id if different from the key.
    pub id: Option<String>,
    /// Tool-calling support.
    pub tool_call: Option<bool>,
    /// Token limits.
    pub limit: Option<ModelLimit>,
}

/// Model token limits.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelLimit {
    /// Context window.
    pub context: Option<u32>,
    /// Max output tokens.
    pub output: Option<u32>,
}

/// Parse JSONC/JSON5 text (comments and trailing commas allowed).
pub fn parse_config(text: &str) -> Result<TantraConfig, ConfigError> {
    json5::from_str(text).map_err(|e| ConfigError::Parse {
        path: None,
        message: e.to_string(),
    })
}

/// Substitute `{env:VAR}` and `{file:path}` references in a config value.
pub fn resolve_value(
    raw: &str,
    base_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, ConfigError> {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('{') {
        let tail = &rest[start..];
        let (kind, is_env) = if tail.starts_with("{env:") {
            ("{env:", true)
        } else if tail.starts_with("{file:") {
            ("{file:", false)
        } else {
            out.push_str(&rest[..=start]);
            rest = &rest[start + 1..];
            continue;
        };
        let Some(end) = tail.find('}') else {
            break;
        };
        out.push_str(&rest[..start]);
        let arg = tail[kind.len()..end].trim();
        if is_env {
            let v = env(arg).ok_or_else(|| ConfigError::MissingEnv(arg.to_string()))?;
            out.push_str(&v);
        } else {
            let path = expand_path(arg, base_dir, env);
            let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::File {
                path: path.clone(),
                source,
            })?;
            out.push_str(text.trim());
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn expand_path(arg: &str, base_dir: &Path, env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(stripped) = arg.strip_prefix("~/") {
        if let Some(home) = env("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    let p = PathBuf::from(arg);
    if p.is_absolute() { p } else { base_dir.join(p) }
}

/// Merge user config with project config; project wins.
pub fn merge(user: TantraConfig, project: TantraConfig) -> TantraConfig {
    let mut providers = user.provider;
    for (key, p) in project.provider {
        match providers.remove(&key) {
            None => {
                providers.insert(key, p);
            }
            Some(mut u) => {
                u.name = p.name.or(u.name);
                u.options.base_url = p.options.base_url.or(u.options.base_url);
                u.options.api_key = p.options.api_key.or(u.options.api_key);
                u.options.timeout = p.options.timeout.or(u.options.timeout);
                for (mk, m) in p.models {
                    u.models.insert(mk, m);
                }
                providers.insert(key, u);
            }
        }
    }
    TantraConfig {
        schema: project.schema.or(user.schema),
        model: project.model.or(user.model),
        provider: providers,
        runtime: match (user.runtime, project.runtime) {
            (u, None) => u,
            (None, p) => p,
            (Some(u), Some(p)) => Some(RuntimeBlock {
                url: p.url.or(u.url),
                token: p.token.or(u.token),
            }),
        },
    }
}

/// Locate `(user, project)` config files. `env` is used only to find the standard
/// config directory (`XDG_CONFIG_HOME` / `HOME`).
pub fn config_paths(
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    const NAMES: [&str; 2] = ["tantra.jsonc", "tantra.json"];
    let user_dir = env("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("tantra"));
    let user = user_dir.and_then(|d| NAMES.iter().map(|n| d.join(n)).find(|p| p.is_file()));
    let project = cwd
        .ancestors()
        .find_map(|dir| NAMES.iter().map(|n| dir.join(n)).find(|p| p.is_file()));
    (user, project)
}

/// A loaded, merged configuration.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// Merged config.
    pub config: TantraConfig,
    /// Files read, lowest precedence first.
    pub sources: Vec<PathBuf>,
    /// Directory of the highest-precedence file (base for relative `{file:}`).
    pub base_dir: PathBuf,
}

fn read_one(path: &Path) -> Result<TantraConfig, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::File {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config(&text).map_err(|e| match e {
        ConfigError::Parse { message, .. } => ConfigError::Parse {
            path: Some(path.to_path_buf()),
            message,
        },
        other => other,
    })
}

fn dir_of(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Load config from disk. Startup-only synchronous IO: call before the async runtime
/// or at process startup, never from a request path.
pub fn load_config(explicit: Option<&Path>) -> Result<LoadedConfig, ConfigError> {
    if let Some(p) = explicit {
        return Ok(LoadedConfig {
            config: read_one(p)?,
            sources: vec![p.to_path_buf()],
            base_dir: dir_of(p),
        });
    }
    let env = |k: &str| std::env::var(k).ok();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (user, project) = config_paths(&cwd, &env);
    match (user, project) {
        (None, None) => Err(ConfigError::NotFound {
            searched: vec![
                env("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".config")))
                    .unwrap_or_default()
                    .join("tantra/tantra.jsonc"),
                cwd.join("tantra.jsonc"),
            ],
        }),
        (Some(u), None) => Ok(LoadedConfig {
            config: read_one(&u)?,
            base_dir: dir_of(&u),
            sources: vec![u],
        }),
        (None, Some(p)) => Ok(LoadedConfig {
            config: read_one(&p)?,
            base_dir: dir_of(&p),
            sources: vec![p],
        }),
        (Some(u), Some(p)) => Ok(LoadedConfig {
            config: merge(read_one(&u)?, read_one(&p)?),
            base_dir: dir_of(&p),
            sources: vec![u, p],
        }),
    }
}

/// Builds a [`StaticCatalog`] of OpenAI-compatible providers from a loaded config.
pub struct ConfigCatalog;

impl ConfigCatalog {
    /// Build the catalog; ids are `"{provider}/{model_key}"`.
    pub fn build(
        loaded: &LoadedConfig,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<StaticCatalog, ConfigError> {
        let mut catalog = StaticCatalog::new();
        let mut first: Option<String> = None;
        let mut ids: Vec<String> = Vec::new();
        for (pkey, p) in &loaded.config.provider {
            if p.models.is_empty() {
                continue;
            }
            let raw_url = p
                .options
                .base_url
                .as_deref()
                .ok_or_else(|| ConfigError::MissingBaseUrl(pkey.clone()))?;
            let base_url = resolve_value(raw_url, &loaded.base_dir, env)?;
            let api_key = match p.options.api_key.as_deref() {
                Some(k) => Some(resolve_value(k, &loaded.base_dir, env)?),
                None => None,
            };
            let timeout = Duration::from_millis(p.options.timeout.unwrap_or(120_000));
            for (mkey, m) in &p.models {
                let upstream = m.id.clone().unwrap_or_else(|| mkey.clone());
                let provider =
                    OpenAiCompatibleProvider::new(base_url.clone(), api_key.clone(), upstream)
                        .with_timeout(timeout);
                let id = format!("{pkey}/{mkey}");
                let info = ModelInfo {
                    id: id.clone(),
                    name: m.name.clone().unwrap_or_else(|| mkey.clone()),
                    tool_call: m.tool_call.unwrap_or(false),
                    context: m.limit.as_ref().and_then(|l| l.context),
                };
                catalog = catalog.with_model_info(info, Arc::new(provider));
                if first.is_none() {
                    first = Some(id.clone());
                }
                ids.push(id);
            }
        }
        let default = match &loaded.config.model {
            Some(d) if ids.iter().any(|i| i == d) => d.clone(),
            Some(d) => return Err(ConfigError::UnknownDefault(d.clone())),
            None => first.ok_or(ConfigError::Empty)?,
        };
        if ids.is_empty() {
            return Err(ConfigError::Empty);
        }
        Ok(catalog.with_default(default))
    }
}
