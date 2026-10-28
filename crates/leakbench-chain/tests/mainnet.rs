//! Tests against real mainnet transactions.
//!
//! Each fixture is a raw transaction captured from the chain (`mainnet/*.hex`)
//! plus the fact bundle derived from it (`mainnet/*.json`). The tests assert
//! three things:
//!
//! 1. the reference parser turns the bytes into exactly the committed facts;
//! 2. the parsed numbers satisfy the consensus value equation, which is an
//!    independent check that the parser and our fact model agree with the chain;
//! 3. the analyzer produces the expected findings on that real data.

use std::fs;
use std::path::PathBuf;

use leakbench_chain::tx_from_hex;
use leakbench_core::{analyze, Subject};

/// (fixture name, expected finding ids, in report order)
const CASES: &[(&str, &[&str])] = &[
    ("crossing-986cfd40", &["LB-001", "LB-002", "LB-010"]),
    ("unshield-ddd1dbe5", &["LB-003", "LB-005", "LB-010"]),
    (
        "round-unshield-776edbb3",
        &["LB-003", "LB-005", "LB-009", "LB-010"],
    ),
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mainnet")
}

fn load(name: &str) -> (String, Subject) {
    let hex = fs::read_to_string(dir().join(format!("{name}.hex"))).expect("hex fixture");
    let json = fs::read_to_string(dir().join(format!("{name}.json"))).expect("fact fixture");
    let subject: Subject = serde_json::from_str(&json).expect("subject parses");
    (hex, subject)
}

#[test]
fn parser_matches_the_committed_facts() {
    for (name, _) in CASES {
        let (hex, subject) = load(name);
        let parsed = tx_from_hex(&hex).expect("parses");
        let expected = &subject.txs[0];

        assert_eq!(parsed.txid, expected.txid, "{name}: txid");
        assert_eq!(parsed.version, expected.version, "{name}: version");
        assert_eq!(parsed.fee_zat, expected.fee_zat, "{name}: fee");

        for pool in ["sapling", "orchard", "ironwood"] {
            let a = match pool {
                "sapling" => &parsed.sapling,
                "orchard" => &parsed.orchard,
                _ => &parsed.ironwood,
            };
            let b = &expected.pool(match pool {
                "sapling" => leakbench_core::Pool::Sapling,
                "orchard" => leakbench_core::Pool::Orchard,
                _ => leakbench_core::Pool::Ironwood,
            });
            assert_eq!(a.spends, b.spends, "{name}: {pool} spends");
            assert_eq!(a.outputs, b.outputs, "{name}: {pool} outputs");
            assert_eq!(
                a.value_balance_zat, b.value_balance_zat,
                "{name}: {pool} value balance"
            );
        }
    }
}

#[test]
fn parsed_facts_satisfy_the_value_equation() {
    // No transparent inputs in these fixtures, so:
    //   sum(shielded value balances) - transparent outputs == fee
    for (name, _) in CASES {
        let (hex, _) = load(name);
        let tx = tx_from_hex(&hex).expect("parses");
        let shielded: i128 = [tx.sapling, tx.orchard, tx.ironwood]
            .iter()
            .filter_map(|pool| pool.value_balance_zat)
            .sum();
        let transparent_out = tx.transparent.value_out_zat.unwrap_or(0);
        let fee = i128::from(tx.fee_zat.expect("shielded tx knows its fee"));
        assert_eq!(
            shielded - transparent_out,
            fee,
            "{name}: value equation does not close"
        );
    }
}

#[test]
fn real_transactions_produce_the_expected_findings() {
    for (name, expected) in CASES {
        let (_, subject) = load(name);
        let report = analyze(&subject);
        let ids: Vec<&str> = report.findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(&ids, expected, "{name}: findings");
        assert_eq!(
            report.verdict,
            leakbench_core::Verdict::Disqualified,
            "{name}"
        );
        assert_eq!(report.fatal_count(), 1, "{name}");
        assert!(
            report
                .engine
                .fact_sources
                .iter()
                .any(|s| s == "raw" || s == "manual" || s == "fixture"),
            "{name}"
        );
    }
}

#[test]
fn the_crossing_exposes_the_exact_amount() {
    let (_, subject) = load("crossing-986cfd40");
    let tx = &subject.txs[0];
    let (from, to, amount) = tx.cross_pool_move().expect("is a crossing");
    assert_eq!(from, leakbench_core::Pool::Orchard);
    assert_eq!(to, leakbench_core::Pool::Ironwood);
    // +63,400 leaving Orchard, 43,400 arriving, 20,000 fee.
    assert_eq!(amount, 43_400);
    assert_eq!(tx.orchard.value_balance_zat, Some(63_400));
    assert_eq!(tx.ironwood.value_balance_zat, Some(-43_400));
    assert_eq!(tx.fee_zat, Some(20_000));
}
