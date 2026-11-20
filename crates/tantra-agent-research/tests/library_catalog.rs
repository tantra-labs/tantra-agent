use tantra_agent_core::IndicatorCatalog;
use tantra_agent_core::library::LibraryIndicatorDescriptor;
use tantra_agent_research::hypothesis::HypothesisGenerator;

#[test]
fn hypothesis_generator_exposes_library_entry() {
    let d = LibraryIndicatorDescriptor::from_entry_json(&serde_json::json!({
        "id": "ind.vidya", "kind": "indicator", "title": "VIDYA", "aliases": [], "params": []
    }))
    .unwrap();
    let c = IndicatorCatalog::new().with_library_descriptors([d]);
    let g = HypothesisGenerator::with_catalog(c);
    assert!(g.catalog().is_approved("vidya"));
}
