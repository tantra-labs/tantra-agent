use std::collections::HashMap;

use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use tantra_agent_core::library::*;
use tantra_agent_core::{CatalogError, IndicatorCatalog};

struct Fake {
    down: bool,
}

fn entry(id: &str, kind: &str, params: Value) -> Value {
    json!({"id": id, "kind": kind, "title": id, "aliases": [], "computable": true, "params": params})
}

fn all_entries() -> Vec<Value> {
    vec![
        entry(
            "ind.predictive-ranges",
            "indicator",
            json!([
                {"name":"length","type":"int","default":200,"minimum":"2","maximum":null},
                {"name":"factor","type":"decimal","default":"6","minimum":"0","maximum":null},
                {"name":"mode","type":"enum","default":"a","choices":["a"]}
            ]),
        ),
        entry("ind.vidya", "indicator", json!([])),
        entry("det.reversal-bar", "detector", json!([])),
        // Malformed (JSON-float default): must be skipped, not abort the whole load.
        entry(
            "ind.bad-float",
            "indicator",
            json!([{"name":"x","type":"decimal","default":1.5,"minimum":null,"maximum":null}]),
        ),
        entry(
            "ind.ema",
            "indicator",
            json!([{"name":"foo","type":"int","default":1,"minimum":"1","maximum":"2"}]),
        ),
    ]
}

#[async_trait::async_trait]
impl IndicatorLibraryPort for Fake {
    async fn search(&self, r: LibrarySearchRequest) -> Result<Value, LibraryPortError> {
        if self.down {
            return Err(LibraryPortError::unavailable("down"));
        }
        let kind = r.kind.unwrap_or_default();
        let res: Vec<Value> = all_entries()
            .into_iter()
            .filter(|e| e["kind"] == kind.as_str())
            .collect();
        Ok(json!({"results": res}))
    }
    async fn describe(&self, r: LibraryDescribeRequest) -> Result<Value, LibraryPortError> {
        all_entries()
            .into_iter()
            .find(|e| e["id"] == r.id.as_str())
            .ok_or_else(|| LibraryPortError::not_found(r.id))
    }
    async fn compute(&self, _: IndicatorComputeRequest) -> Result<Value, LibraryPortError> {
        Err(LibraryPortError::unavailable("n/a"))
    }
    async fn signal_study(&self, _: SignalStudyRequest) -> Result<Value, LibraryPortError> {
        Err(LibraryPortError::unavailable("n/a"))
    }
}

#[tokio::test]
async fn library_entries_approved() {
    let c = IndicatorCatalog::with_library(&Fake { down: false }, 50).await;
    for n in [
        "predictive-ranges",
        "IND.PREDICTIVE-RANGES",
        "vidya",
        "reversal-bar",
    ] {
        assert!(c.is_approved(n), "{n}");
    }
    let mut p = HashMap::new();
    p.insert("length".to_string(), Decimal::ONE);
    assert!(matches!(
        c.validate_indicator("predictive-ranges", &p),
        Err(CatalogError::ParameterOutOfBounds { .. })
    ));
    p.insert("length".to_string(), Decimal::from(10));
    assert!(c.validate_indicator("predictive-ranges", &p).is_ok());
}

#[tokio::test]
async fn static_wins() {
    let c = IndicatorCatalog::with_library(&Fake { down: false }, 50).await;
    assert_eq!(c.get("EMA"), IndicatorCatalog::new().get("EMA"));
}

#[tokio::test]
async fn unavailable_falls_back() {
    let c = IndicatorCatalog::with_library(&Fake { down: true }, 50).await;
    assert_eq!(c.library_count(), 0);
    assert!(!c.is_approved("predictive-ranges"));
    assert!(c.is_approved("RSI"));
}

#[test]
fn rejects_float_default() {
    let v = entry(
        "ind.x",
        "indicator",
        json!([{"name":"a","type":"decimal","default":1.5}]),
    );
    let e = LibraryIndicatorDescriptor::from_entry_json(&v).unwrap_err();
    assert_eq!(e.kind, LibraryPortErrorKind::InvalidRequest);
}

fn compute(
    bars: Option<Vec<Value>>,
    file: Option<&str>,
    tail: Option<u32>,
) -> IndicatorComputeRequest {
    IndicatorComputeRequest {
        id: "ind.sma".into(),
        params: Map::new(),
        instrument: "X".into(),
        interval: None,
        bars,
        bars_file: file.map(str::to_string),
        tail,
        overlay_scene: None,
    }
}

#[test]
fn dto_caps() {
    assert!(
        compute(Some(vec![json!({}); 5000]), None, Some(2000))
            .validate()
            .is_ok()
    );
    assert!(
        compute(Some(vec![json!({}); 5001]), None, None)
            .validate()
            .is_err()
    );
    assert!(compute(Some(vec![]), None, Some(0)).validate().is_err());
    assert!(compute(Some(vec![]), None, Some(2001)).validate().is_err());
    assert!(
        compute(Some(vec![]), Some("a.csv"), None)
            .validate()
            .is_err()
    );
    assert!(compute(None, None, None).validate().is_err());
    for f in ["../x", "/etc/passwd", "C:x", "a\\b", "a\0b"] {
        assert!(compute(None, Some(f), None).validate().is_err(), "{f}");
    }
    assert!(compute(None, Some("data/a.csv"), None).validate().is_ok());
}

#[test]
fn study_caps() {
    let mk = |h: Option<Vec<u32>>, d: Option<i8>| SignalStudyRequest {
        id: "det.x".into(),
        params: Map::new(),
        instrument: "X".into(),
        interval: None,
        bars: Some(vec![]),
        bars_file: None,
        event_column: "e".into(),
        horizons: h,
        direction: d,
        filter: None,
        overlay_scene: None,
    };
    assert!(mk(Some(vec![1; 10]), Some(-1)).validate().is_ok());
    assert!(mk(Some(vec![1; 11]), None).validate().is_err());
    assert!(mk(None, Some(0)).validate().is_err());
}

#[tokio::test]
async fn malformed_entry_is_skipped_and_count_is_distinct_entries() {
    let c = IndicatorCatalog::with_library(&Fake { down: false }, 50).await;
    assert!(!c.is_approved("bad-float"));
    assert!(c.is_approved("predictive-ranges"));
    // predictive-ranges, vidya, reversal-bar, ema (registered as IND.EMA only; static EMA wins)
    assert_eq!(c.library_count(), 4);
}
