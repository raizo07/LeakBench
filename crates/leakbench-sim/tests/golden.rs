//! Golden tests: the demo numbers are part of the product.
//!
//! If a rule changes, these fail on purpose so the README, the screenshots and
//! the demo script have to be updated together. That is cheaper than discovering
//! mid-demo that the naive wallet now scores 64.

use std::collections::BTreeMap;

use leakbench_core::{analyze, Report, Verdict};

fn counts(report: &Report) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for finding in &report.findings {
        *map.entry(finding.id.clone()).or_insert(0) += 1;
    }
    map
}

#[test]
fn naive_scenario_is_pinned() {
    let report = analyze(&leakbench_sim::naive().subject);

    assert_eq!(report.verdict, Verdict::Disqualified);
    assert_eq!(report.score.value, 0);
    assert_eq!(report.score.grade, "F");
    assert_eq!(report.findings.len(), 23);

    let fatal: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.fatal)
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(fatal, vec!["LB-001", "LB-003"]);

    let expected: BTreeMap<String, usize> = [
        ("LB-001", 1),
        ("LB-002", 1),
        ("LB-003", 1),
        ("LB-004", 4),
        ("LB-006", 1),
        ("LB-007", 1),
        ("LB-008", 1),
        ("LB-009", 4),
        ("LB-010", 1),
        ("LB-011", 1),
        ("LB-012", 1),
        ("LB-013", 4),
        ("LB-014", 1),
        ("LB-015", 1),
    ]
    .into_iter()
    .map(|(id, n)| (id.to_string(), n))
    .collect();
    assert_eq!(counts(&report), expected);

    // Fourteen of the fifteen rules fire. LB-005 (action-count disclosure) does
    // not: neither demo wallet ever consolidates more than two notes at once.
    assert_eq!(counts(&report).len(), 14);
    assert!(!report.findings.iter().any(|f| f.id == "LB-005"));
}

#[test]
fn hardened_scenario_is_pinned() {
    let report = analyze(&leakbench_sim::hardened().subject);

    assert_eq!(report.verdict, Verdict::Acceptable);
    assert_eq!(report.score.value, 79);
    assert_eq!(report.findings.len(), 5);
    assert!(!report.findings.iter().any(|f| f.fatal));
    assert!(report.findings.iter().all(|f| f.id == "LB-001"));
    assert!(report
        .findings
        .iter()
        .all(|f| f.severity == leakbench_core::Severity::Medium));
}

#[test]
fn the_turnstile_is_the_only_shared_failure_mode() {
    let naive = analyze(&leakbench_sim::naive().subject);
    let hardened = analyze(&leakbench_sim::hardened().subject);

    let ids = |r: &Report| -> std::collections::BTreeSet<String> {
        r.findings.iter().map(|f| f.id.clone()).collect()
    };
    let shared: Vec<String> = ids(&naive).intersection(&ids(&hardened)).cloned().collect();
    assert_eq!(shared, vec!["LB-001".to_string()]);
}

#[test]
fn reports_survive_a_json_round_trip() {
    let report = analyze(&leakbench_sim::hardened().subject);
    let json = report.to_json_pretty();
    let back: Report = serde_json::from_str(&json).expect("report parses");
    assert_eq!(back.schema, "leakbench.report.v1");
    assert_eq!(back.score.value, report.score.value);
    assert_eq!(back.findings.len(), report.findings.len());
    assert_eq!(back.timeline.len(), report.timeline.len());
}
