//! Helpers for the `_meta.tantra` ACP extension namespace.

use agent_client_protocol::v1::schema::Meta;
use serde_json::Value;

pub const NAMESPACE: &str = "tantra";

pub fn tantra_meta(value: Value) -> Meta {
    let mut m = Meta::new();
    m.insert(NAMESPACE.to_string(), value);
    m
}

pub fn read_tantra(meta: &Option<Meta>) -> Option<&Value> {
    meta.as_ref().and_then(|m| m.get(NAMESPACE))
}
