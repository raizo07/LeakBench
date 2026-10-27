//! Terminal rendering. Reports are for machines; this is for the person
//! reading the terminal at 2am.

use std::io::IsTerminal;

use leakbench_core::{Report, Severity};

fn color(code: &str, text: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn sev_tag(s: Severity) -> String {
    match s {
        Severity::Critical => color("1;97;41", " CRIT "),
        Severity::High => color("1;31", " HIGH "),
        Severity::Medium => color("1;33", " MED  "),
        Severity::Low => color("1;36", " LOW  "),
        Severity::Info => color("1;90", " INFO "),
    }
}

fn verdict_line(report: &Report) -> String {
    let v = report.verdict;
    let label = format!("{} / {}", v.headline(), v.as_str().to_uppercase());
    match v {
        leakbench_core::Verdict::Disqualified => color("1;97;41", &format!(" {label} ")),
        leakbench_core::Verdict::AtRisk => color("1;97;43", &format!(" {label} ")),
        leakbench_core::Verdict::Acceptable => color("1;30;103", &format!(" {label} ")),
        leakbench_core::Verdict::Clean => color("1;97;42", &format!(" {label} ")),
    }
}

fn bar(fraction: f32, width: usize) -> String {
    let filled = ((fraction.clamp(0.0, 1.0)) * width as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

pub fn render(report: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n{} {}\n",
        color("1;97;44", " LEAKBENCH "),
        color(
            "1",
            &format!(
                "{} ({})",
                report.subject.label,
                report.subject.kind.as_str()
            )
        )
    ));
    if let Some(d) = &report.subject.description {
        out.push_str(&format!("  {}\n", color("90", d)));
    }
    out.push('\n');
    out.push_str(&format!("  verdict   {}\n", verdict_line(report)));
    out.push_str(&format!(
        "  score     {} {}/100  (grade {})\n",
        bar(report.score.value as f32 / 100.0, 24),
        report.score.value,
        report.score.grade
    ));
    out.push_str(&format!(
        "  risk      {:.1} points  ·  {} transaction(s)  ·  {} rules  ·  facts: {}\n",
        report.score.risk_points,
        report.subject.tx_count,
        report.engine.heuristics,
        report.engine.fact_sources.join(", ")
    ));
    let counts = &report.score.by_severity;
    out.push_str(&format!(
        "  found     {} critical · {} high · {} medium · {} low · {} info  ({} fatal)\n",
        counts.get("critical").unwrap_or(&0),
        counts.get("high").unwrap_or(&0),
        counts.get("medium").unwrap_or(&0),
        counts.get("low").unwrap_or(&0),
        counts.get("info").unwrap_or(&0),
        report.fatal_count(),
    ));

    if report.findings.is_empty() {
        out.push_str(&format!(
            "\n  {}\n\n",
            color(
                "1;32",
                "No findings. Nothing in the catalogue can see this subject."
            )
        ));
    } else {
        out.push_str(&format!("\n  {}\n\n", color("1", "FINDINGS")));
        for f in &report.findings {
            out.push_str(&format!(
                "  {} {} {}\n",
                sev_tag(f.severity),
                color("1", f.id.as_str()),
                f.title
            ));
            out.push_str(&format!("        {}", wrap(&f.summary, 92, 8)));
            out.push('\n');
            for e in f.evidence.iter().take(4) {
                out.push_str(&format!(
                    "        {} {}\n",
                    color("90", &format!("{:<24}", format!("{}:", e.label))),
                    e.value
                ));
            }
            out.push_str(&format!(
                "        {} {}\n",
                color("90", &format!("{:<24}", "fix:")),
                color("32", &first_sentence(&f.remediation))
            ));
            if f.confidence != leakbench_core::Confidence::High {
                out.push_str(&format!(
                    "        {}\n",
                    color(
                        "90",
                        &format!("confidence: {} (evidence limited)", f.confidence.as_str())
                    )
                ));
            }
            out.push('\n');
        }
    }

    out.push_str(&format!("  {}\n\n", color("90", &report.disclaimer)));
    out
}

/// Compact one-screen summary for `demo`, comparing two reports.
pub fn compare_table(reports: &[Report]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n  {:<22} {:>7} {:>8} {:>7}  {}\n",
        color("1", "subject"),
        color("1", "score"),
        color("1", "verdict"),
        color("1", "fatal"),
        color("1", "top finding")
    ));
    for r in reports {
        let top = r
            .findings
            .first()
            .map(|f| format!("{} {}", f.id, f.title))
            .unwrap_or_else(|| "—".into());
        out.push_str(&format!(
            "  {:<22} {:>7} {:>8} {:>7}  {}\n",
            r.subject.label,
            r.score.value,
            r.verdict.as_str(),
            r.fatal_count(),
            top
        ));
    }
    out
}

fn first_sentence(s: &str) -> String {
    match s.find(". ") {
        Some(i) if i < 160 => format!("{}.", &s[..i]),
        _ if s.len() > 160 => format!("{}…", &s[..157]),
        _ => s.to_string(),
    }
}

fn wrap(text: &str, width: usize, indent: usize) -> String {
    let mut out = String::new();
    let mut line = 0usize;
    for word in text.split_whitespace() {
        if line > 0 && line + word.len() + 1 > width {
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            line = 0;
        } else if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(word);
        line += word.len();
    }
    out
}
