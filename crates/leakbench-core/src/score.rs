//! Risk scoring.
//!
//! The model is deliberately simple and legible, because a score nobody can
//! audit is worse than no score:
//!
//! * a **fatal** finding (a disclosure the brief would treat as disqualifying)
//!   removes 60 points;
//! * otherwise severity sets the base penalty (critical 34, high 20, medium 8,
//!   low 3, info 1), scaled by confidence (high 1.0, medium 0.7, low 0.4);
//! * repeated findings from the same rule are damped, so a wallet with forty
//!   round-number payments is worse than one with two but not twenty times
//!   worse — a saturated score hides the shape of the problem.

use std::collections::BTreeMap;

use crate::model::{Finding, Score, Severity, Verdict};

pub const FATAL_PENALTY: f32 = 60.0;

pub fn score_findings(findings: &[Finding]) -> Score {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut risk = 0.0f32;
    let mut by_severity: BTreeMap<String, usize> = BTreeMap::new();

    for f in findings {
        *by_severity
            .entry(f.severity.as_str().to_string())
            .or_default() += 1;
        let occurrence = seen.entry(f.id.as_str()).or_insert(0);
        *occurrence += 1;
        let damping = 1.0 / (1.0 + 0.7 * (*occurrence as f32 - 1.0));
        // A fatal disclosure costs the same whether we are certain or not: the
        // policy is "leaks are disqualifying", not "leaks are disqualifying with
        // probability p". Scaling the fatal penalty by confidence had a perverse
        // effect — gathering better evidence *lowered* the score.
        risk += if f.fatal {
            FATAL_PENALTY * damping
        } else {
            f.severity.weight() as f32 * f.confidence.factor() * damping
        };
    }

    let value = (100.0 - risk).clamp(0.0, 100.0).round() as u32;
    let any_fatal = findings.iter().any(|f| f.fatal);
    let verdict = if any_fatal {
        Verdict::Disqualified
    } else if value < 60 {
        Verdict::AtRisk
    } else if value < 85 {
        Verdict::Acceptable
    } else {
        Verdict::Clean
    };

    for sev in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        by_severity.entry(sev.as_str().to_string()).or_default();
    }

    Score {
        value,
        grade: Score::grade_for(value).to_string(),
        verdict,
        risk_points: (risk * 10.0).round() / 10.0,
        by_severity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Confidence};

    fn finding(id: &str, sev: Severity, fatal: bool) -> Finding {
        Finding {
            id: id.into(),
            title: id.into(),
            severity: sev,
            confidence: Confidence::High,
            category: Category::Leak,
            fatal,
            summary: String::new(),
            evidence: vec![],
            remediation: String::new(),
            txids: vec![],
            references: vec![],
        }
    }

    #[test]
    fn clean_subject_scores_perfect() {
        let s = score_findings(&[]);
        assert_eq!(s.value, 100);
        assert_eq!(s.verdict, Verdict::Clean);
    }

    #[test]
    fn a_fatal_finding_disqualifies() {
        let s = score_findings(&[finding("LB-001", Severity::Critical, true)]);
        assert_eq!(s.value, 40);
        assert_eq!(s.verdict, Verdict::Disqualified);
    }

    #[test]
    fn repeats_are_damped() {
        let one = score_findings(&[finding("LB-009", Severity::Medium, false)]);
        let four: Vec<Finding> = (0..4)
            .map(|_| finding("LB-009", Severity::Medium, false))
            .collect();
        let many = score_findings(&four);
        assert!(one.value > many.value);
        // four repeats must not cost four times as much
        assert!(many.value > one.value - 4 * 8);
    }

    #[test]
    fn better_evidence_never_costs_more_for_a_fatal_finding() {
        let mut low = finding("LB-001", Severity::Critical, true);
        let high = finding("LB-001", Severity::Critical, true);
        low.confidence = Confidence::Low;
        let a = score_findings(&[low]);
        let b = score_findings(&[high]);
        assert_eq!(
            a.value, b.value,
            "fatal impact must not depend on confidence"
        );
    }

    #[test]
    fn low_confidence_costs_less() {
        let mut a = finding("LB-004", Severity::Medium, false);
        let b = finding("LB-004", Severity::Medium, false);
        a.confidence = Confidence::Low;
        let sa = score_findings(&[a]);
        let sb = score_findings(&[b]);
        assert!(sa.value > sb.value);
    }

    #[test]
    fn severity_histogram_is_complete() {
        let s = score_findings(&[finding("LB-001", Severity::Critical, true)]);
        assert_eq!(s.by_severity.get("critical"), Some(&1));
        assert_eq!(s.by_severity.get("info"), Some(&0));
    }
}
