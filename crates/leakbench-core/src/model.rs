//! Report model: the stable JSON contract between the engine, the CLI and the
//! web UI. Anything a judge reads comes from here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::facts::{format_zec, SubjectKind, TxFacts, WalletFacts};

pub const SCHEMA: &str = "leakbench.report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    /// Risk points subtracted from a perfect score. A `fatal` finding scores
    /// differently; see `score.rs`.
    pub fn weight(self) -> i32 {
        match self {
            Severity::Info => 1,
            Severity::Low => 3,
            Severity::Medium => 8,
            Severity::High => 20,
            Severity::Critical => 34,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            Confidence::Low => 0.4,
            Confidence::Medium => 0.7,
            Confidence::High => 1.0,
        }
    }
}

/// What kind of problem a finding describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Value or linkage that becomes public. These are what the ZECATHON rules
    /// call disqualifying.
    Leak,
    /// Statistical linkage between otherwise-anonymous items.
    Linkage,
    /// Software / behaviour fingerprint that narrows the anonymity set.
    Fingerprint,
    /// Best-practice gap that is not itself a leak.
    Hygiene,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Leak => "leak",
            Category::Linkage => "linkage",
            Category::Fingerprint => "fingerprint",
            Category::Hygiene => "hygiene",
        }
    }
}

/// A human-readable piece of evidence. Ordered, not a map: evidence should read
/// like an argument, and stable ordering keeps report diffs meaningful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Evidence {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn zat(label: impl Into<String>, zat: i128) -> Self {
        Self::new(label, format_zec(zat))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable rule id, e.g. `LB-001`.
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub category: Category,
    /// `true` when this finding is a disclosure that the hackathon brief would
    /// treat as disqualifying rather than a deduction.
    pub fatal: bool,
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub remediation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// At least one fatal disclosure.
    Disqualified,
    /// No fatal disclosure, but the residual fingerprint is significant.
    AtRisk,
    Acceptable,
    Clean,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Disqualified => "disqualified",
            Verdict::AtRisk => "at-risk",
            Verdict::Acceptable => "acceptable",
            Verdict::Clean => "clean",
        }
    }

    pub fn headline(self) -> &'static str {
        match self {
            Verdict::Disqualified => "Leaks on chain",
            Verdict::AtRisk => "Material leaks",
            Verdict::Acceptable => "Survivable",
            Verdict::Clean => "No material leaks",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    /// 0..=100. Higher is better (fewer leaks).
    pub value: u32,
    pub grade: String,
    pub verdict: Verdict,
    /// Total weighted risk points removed, before clamping.
    pub risk_points: f32,
    /// Count of findings per severity, for the UI summary bars.
    pub by_severity: BTreeMap<String, usize>,
}

impl Score {
    pub fn grade_for(value: u32) -> &'static str {
        match value {
            95..=100 => "A+",
            85..=94 => "A",
            70..=84 => "B",
            55..=69 => "C",
            35..=54 => "D",
            _ => "F",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub heuristics: usize,
    /// Where the facts came from: `simulated`, `fixture`, `rpc`, `manual`.
    pub fact_sources: Vec<String>,
    pub network: String,
    pub block_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectSummary {
    pub label: String,
    pub kind: SubjectKind,
    pub network: String,
    pub tx_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub has_wallet_facts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<i64>,
    pub txid: String,
    pub label: String,
    pub shape: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactsBundle {
    pub transactions: Vec<TxFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletFacts>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub generated_at: String,
    pub engine: EngineInfo,
    pub subject: SubjectSummary,
    pub verdict: Verdict,
    pub score: Score,
    pub findings: Vec<Finding>,
    pub timeline: Vec<TimelineEntry>,
    pub facts: FactsBundle,
    pub disclaimer: String,
}

impl Report {
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn fatal_count(&self) -> usize {
        self.findings.iter().filter(|f| f.fatal).count()
    }

    /// Highest severity present, for the CLI summary line.
    pub fn max_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }
}

pub const DISCLAIMER: &str = "LeakBench reports heuristic evidence, not a proof of \
deanonymization. Every finding names the observable it is derived from, so it can be \
checked by hand. Absence of findings is not a proof of privacy: an unmeasured leak is \
still a leak.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_for_ci_gates() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Info < Severity::Low);
    }

    #[test]
    fn grades_are_monotonic() {
        assert_eq!(Score::grade_for(100), "A+");
        assert_eq!(Score::grade_for(90), "A");
        assert_eq!(Score::grade_for(70), "B");
        assert_eq!(Score::grade_for(55), "C");
        assert_eq!(Score::grade_for(40), "D");
        assert_eq!(Score::grade_for(0), "F");
    }
}
