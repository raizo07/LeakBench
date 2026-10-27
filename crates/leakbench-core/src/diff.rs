//! Report diffing: what changed between two observations of the same subject?
//!
//! A single score is a snapshot. The useful question in CI, and over the weeks
//! of a protocol migration, is whether the wallet got *worse*. Diffing is
//! deliberately structural rather than textual: findings are compared by rule id
//! and occurrence count, so a reordered report is not a change.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Confidence, Report, Severity, Verdict};

pub const DIFF_SCHEMA: &str = "leakbench.diff.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub label: String,
    pub score: u32,
    pub grade: String,
    pub verdict: Verdict,
    pub findings: usize,
    pub fatal: usize,
    /// Unclamped risk: a score of 0 can still get worse, but risk cannot hide it.
    pub risk_points: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

impl From<&Report> for ReportSummary {
    fn from(report: &Report) -> Self {
        Self {
            label: report.subject.label.clone(),
            score: report.score.value,
            grade: report.score.grade.clone(),
            verdict: report.verdict,
            findings: report.findings.len(),
            fatal: report.fatal_count(),
            risk_points: report.score.risk_points,
            generated_at: Some(report.generated_at.clone()),
        }
    }
}

/// One rule's movement between the two reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingDelta {
    pub id: String,
    pub title: String,
    pub before: usize,
    pub after: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_before: Option<Severity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_after: Option<Severity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_before: Option<Confidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_after: Option<Confidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDiff {
    pub schema: String,
    pub before: ReportSummary,
    pub after: ReportSummary,
    /// Positive means the score improved.
    pub score_delta: i32,
    /// Positive means more total risk.
    pub risk_delta: f32,
    pub verdict_changed: bool,
    /// Rules present only in `after`.
    pub added: Vec<FindingDelta>,
    /// Rules present only in `before`.
    pub resolved: Vec<FindingDelta>,
    /// Rules present in both, with a different occurrence count or severity.
    pub changed: Vec<FindingDelta>,
}

impl ReportDiff {
    /// A regression is a new fatal disclosure, a score loss, or more total risk.
    ///
    /// The risk term matters because the score is clamped at zero: a wallet that
    /// is already disqualified can pick up three more high findings and still
    /// read "0 → 0". Risk is unclamped, so it notices.
    pub fn is_regression(&self) -> bool {
        self.after.fatal > self.before.fatal || self.score_delta < 0 || self.risk_delta > 0.5
    }

    pub fn points_lost(&self) -> i32 {
        -self.score_delta
    }

    pub fn is_noop(&self) -> bool {
        self.added.is_empty() && self.resolved.is_empty() && self.changed.is_empty()
    }
}

#[derive(Clone)]
struct Agg {
    count: usize,
    severity: Severity,
    confidence: Confidence,
    title: String,
}

fn aggregate(report: &Report) -> BTreeMap<String, Agg> {
    let mut map: BTreeMap<String, Agg> = BTreeMap::new();
    for finding in &report.findings {
        map.entry(finding.id.clone())
            .and_modify(|agg| {
                agg.count += 1;
                if finding.severity > agg.severity {
                    agg.severity = finding.severity;
                }
                if finding.confidence > agg.confidence {
                    agg.confidence = finding.confidence;
                }
            })
            .or_insert(Agg {
                count: 1,
                severity: finding.severity,
                confidence: finding.confidence,
                title: finding.title.clone(),
            });
    }
    map
}

pub fn diff(before: &Report, after: &Report) -> ReportDiff {
    let a = aggregate(before);
    let b = aggregate(after);

    let mut added = Vec::new();
    let mut resolved = Vec::new();
    let mut changed = Vec::new();

    for (id, after_agg) in &b {
        match a.get(id) {
            None => added.push(FindingDelta {
                id: id.clone(),
                title: after_agg.title.clone(),
                before: 0,
                after: after_agg.count,
                severity_before: None,
                severity_after: Some(after_agg.severity),
                confidence_before: None,
                confidence_after: Some(after_agg.confidence),
            }),
            Some(before_agg) => {
                if before_agg.count != after_agg.count
                    || before_agg.severity != after_agg.severity
                    || before_agg.confidence != after_agg.confidence
                {
                    changed.push(FindingDelta {
                        id: id.clone(),
                        title: after_agg.title.clone(),
                        before: before_agg.count,
                        after: after_agg.count,
                        severity_before: Some(before_agg.severity),
                        severity_after: Some(after_agg.severity),
                        confidence_before: Some(before_agg.confidence),
                        confidence_after: Some(after_agg.confidence),
                    });
                }
            }
        }
    }

    for (id, before_agg) in &a {
        if !b.contains_key(id) {
            resolved.push(FindingDelta {
                id: id.clone(),
                title: before_agg.title.clone(),
                before: before_agg.count,
                after: 0,
                severity_before: Some(before_agg.severity),
                severity_after: None,
                confidence_before: Some(before_agg.confidence),
                confidence_after: None,
            });
        }
    }

    ReportDiff {
        schema: DIFF_SCHEMA.to_string(),
        before: before.into(),
        after: after.into(),
        score_delta: after.score.value as i32 - before.score.value as i32,
        risk_delta: ((after.score.risk_points - before.score.risk_points) * 10.0).round() / 10.0,
        verdict_changed: before.verdict != after.verdict,
        added,
        resolved,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::analyze;
    use crate::facts::{PoolFacts, Source, Subject, SubjectKind, TxFacts};

    fn crossing(peers: Vec<crate::facts::PeerSummary>) -> Subject {
        let tx = TxFacts {
            txid: "ab".repeat(32),
            version: 6,
            block_height: Some(3_500_041),
            block_time: None,
            fee_zat: Some(10_000),
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: PoolFacts {
                spends: 2,
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
        let mut subject = Subject::new("wallet", SubjectKind::Wallet, "main");
        subject.txs.push(tx);
        subject.peers = peers;
        subject
    }

    fn crowded() -> Vec<crate::facts::PeerSummary> {
        (0..6)
            .map(|i| crate::facts::PeerSummary {
                txid: format!("peer-{i}"),
                height: 3_500_040 + i,
                shape: "t0/0 s0/0 o1/0 i0/1".into(),
                amount_zat: Some(480_000_000 + i as i128 * 1_000_000),
                kind: Some("migration".into()),
            })
            .collect()
    }

    #[test]
    fn fatal_to_crowded_is_an_improvement_not_a_regression() {
        let before = analyze(&crossing(vec![]));
        let after = analyze(&crossing(crowded()));
        let d = diff(&before, &after);
        assert!(!d.is_regression());
        assert!(d.score_delta > 0);
        // LB-001 survives — the amount is still round, so it is still fatal — but
        // the peer sample turned an unmeasured claim into a measured one.
        let lb001 = d
            .changed
            .iter()
            .find(|c| c.id == "LB-001")
            .expect("LB-001 changed");
        assert_eq!(lb001.severity_after, Some(Severity::Critical));
        assert_eq!(lb001.confidence_before, Some(Confidence::Medium));
        assert_eq!(lb001.confidence_after, Some(Confidence::High));
        assert!(d.resolved.iter().any(|r| r.id == "LB-002"));
    }

    #[test]
    fn crowded_to_fatal_is_a_regression() {
        let before = analyze(&crossing(crowded()));
        let after = analyze(&crossing(vec![]));
        let d = diff(&before, &after);
        assert!(d.is_regression());
        assert!(d.points_lost() > 0);
        assert_eq!(d.after.fatal, 1);
    }

    #[test]
    fn identical_reports_have_no_deltas() {
        let report = analyze(&crossing(crowded()));
        let d = diff(&report, &report);
        assert!(d.is_noop());
        assert_eq!(d.score_delta, 0);
        assert!(!d.is_regression());
    }

    #[test]
    fn resolved_rules_are_reported_by_id() {
        let mut clean = Subject::new("wallet", SubjectKind::Wallet, "main");
        clean.txs.clear();
        let before = analyze(&crossing(vec![]));
        let after = analyze(&clean);
        let d = diff(&before, &after);
        assert!(d.resolved.iter().any(|r| r.id == "LB-001"));
        assert!(!d.is_noop());
        assert!(d.score_delta > 0);
    }
}
