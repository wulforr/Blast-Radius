//! Toolchain smoke test.
//!
//! This exists to prove the workspace, the test harness, and the serde
//! boundary all work before any real analysis lands. It is also the first
//! consumer of the crate's public surface, which is deliberately tiny.

use analyzer::{analyzer_info, AnalyzerInfo};

#[test]
fn reports_its_own_name_and_version() {
    let info = analyzer_info();
    assert_eq!(info.name, "blast-radius-analyzer");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn version_is_a_three_part_semver() {
    let info = analyzer_info();
    let parts: Vec<&str> = info.version.split('.').collect();
    assert_eq!(parts.len(), 3, "version was {:?}", info.version);
    for part in parts {
        assert!(
            part.chars().all(|c| c.is_ascii_digit()),
            "non-numeric version component {part:?}"
        );
    }
}

#[test]
fn serialises_to_the_json_the_action_will_read() {
    let json = serde_json::to_string(&analyzer_info()).expect("info serialises");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("info round-trips");
    assert_eq!(parsed["name"], "blast-radius-analyzer");
    assert!(parsed["version"].is_string());
}

#[test]
fn info_round_trips_through_serde() {
    let original = analyzer_info();
    let json = serde_json::to_string(&original).expect("info serialises");
    let restored: AnalyzerInfo = serde_json::from_str(&json).expect("info deserialises");
    assert_eq!(restored, original);
}
