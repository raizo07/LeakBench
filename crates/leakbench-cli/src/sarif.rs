//! SARIF 2.1.0 output.
//!
//! The point is distribution: a SARIF file drops into GitHub code scanning (or
//! any other SARIF consumer) and the findings show up where people already look,
//! with no dashboard to build and no service to run.

use leakbench_core::{catalogue, Report, Severity};
use serde_json::{json, Value};

pub fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

pub fn build(report: &Report) -> Value {
    let rules: Vec<Value> = catalogue()
        .into_iter()
        .map(|(id, title, description)| {
            json!({
                "id": id,
                "name": title,
                "shortDescription": { "text": title },
                "fullDescription": { "text": description },
            })
        })
        .collect();

    let results: Vec<Value> = report
        .findings
        .iter()
        .map(|finding| {
            let uri = match finding.txids.first() {
                Some(txid) => format!("zcash://transaction/{txid}"),
                None => format!("leakbench://subject/{}", slug(&report.subject.label)),
            };
            let fingerprint = format!(
                "{}:{}",
                finding.id,
                finding
                    .txids
                    .first()
                    .cloned()
                    .unwrap_or_else(|| report.subject.label.clone())
            );
            json!({
                "ruleId": finding.id,
                "level": level(finding.severity),
                "message": { "text": finding.summary },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                        "region": { "startLine": 1 }
                    }
                }],
                "partialFingerprints": { "leakbenchFinding": fingerprint },
                "properties": {
                    "severity": finding.severity.as_str(),
                    "confidence": finding.confidence.as_str(),
                    "category": finding.category.as_str(),
                    "fatal": finding.fatal,
                    "remediation": finding.remediation,
                }
            })
        })
        .collect();

    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "LeakBench",
                    "informationUri": "https://github.com/raizo07/LeakBench",
                    "version": report.engine.version,
                    "rules": rules,
                }
            },
            "results": results,
            "properties": {
                "schema": report.schema,
                "verdict": report.verdict.as_str(),
                "score": report.score.value,
                "grade": report.score.grade,
                "subject": report.subject.label,
                "network": report.subject.network,
            }
        }]
    })
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use leakbench_core::analyze;
    use leakbench_core::facts::{PoolFacts, Source, Subject, SubjectKind, TxFacts};

    fn report() -> Report {
        let tx = TxFacts {
            txid: "cd".repeat(32),
            version: 6,
            block_height: Some(3_500_041),
            block_time: None,
            fee_zat: Some(10_000),
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: PoolFacts {
                spends: 1,
                outputs: 0,
                value_balance_zat: Some(500_000_000),
                ..Default::default()
            },
            ironwood: PoolFacts {
                spends: 0,
                outputs: 1,
                value_balance_zat: Some(-500_000_000),
                ..Default::default()
            },
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        let mut subject = Subject::new("my wallet", SubjectKind::Wallet, "main");
        subject.txs.push(tx);
        analyze(&subject)
    }

    #[test]
    fn produces_valid_sarif_shape() {
        let sarif = build(&report());
        assert_eq!(sarif["version"], "2.1.0");
        let run = &sarif["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "LeakBench");
        assert!(run["tool"]["driver"]["rules"].as_array().unwrap().len() >= 15);
        let results = run["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r["ruleId"] == "LB-001"));
        assert!(results
            .iter()
            .any(|r| r["level"] == "error" && r["properties"]["fatal"] == true));
        assert!(
            results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .starts_with("zcash://transaction/")
        );
    }

    #[test]
    fn severity_maps_to_sarif_levels() {
        assert_eq!(level(Severity::Critical), "error");
        assert_eq!(level(Severity::High), "error");
        assert_eq!(level(Severity::Medium), "warning");
        assert_eq!(level(Severity::Low), "note");
    }
}
