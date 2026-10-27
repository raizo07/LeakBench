//! Report assembly: run the catalogue, score it, and freeze a report.

use std::collections::BTreeMap;

use crate::facts::{Subject, TxFacts};
use crate::heuristics;
use crate::model::{
    EngineInfo, FactsBundle, Report, Severity, SubjectSummary, TimelineEntry, DISCLAIMER, SCHEMA,
};
use crate::score;
use crate::time;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn analyze(subject: &Subject) -> Report {
    let findings = heuristics::run_all(subject);
    let score = score::score_findings(&findings);

    // Which findings touch which transaction, for the timeline.
    let mut worst: BTreeMap<&str, Severity> = BTreeMap::new();
    for f in &findings {
        for txid in &f.txids {
            worst
                .entry(txid.as_str())
                .and_modify(|s| {
                    if f.severity > *s {
                        *s = f.severity;
                    }
                })
                .or_insert(f.severity);
        }
    }

    let timeline = subject
        .sorted_txs()
        .into_iter()
        .map(|tx| TimelineEntry {
            height: tx.block_height,
            time: tx.block_time,
            txid: tx.txid.clone(),
            label: tx.label.clone().unwrap_or_else(|| describe(tx)),
            shape: tx.shape(),
            severity: worst.get(tx.txid.as_str()).copied(),
        })
        .collect();

    let (first_height, last_height) = match subject.span() {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };

    let fact_sources = subject
        .all_sources()
        .into_iter()
        .map(|s| s.as_str().to_string())
        .collect();

    Report {
        schema: SCHEMA.to_string(),
        generated_at: time::to_iso8601(time::now_epoch_secs()),
        engine: EngineInfo {
            name: "leakbench".into(),
            version: VERSION.into(),
            heuristics: heuristics::RULES.len(),
            fact_sources,
            network: subject.network.clone(),
            block_interval_secs: subject.context.block_interval_secs,
        },
        subject: SubjectSummary {
            label: subject.label.clone(),
            kind: subject.kind,
            network: subject.network.clone(),
            tx_count: subject.txs.len(),
            first_height,
            last_height,
            description: subject.description.clone(),
            has_wallet_facts: subject.wallet.is_some(),
        },
        verdict: score.verdict,
        score,
        findings,
        timeline,
        facts: FactsBundle {
            transactions: subject.txs.clone(),
            wallet: subject.wallet.clone(),
        },
        disclaimer: DISCLAIMER.to_string(),
    }
}

/// A short human description of a transaction's shape, used when the facts do
/// not carry a label.
pub fn describe(tx: &TxFacts) -> String {
    if let Some((from, to, _)) = tx.cross_pool_move() {
        return format!("{} → {} crossing", from.title(), to.title());
    }
    if tx.shielded_spend_count() > 0 && tx.transparent.outputs > 0 {
        return "unshield to transparent".into();
    }
    if tx.transparent.spends > 0 && tx.shielded_output_count() > 0 {
        return "shield from transparent".into();
    }
    if let Some((pool, _)) = tx.active_shielded().next() {
        return format!("{} transfer", pool.title());
    }
    "transparent transfer".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{PoolFacts, Source, SubjectKind};

    fn subject_with_txs(txs: Vec<TxFacts>) -> Subject {
        let mut s = Subject::new("t", SubjectKind::Wallet, "main");
        s.txs = txs;
        s
    }

    #[test]
    fn report_shape_is_stable() {
        let subject = subject_with_txs(vec![]);
        let report = analyze(&subject);
        assert_eq!(report.schema, "leakbench.report.v1");
        assert_eq!(report.engine.heuristics, heuristics::RULES.len());
        assert!(report.timeline.is_empty());
        let json = report.to_json_pretty();
        assert!(json.contains("\"schema\""));
        assert!(json.contains("\"verdict\""));
    }

    #[test]
    fn describes_shapes() {
        let mut tx = TxFacts {
            txid: "aa".into(),
            version: 6,
            block_height: Some(1),
            block_time: None,
            fee_zat: None,
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: PoolFacts {
                spends: 1,
                outputs: 0,
                value_balance_zat: Some(500),
                ..Default::default()
            },
            ironwood: PoolFacts {
                spends: 0,
                outputs: 1,
                value_balance_zat: Some(-500),
                ..Default::default()
            },
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        assert_eq!(describe(&tx), "Orchard → Ironwood crossing");
        tx.ironwood = PoolFacts::default();
        assert_eq!(describe(&tx), "Orchard transfer");
    }

    #[test]
    fn timeline_carries_worst_severity_per_tx() {
        let tx = TxFacts {
            txid: "bb".into(),
            version: 6,
            block_height: Some(100),
            block_time: None,
            fee_zat: Some(5_000),
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: PoolFacts {
                spends: 1,
                outputs: 0,
                value_balance_zat: Some(100_000_000),
                ..Default::default()
            },
            ironwood: PoolFacts {
                spends: 0,
                outputs: 1,
                value_balance_zat: Some(-100_000_000),
                ..Default::default()
            },
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        let report = analyze(&subject_with_txs(vec![tx]));
        assert_eq!(report.timeline.len(), 1);
        assert_eq!(report.timeline[0].severity, Some(Severity::Critical));
        assert_eq!(report.verdict, crate::model::Verdict::Disqualified);
    }
}
