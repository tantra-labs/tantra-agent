use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tantra_agent_llm::providers::ProviderOptions;
use tantra_agent_llm::{
    ConfigCatalog, ConfigError, LoadedConfig, ModelCatalog, config_paths, merge, parse_config,
    resolve_value,
};

const GOLDEN: &str = r#"
// opencode-shaped config
{"$schema":"https://opencode.ai/config.json", "model":"local/qwen3-30b-a3b",
 "provider":{
   "local":{"name":"llama.cpp","options":{"baseURL":"http://127.0.0.1:8080/v1"},
            "models":{"qwen3-30b-a3b":{"name":"Qwen3 30B A3B","tool_call":true,"limit":{"context":16384,"output":4096}}}},
   "gw":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"{env:GW_URL}","apiKey":"{env:GW_KEY}","timeout":5000},
         "models":{"ds":{"id":"deepseek-v4-flash"}}},
 }}
"#;

fn env_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn loaded(text: &str) -> LoadedConfig {
    LoadedConfig {
        config: parse_config(text).expect("parse"),
        sources: vec![],
        base_dir: PathBuf::from("."),
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tantra-providers-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

#[test]
fn golden_builds_catalog() {
    let m = env_map(&[("GW_URL", "http://gw/v1"), ("GW_KEY", "sk-secret")]);
    let cat = ConfigCatalog::build(&loaded(GOLDEN), &|k| m.get(k).cloned()).expect("build");
    let models = cat.models();
    let ids: Vec<_> = models.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["gw/ds", "local/qwen3-30b-a3b"]);
    assert_eq!(cat.default_model(), "local/qwen3-30b-a3b");
    assert_eq!(
        cat.provider("gw/ds").expect("p").model_name(),
        "deepseek-v4-flash"
    );
    let local = &models[1];
    assert!(local.tool_call);
    assert_eq!(local.context, Some(16384));
    assert!(cat.provider("nope").is_err());
}

#[test]
fn missing_env_is_typed() {
    let m = env_map(&[("GW_URL", "http://gw/v1")]);
    let err = ConfigCatalog::build(&loaded(GOLDEN), &|k| m.get(k).cloned())
        .map(|_| ())
        .expect_err("err");
    assert!(matches!(err, ConfigError::MissingEnv(ref v) if v == "GW_KEY"));
}

#[test]
fn file_reference_resolves_against_base_dir() {
    let d = tmp("file");
    std::fs::write(d.join("key.txt"), "  abc123\n").expect("write");
    let out = resolve_value("Bearer {file:key.txt}", &d, &|_| None).expect("resolve");
    assert_eq!(out, "Bearer abc123");
    let err = resolve_value("{file:missing.txt}", &d, &|_| None)
        .map(|_| ())
        .expect_err("err");
    assert!(matches!(err, ConfigError::File { .. }));
}

#[test]
fn merge_project_overrides() {
    let user = parse_config(
        r#"{"model":"a/m1","provider":{"a":{"options":{"baseURL":"http://u","apiKey":"k"},
        "models":{"m1":{"name":"user1"},"m2":{"name":"user2"}}}}}"#,
    )
    .expect("user");
    let project = parse_config(
        r#"{"model":"b/x","provider":{
        "a":{"options":{"baseURL":"http://p"},"models":{"m1":{"name":"proj1"}}},
        "b":{"options":{"baseURL":"http://b"},"models":{"x":{}}}}}"#,
    )
    .expect("project");
    let merged = merge(user, project);
    assert_eq!(merged.model.as_deref(), Some("b/x"));
    let a = &merged.provider["a"];
    assert_eq!(a.options.base_url.as_deref(), Some("http://p"));
    assert_eq!(a.options.api_key.as_deref(), Some("k"));
    assert_eq!(a.models["m1"].name.as_deref(), Some("proj1"));
    assert_eq!(a.models["m2"].name.as_deref(), Some("user2"));
    assert!(merged.provider.contains_key("b"));
}

#[test]
fn config_paths_walk_up_and_xdg() {
    let root = tmp("paths");
    let nested = root.join("a/b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    std::fs::write(root.join("tantra.jsonc"), "{}").expect("write");
    let xdg = root.join("xdg");
    std::fs::create_dir_all(xdg.join("tantra")).expect("mkdir");
    std::fs::write(xdg.join("tantra/tantra.json"), "{}").expect("write");
    let xdg_s = xdg.to_string_lossy().to_string();
    let env = move |k: &str| (k == "XDG_CONFIG_HOME").then(|| xdg_s.clone());
    let (user, project) = config_paths(&nested, &env);
    assert_eq!(user, Some(xdg.join("tantra/tantra.json")));
    assert_eq!(project, Some(root.join("tantra.jsonc")));
    let none = |_: &str| -> Option<String> { None };
    let (u, _) = config_paths(Path::new("/nonexistent-dir-x"), &none);
    assert_eq!(u, None);
}

#[test]
fn error_cases() {
    let none = |_: &str| -> Option<String> { None };
    let e = ConfigCatalog::build(
        &loaded(r#"{"model":"zz/q","provider":{"a":{"options":{"baseURL":"http://x"},"models":{"m":{}}}}}"#),
        &none,
    ).map(|_| ()).expect_err("err");
    assert!(matches!(e, ConfigError::UnknownDefault(_)));
    let e = ConfigCatalog::build(&loaded(r#"{"provider":{"a":{"models":{"m":{}}}}}"#), &none)
        .map(|_| ())
        .expect_err("err");
    assert!(matches!(e, ConfigError::MissingBaseUrl(ref p) if p == "a"));
    let e = ConfigCatalog::build(&loaded("{}"), &none)
        .map(|_| ())
        .expect_err("err");
    assert!(matches!(e, ConfigError::Empty));
    let e = parse_config("{not json").expect_err("err");
    assert!(matches!(e, ConfigError::Parse { .. }));
}

#[test]
fn debug_redacts_api_key() {
    let opts = ProviderOptions {
        base_url: Some("http://x".into()),
        api_key: Some("sk-very-secret".into()),
        timeout: None,
    };
    let dbg = format!("{opts:?}");
    assert!(!dbg.contains("sk-very-secret"));
    assert!(dbg.contains("[REDACTED]"));
}
