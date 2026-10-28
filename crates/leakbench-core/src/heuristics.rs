//! The heuristic catalogue.
//!
//! Each rule answers one question: *what does this transaction, or this wallet,
//! tell an observer that it should not?* Rules are pure functions of
//! [`Subject`], so they can be tested against crafted facts without a node.
//!
//! Rules never claim more than their evidence supports. Where a rule needs an
//! anonymity set (peer observations) and none was supplied, it says so by
//! lowering confidence instead of guessing — that distinction is the difference
//! between a privacy tool and a novelty.

use std::collections::BTreeMap;

use crate::facts::{format_height, format_zec, PeerSummary, Subject, TxFacts};
use crate::model::{Category, Confidence, Evidence, Finding, Severity};

pub struct Rule {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub run: fn(&Subject) -> Vec<Finding>,
}

pub const RULES: &[Rule] = &[
    Rule {
        id: "LB-001",
        title: "Turnstile amount disclosure",
        description: "Cross-pool shielded movement reveals the exact migrated amount through public value balances.",
        run: turnstile_amount_disclosure,
    },
    Rule {
        id: "LB-002",
        title: "Turnstile timing anonymity set",
        description: "A pool crossing is only anonymous if enough other users cross in the same window.",
        run: turnstile_timing_anonymity,
    },
    Rule {
        id: "LB-003",
        title: "Transparent leg",
        description: "Shielded value that enters or leaves the transparent pool publishes its amount.",
        run: transparent_leg,
    },
    Rule {
        id: "LB-004",
        title: "Fee fingerprint",
        description: "A fee that deviates from the ZIP 317 conventional fee identifies the wallet software.",
        run: fee_fingerprint,
    },
    Rule {
        id: "LB-005",
        title: "Action-count disclosure",
        description: "Bundle action counts reveal how many notes were spent and how many recipients there were.",
        run: action_count_disclosure,
    },
    Rule {
        id: "LB-006",
        title: "Anchor staleness",
        description: "How far behind the tip the spend anchor is fingerprints the wallet's sync cadence.",
        run: anchor_staleness,
    },
    Rule {
        id: "LB-007",
        title: "Temporal clustering",
        description: "Several transactions inside a few blocks are trivially groupable by an observer.",
        run: temporal_clustering,
    },
    Rule {
        id: "LB-008",
        title: "Transparent address reuse",
        description: "A reused transparent address links every transaction that touched it.",
        run: address_reuse,
    },
    Rule {
        id: "LB-009",
        title: "Round-number amounts",
        description: "Publicly revealed whole-ZEC amounts are rare and therefore identifying.",
        run: round_amounts,
    },
    Rule {
        id: "LB-010",
        title: "Expiry-height fingerprint",
        description: "Setting nExpiryHeight where other wallets do not narrows the anonymity set.",
        run: expiry_fingerprint,
    },
    Rule {
        id: "LB-011",
        title: "Legacy transaction version",
        description: "Version 4 transactions become invalid at NU7 and are a historical fingerprint.",
        run: legacy_version,
    },
    Rule {
        id: "LB-012",
        title: "Memo-length fingerprint",
        description: "Varying memo lengths across a wallet's notes reveal message classes and software.",
        run: memo_fingerprint,
    },
    Rule {
        id: "LB-013",
        title: "Unique note value",
        description: "A note whose value nobody else holds is identifiable by amount alone.",
        run: unique_note_value,
    },
    Rule {
        id: "LB-014",
        title: "Pool fragmentation",
        description: "Holding notes across three or more pools maps the wallet's migration history.",
        run: pool_fragmentation,
    },
    Rule {
        id: "LB-015",
        title: "Full-balance migration",
        description: "Migrating an entire balance in one crossing ties the new account to the whole old one.",
        run: full_balance_migration,
    },
];

pub fn run_all(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for rule in RULES {
        out.extend((rule.run)(subject));
    }
    out.sort_by(|a, b| {
        b.fatal
            .cmp(&a.fatal)
            .then(b.severity.cmp(&a.severity))
            .then(a.id.cmp(&b.id))
            .then(a.txids.cmp(&b.txids))
    });
    out
}

// ---------------------------------------------------------------------------
// rules
// ---------------------------------------------------------------------------

fn turnstile_amount_disclosure(subject: &Subject) -> Vec<Finding> {
    // A crossing always publishes *an* amount. The question that matters is
    // whether that amount identifies anyone. Three things make it identifying:
    // it is round, it is the wallet's whole balance, or too few other crossings
    // sit in the same size band. Everything else is a public number lost in a
    // crowd, which is a much smaller problem and is rated accordingly.
    const BAND_PCT: i128 = 25;
    let has_peers = !subject.peers.is_empty();
    let mut out = Vec::new();

    for tx in &subject.txs {
        let Some((from, to, amount)) = tx.cross_pool_move() else {
            continue;
        };
        let amount_i = amount as i128;
        let round = roundness(amount_i);
        let wallet_total = subject
            .wallet
            .as_ref()
            .map(|w| w.total_notes_zat(from))
            .filter(|t| *t > 0);
        let full_balance = wallet_total.map(|t| t == amount_i).unwrap_or(false);
        let band = band_count(subject, amount_i, BAND_PCT);
        let identifying = round.is_some() || full_balance || !has_peers || band < 3;
        let confidence = if has_peers {
            Confidence::High
        } else {
            Confidence::Medium
        };

        let crossing = format!("{} -> {}", from.as_str(), to.as_str());
        let mut evidence = vec![
            Evidence::new("crossing", crossing.clone()),
            Evidence::zat("amount revealed", amount_i),
            Evidence::new(
                format!("{} value balance", from.as_str()),
                format_zec(tx.pool(from).value_balance_zat.unwrap_or(0)),
            )
            .with_detail("positive = value leaving the pool; public consensus field"),
            Evidence::new(
                format!("{} value balance", to.as_str()),
                format_zec(tx.pool(to).value_balance_zat.unwrap_or(0)),
            )
            .with_detail("negative = value entering the pool; public consensus field"),
            Evidence::new(
                "same-size-band crossings",
                if has_peers {
                    format!("{band} other(s) within ±{BAND_PCT}%")
                } else {
                    "unmeasured (no peer sample)".to_string()
                },
            ),
        ];
        if let Some(total) = wallet_total {
            evidence.push(Evidence::new(
                "wallet total in source pool",
                format_zec(total),
            ));
        }
        evidence.push(Evidence::new("height", height_or_unknown(tx)));
        evidence.push(Evidence::new("txid", tx.txid.clone()));

        let (severity, fatal, summary, remediation) = if identifying {
            let why = if let Some(kind) = round {
                format!("The amount is a {kind} amount, which is rare in real migration traffic.")
            } else if full_balance {
                "The crossing is exactly this wallet's entire balance in the source pool."
                    .to_string()
            } else if !has_peers {
                "No peer sample was supplied, so there is no evidence this amount is hidden."
                    .to_string()
            } else {
                format!(
                    "Only {band} other crossing(s) in the sample fall within ±{BAND_PCT}% of \
                     this amount."
                )
            };
            (
                Severity::Critical,
                true,
                format!(
                    "This transaction moved {} from the {} pool to the {} pool. Each pool's \
                     value balance is a consensus-encoded public field, so the amount is \
                     visible to anyone with a block explorer. {why} That makes the crossing \
                     identifiable by amount alone, without breaking any cryptography.",
                    format_zec(amount_i),
                    from.title(),
                    to.title()
                ),
                "Do not cross pools with an amount you would not publish. Split the migration \
                 into several crossings of non-round amounts, separated in time, and draw each \
                 chunk from a distribution that overlaps other users' behaviour. A crossing \
                 that is exactly your balance is the worst case: it publishes your total \
                 holdings."
                    .to_string(),
            )
        } else {
            (
                Severity::Medium,
                false,
                format!(
                    "This crossing moved {} from {} to {}, and that amount is public. But {band} \
                     other crossings in the sample sit within ±{BAND_PCT}% of it, so the \
                     number does not identify you on its own: it is a disclosure, not a \
                     signature. This is the residual cost of using the turnstile at all.",
                    format_zec(amount_i),
                    from.title(),
                    to.title()
                ),
                "Keep every chunk inside the common size band and avoid unique or round \
                 amounts. The disclosure cannot be removed without a protocol change; it can \
                 only be made uninteresting."
                    .to_string(),
            )
        };

        out.push(Finding {
            id: "LB-001".into(),
            title: "Turnstile amount disclosure".into(),
            severity,
            confidence,
            category: Category::Leak,
            fatal,
            summary,
            evidence,
            remediation,
            txids: vec![tx.txid.clone()],
            references: vec![
                "https://zips.z.cash/zip-0258".into(),
                "https://zips.z.cash/protocol/protocol.pdf".into(),
            ],
        });
    }
    out
}

/// How many peer observations fall within `pct` percent of `amount`.
fn band_count(subject: &Subject, amount: i128, pct: i128) -> usize {
    let lo = amount - amount * pct / 100;
    let hi = amount + amount * pct / 100;
    subject
        .peers
        .iter()
        .filter(|p| p.amount_zat.map(|a| a >= lo && a <= hi).unwrap_or(false))
        .count()
}

fn turnstile_timing_anonymity(subject: &Subject) -> Vec<Finding> {
    const WINDOW_BLOCKS: u64 = 20;
    let mut out = Vec::new();
    for tx in &subject.txs {
        let Some((from, to, amount)) = tx.cross_pool_move() else {
            continue;
        };
        let Some(height) = tx.block_height else {
            continue;
        };
        let peers: Vec<&PeerSummary> = subject
            .peers
            .iter()
            .filter(|p| p.height.abs_diff(height) <= WINDOW_BLOCKS)
            .filter(|p| {
                p.kind.as_deref() == Some("migration")
                    || p.shape == tx.shape()
                    || p.amount_zat == Some(amount as i128)
            })
            .collect();

        let (confidence, severity, k_note) = if subject.peers.is_empty() {
            (
                Confidence::Low,
                Severity::Medium,
                "no peer sample was supplied, so the anonymity set is unmeasured".to_string(),
            )
        } else {
            let k = peers.len();
            let severity = match k {
                0 | 1 => Severity::High,
                2..=4 => Severity::Medium,
                _ => continue,
            };
            (
                Confidence::High,
                severity,
                format!("{k} other crossings in the window"),
            )
        };

        let window_secs = subject.context.block_interval_secs * WINDOW_BLOCKS;
        out.push(Finding {
            id: "LB-002".into(),
            title: "Turnstile timing anonymity set".into(),
            severity,
            confidence,
            category: Category::Linkage,
            fatal: false,
            summary: format!(
                "A pool crossing is anonymous only if it hides among other crossings. In the \
                 {} blocks (+/- {} minutes) around this one, {}.",
                WINDOW_BLOCKS,
                window_secs / 60,
                k_note
            ),
            evidence: vec![
                Evidence::new("window", format!("{WINDOW_BLOCKS} blocks / {window_secs}s")),
                Evidence::new("anonymity set", k_note),
                Evidence::new("crossing", format!("{} -> {}", from.as_str(), to.as_str())),
                Evidence::new("height", format_height(height)),
                Evidence::new("txid", tx.txid.clone()),
            ],
            remediation: "Schedule crossings with randomized delay measured in hours, not \
                          blocks, and avoid known migration waves (network-upgrade days, \
                          exchange announcements) where your crossing is one of very few."
                .into(),
            txids: vec![tx.txid.clone()],
            references: vec!["https://zips.z.cash/zip-0258".into()],
        });
    }
    out
}

fn transparent_leg(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for tx in &subject.txs {
        let shielded_in = tx.shielded_spend_count() > 0;
        let shielded_out = tx.shielded_output_count() > 0;
        let t_in = tx.transparent.value_in_zat.unwrap_or(0);
        let t_out = tx.transparent.value_out_zat.unwrap_or(0);
        let t_outs = tx.transparent.outputs;

        if shielded_in && t_outs > 0 {
            let revealed = t_out - t_in;
            out.push(Finding {
                id: "LB-003".into(),
                title: "Transparent leg (unshielding)".into(),
                severity: Severity::Critical,
                confidence: Confidence::High,
                category: Category::Leak,
                fatal: true,
                summary: format!(
                    "Shielded funds were paid out to {t_outs} transparent output(s). The \
                     transparent pool is public: the amount, the addresses and the timing are \
                     all visible, which breaks the shielded set for this payment."
                ),
                evidence: vec![
                    Evidence::zat("amount exposed", revealed),
                    Evidence::new("transparent outputs", t_outs.to_string()),
                    Evidence::new("height", height_or_unknown(tx)),
                    Evidence::new("txid", tx.txid.clone()),
                ],
                remediation: "Pay shielded recipients shielded. If an exchange forces a \
                              transparent deposit address, treat that amount as public and \
                              keep it separate from the rest of your balance."
                    .into(),
                txids: vec![tx.txid.clone()],
                references: vec!["https://zips.z.cash/zip-0320".into()],
            });
        } else if shielded_out && t_out > 0 && t_in > 0 {
            let revealed = t_in - t_out;
            out.push(Finding {
                id: "LB-003".into(),
                title: "Transparent leg (shielding)".into(),
                severity: Severity::High,
                confidence: Confidence::High,
                category: Category::Leak,
                fatal: false,
                summary: "Transparent funds were shielded in this transaction. The amount \
                          entering the shielded pool is public, so the resulting note's value is \
                          known to everyone even though its owner is not."
                    .to_string(),
                evidence: vec![
                    Evidence::zat("amount exposed", revealed.max(0)),
                    Evidence::new("transparent inputs", tx.transparent.spends.to_string()),
                    Evidence::new("height", height_or_unknown(tx)),
                    Evidence::new("txid", tx.txid.clone()),
                ],
                remediation: "Where possible, receive funds shielded from the start. If \
                              shielding from transparent funds is unavoidable, avoid doing it \
                              in round amounts and avoid shielding directly before an \
                              identifying payment."
                    .into(),
                txids: vec![tx.txid.clone()],
                references: vec!["https://zips.z.cash/zip-0320".into()],
            });
        }
    }
    out
}

fn fee_fingerprint(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for tx in &subject.txs {
        let Some(fee) = tx.fee_zat else { continue };
        let floor = tx.zip317_floor_zat();
        if fee == floor {
            continue;
        }
        let delta = i128::from(fee) - i128::from(floor);
        let round = fee % 10_000 == 0;
        out.push(Finding {
            id: "LB-004".into(),
            title: "Fee fingerprint".into(),
            severity: if round {
                Severity::Medium
            } else {
                Severity::Low
            },
            confidence: if round {
                Confidence::High
            } else {
                Confidence::Medium
            },
            category: Category::Fingerprint,
            fatal: false,
            summary: format!(
                "The fee is {} but the ZIP 317 conventional fee for {} logical action(s) is \
                 {}. {} A non-conventional fee is a software fingerprint: transactions sharing \
                 it are likely to come from the same wallet build.",
                format_zec(i128::from(fee)),
                tx.logical_actions(),
                format_zec(i128::from(floor)),
                if round {
                    "It is also a round number, which is itself uncommon and memorable."
                } else {
                    ""
                }
            ),
            evidence: vec![
                Evidence::new("fee", format_zec(i128::from(fee))),
                Evidence::new("zip-317 floor", format_zec(i128::from(floor))),
                Evidence::new("difference", format!("{} zats", delta)),
                Evidence::new("logical actions", tx.logical_actions().to_string()),
                Evidence::new("txid", tx.txid.clone()),
            ],
            remediation: "Pay exactly the ZIP 317 conventional fee. Do not randomise fees \
                          upward to look unusual: a unique fee is a better fingerprint than a \
                          common one, because it isolates you."
                .into(),
            txids: vec![tx.txid.clone()],
            references: vec!["https://zips.z.cash/zip-0317".into()],
        });
    }
    out
}

fn action_count_disclosure(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for tx in &subject.txs {
        // A pool crossing is covered by LB-001, and its minimal shape is the
        // normal crossing shape rather than a distinguishing one.
        if tx.cross_pool_move().is_some() {
            continue;
        }
        // In the Orchard protocol every action spends one note and creates one
        // output, so `spends == outputs` by construction. The meaningful public
        // number is the action count: how many notes were consumed together.
        let mut evidence = Vec::new();
        let mut finding: Option<(Severity, String)> = None;
        for (pool, facts) in tx.active_shielded() {
            evidence.push(Evidence::new(
                format!("{} bundle", pool.as_str()),
                format!("{} action(s)", facts.spends),
            ));
            if facts.spends > 2 && finding.is_none() {
                finding = Some((
                    Severity::Medium,
                    format!(
                        "A {}-action {} bundle discloses that at least {} notes were held and \
                         consumed together.",
                        facts.spends,
                        pool.title(),
                        facts.spends
                    ),
                ));
            }
        }
        let Some((severity, why)) = finding else {
            continue;
        };
        evidence.push(Evidence::new(
            "logical actions",
            tx.logical_actions().to_string(),
        ));
        evidence.push(Evidence::new("height", height_or_unknown(tx)));
        evidence.push(Evidence::new("txid", tx.txid.clone()));
        out.push(Finding {
            id: "LB-005".into(),
            title: "Action-count disclosure".into(),
            severity,
            confidence: Confidence::High,
            category: Category::Fingerprint,
            fatal: false,
            summary: format!(
                "{why} Bundle sizes are public, so the counts themselves narrow the anonymity \
                 set even though the notes are hidden."
            ),
            evidence,
            remediation: "Consolidate notes in a dedicated transaction that is not adjacent to \
                          an identifying payment, and pad to the ordinary 2-action shape."
                .into(),
            txids: vec![tx.txid.clone()],
            references: vec!["https://zips.z.cash/zip-0225".into()],
        });
    }
    out
}

fn anchor_staleness(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for tx in &subject.txs {
        let Some(age) = tx.anchor_age_blocks else {
            continue;
        };
        if age <= 10 {
            continue;
        }
        let severity = if age > 100 {
            Severity::High
        } else {
            Severity::Medium
        };
        out.push(Finding {
            id: "LB-006".into(),
            title: "Anchor staleness".into(),
            severity,
            confidence: Confidence::Medium,
            category: Category::Fingerprint,
            fatal: false,
            summary: format!(
                "This spend commits to an anchor {age} blocks behind the chain tip ({}). The \
                 lag is a behavioural fingerprint: wallets with the same sync cadence produce \
                 the same lag, and an unusually stale anchor is rare.",
                crate::time::describe_span(subject.context.block_interval_secs, age)
            ),
            evidence: vec![
                Evidence::new("anchor age", format!("{age} blocks")),
                Evidence::new(
                    "wall-clock lag",
                    crate::time::describe_span(subject.context.block_interval_secs, age),
                ),
                Evidence::new("txid", tx.txid.clone()),
            ],
            remediation: "Sync to the tip immediately before spending so the anchor is fresh, \
                          and avoid long gaps between sync and spend that make your anchor \
                          older than everyone else's."
                .into(),
            txids: vec![tx.txid.clone()],
            references: vec!["https://zips.z.cash/protocol/protocol.pdf".into()],
        });
    }
    out
}

fn temporal_clustering(subject: &Subject) -> Vec<Finding> {
    const WINDOW: u64 = 8;
    const THRESHOLD: usize = 3;
    let txs = subject.sorted_txs();
    let mut out = Vec::new();
    let mut i = 0;
    while i < txs.len() {
        let Some(start) = txs[i].block_height else {
            i += 1;
            continue;
        };
        let end = start + WINDOW;
        let cluster: Vec<&TxFacts> = txs[i..]
            .iter()
            .copied()
            .take_while(|t| t.block_height.map(|h| h <= end).unwrap_or(false))
            .collect();
        if cluster.len() >= THRESHOLD {
            let txids: Vec<String> = cluster.iter().map(|t| t.txid.clone()).collect();
            out.push(Finding {
                id: "LB-007".into(),
                title: "Temporal clustering".into(),
                severity: Severity::Medium,
                confidence: Confidence::High,
                category: Category::Linkage,
                fatal: false,
                summary: format!(
                    "{} transactions fall inside {} blocks ({}). Grouping them costs an \
                     observer nothing, and the group then shares the anonymity set of its \
                     weakest member.",
                    cluster.len(),
                    WINDOW,
                    crate::time::describe_span(subject.context.block_interval_secs, WINDOW)
                ),
                evidence: vec![
                    Evidence::new("cluster size", cluster.len().to_string()),
                    Evidence::new(
                        "heights",
                        format!("{}..={}", format_height(start), format_height(end)),
                    ),
                    Evidence::new("txids", txids.join(", ")).with_detail("first 8 shown by the UI"),
                ],
                remediation: "Separate unrelated actions by hours, not blocks. Queue and \
                              release on a randomised schedule so that a burst does not form."
                    .into(),
                txids,
                references: vec![],
            });
            i += cluster.len();
        } else {
            i += 1;
        }
    }
    out
}

fn address_reuse(subject: &Subject) -> Vec<Finding> {
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tx in &subject.txs {
        for addr in &tx.transparent.addresses {
            uses.entry(addr.clone()).or_default().push(tx.txid.clone());
        }
    }
    if let Some(wallet) = &subject.wallet {
        for a in &wallet.addresses {
            if a.uses >= 2 {
                uses.entry(a.address.clone()).or_default();
            }
        }
    }

    let mut out = Vec::new();
    for (address, txids) in uses {
        let wallet_uses = subject
            .wallet
            .as_ref()
            .and_then(|w| w.addresses.iter().find(|a| a.address == address))
            .map(|a| a.uses)
            .unwrap_or(0);
        let total = (txids.len() as u32).max(wallet_uses);
        if total < 2 {
            continue;
        }
        out.push(Finding {
            id: "LB-008".into(),
            title: "Transparent address reuse".into(),
            severity: Severity::High,
            confidence: Confidence::High,
            category: Category::Linkage,
            fatal: false,
            summary: format!(
                "Transparent address {} was used {total} times. Every transaction touching it \
                 is linked, and the linkage survives any amount of shielding later.",
                short_hex(&address)
            ),
            evidence: vec![
                Evidence::new("address", address.clone()),
                Evidence::new("uses", total.to_string()),
                Evidence::new("linked transactions", txids.join(", ")),
            ],
            remediation: "Use a fresh transparent address per transaction, or stay in a \
                          shielded pool. Treat a transparent address as a permanent public \
                          identifier for its owner."
                .into(),
            txids,
            references: vec!["https://zips.z.cash/zip-0316".into()],
        });
    }
    out
}

fn round_amounts(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    for tx in &subject.txs {
        let mut candidates: Vec<(String, i128)> = Vec::new();
        if let Some((_, _, amount)) = tx.cross_pool_move() {
            candidates.push(("turnstile crossing".into(), amount as i128));
        }
        if let Some(v) = tx.transparent.value_out_zat {
            if v > 0 {
                candidates.push(("transparent output".into(), v));
            }
        }
        if let Some(v) = tx.transparent.value_in_zat {
            if v > 0 {
                candidates.push(("transparent input".into(), v));
            }
        }
        for (label, amount) in candidates {
            let Some(roundness) = roundness(amount) else {
                continue;
            };
            out.push(Finding {
                id: "LB-009".into(),
                title: "Round-number amount".into(),
                severity: Severity::Medium,
                confidence: Confidence::High,
                category: Category::Fingerprint,
                fatal: false,
                summary: format!(
                    "A publicly visible amount is exactly {}. {roundness} amounts are rare in \
                     real payments and stand out in any amount histogram.",
                    format_zec(amount)
                ),
                evidence: vec![
                    Evidence::new("amount", format_zec(amount)),
                    Evidence::new("source", label),
                    Evidence::new("txid", tx.txid.clone()),
                ],
                remediation: "Do not send or migrate whole-ZEC amounts from an address or \
                              account you care about. Amounts drawn from a realistic \
                              distribution are harder to isolate than anything round."
                    .into(),
                txids: vec![tx.txid.clone()],
                references: vec![],
            });
        }
    }
    out
}

fn expiry_fingerprint(subject: &Subject) -> Vec<Finding> {
    let with_expiry: Vec<&TxFacts> = subject
        .txs
        .iter()
        .filter(|t| t.expiry_height.unwrap_or(0) != 0)
        .collect();
    if with_expiry.is_empty() {
        return Vec::new();
    }
    let values: Vec<String> = with_expiry
        .iter()
        .filter_map(|t| t.expiry_height.map(|h| h.to_string()))
        .collect();
    vec![Finding {
        id: "LB-010".into(),
        title: "Expiry-height fingerprint".into(),
        severity: Severity::Low,
        confidence: Confidence::Medium,
        category: Category::Fingerprint,
        fatal: false,
        summary: format!(
            "{} of {} transactions set nExpiryHeight ({}). Most wallets leave it unset, so \
             setting it — or setting it to a distinctive value — separates these from the \
             crowd.",
            with_expiry.len(),
            subject.txs.len(),
            values.join(", ")
        ),
        evidence: vec![
            Evidence::new("transactions with expiry", with_expiry.len().to_string()),
            Evidence::new("expiry heights", values.join(", ")),
        ],
        remediation: "Match the ecosystem default (unset) unless you need expiry, and never \
                      use a unique expiry value as a marker: markers mark you."
            .into(),
        txids: with_expiry.iter().map(|t| t.txid.clone()).collect(),
        references: vec!["https://zips.z.cash/zip-0203".into()],
    }]
}

fn legacy_version(subject: &Subject) -> Vec<Finding> {
    let legacy: Vec<&TxFacts> = subject.txs.iter().filter(|t| t.version == 4).collect();
    if legacy.is_empty() {
        return Vec::new();
    }
    vec![Finding {
        id: "LB-011".into(),
        title: "Legacy transaction version".into(),
        severity: Severity::Medium,
        confidence: Confidence::High,
        category: Category::Hygiene,
        fatal: false,
        summary: format!(
            "{} transaction(s) use format version 4. Version 4 transactions become invalid when \
             NU7 activates (mainnet height 3,543,405, 5 November 2026), and until then they \
             are a pre-NU7 fingerprint.",
            legacy.len()
        ),
        evidence: vec![Evidence::new(
            "versions",
            legacy
                .iter()
                .map(|t| t.version.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )],
        remediation: "Build and sign v5/v6 transactions. If you have a v4 transaction \
                      pending, re-create it in a supported format; signatures do not carry \
                      over."
            .into(),
        txids: legacy.iter().map(|t| t.txid.clone()).collect(),
        references: vec![
            "https://zips.z.cash/zip-0259".into(),
            "https://zips.z.cash/zip-2003".into(),
        ],
    }]
}

fn memo_fingerprint(subject: &Subject) -> Vec<Finding> {
    let Some(wallet) = &subject.wallet else {
        return Vec::new();
    };
    let mut lens: Vec<u32> = wallet
        .notes
        .iter()
        .map(|n| n.memo_len)
        .filter(|l| *l > 0)
        .collect();
    lens.sort_unstable();
    lens.dedup();
    if lens.len() < 2 {
        return Vec::new();
    }
    vec![Finding {
        id: "LB-012".into(),
        title: "Memo-length fingerprint".into(),
        severity: Severity::Low,
        confidence: Confidence::Low,
        category: Category::Fingerprint,
        fatal: false,
        summary: format!(
            "This wallet's notes carry {} distinct non-empty memo lengths ({}). Memo content \
             is encrypted, but the size pattern is not, and it separates payments into \
             classes (invoice, payroll, chat).",
            lens.len(),
            lens.iter()
                .map(|l| format!("{l}B"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        evidence: vec![Evidence::new(
            "memo lengths",
            lens.iter()
                .map(|l| format!("{l}B"))
                .collect::<Vec<_>>()
                .join(", "),
        )],
        remediation: "Pad memos to a fixed length (512 bytes is the field size) so that message \
                      class is not inferable from size."
            .into(),
        txids: vec![],
        references: vec!["https://zips.z.cash/zip-0302".into()],
    }]
}

fn unique_note_value(subject: &Subject) -> Vec<Finding> {
    let Some(wallet) = &subject.wallet else {
        return Vec::new();
    };
    if subject.peers.is_empty() {
        return Vec::new();
    }
    let mut peer_amounts: BTreeMap<i128, usize> = BTreeMap::new();
    for p in &subject.peers {
        if let Some(a) = p.amount_zat {
            *peer_amounts.entry(a).or_default() += 1;
        }
    }
    let mut seen = BTreeMap::new();
    let mut out = Vec::new();
    for note in &wallet.notes {
        if note.value_zat < 1_000_000 {
            continue;
        }
        if seen.insert(note.value_zat, ()).is_some() {
            continue;
        }
        let k = peer_amounts
            .get(&i128::from(note.value_zat))
            .copied()
            .unwrap_or(0);
        if k >= 3 {
            continue;
        }
        out.push(Finding {
            id: "LB-013".into(),
            title: "Unique note value".into(),
            severity: if k == 0 {
                Severity::High
            } else {
                Severity::Medium
            },
            confidence: Confidence::Medium,
            category: Category::Linkage,
            fatal: false,
            summary: format!(
                "A note of {} is held by {} of the {} observed notes from other wallets. When \
                 a value is unique, an observer can follow the amount rather than the address.",
                format_zec(i128::from(note.value_zat)),
                k,
                subject.peers.len()
            ),
            evidence: vec![
                Evidence::new("note value", format_zec(i128::from(note.value_zat))),
                Evidence::new("peer notes with same value", k.to_string()),
                Evidence::new("peer sample", subject.peers.len().to_string()),
                Evidence::new("txid", note.txid.clone()),
            ],
            remediation: "Break unique denominations: split incoming funds into a small set of \
                          common denominations, or receive into a fresh account whose value \
                          histogram resembles everyone else's."
                .into(),
            txids: vec![note.txid.clone()],
            references: vec![],
        });
    }
    out
}

fn pool_fragmentation(subject: &Subject) -> Vec<Finding> {
    let Some(wallet) = &subject.wallet else {
        return Vec::new();
    };
    let pools = wallet.pools_held();
    if pools.len() < 3 {
        return Vec::new();
    }
    let evidence: Vec<Evidence> = pools
        .iter()
        .map(|p| Evidence::new(p.title(), format_zec(wallet.total_notes_zat(*p))))
        .collect();
    vec![Finding {
        id: "LB-014".into(),
        title: "Pool fragmentation".into(),
        severity: Severity::Medium,
        confidence: Confidence::High,
        category: Category::Fingerprint,
        fatal: false,
        summary: format!(
            "This wallet holds notes in {} pools ({}). A wallet present in Sapling, Orchard \
             and Ironwood documents its own migration path, and each pool's activity is \
             separately observable.",
            pools.len(),
            pools
                .iter()
                .map(|p| p.title())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        evidence,
        remediation: "Consolidate into the current pool (Ironwood) through a single \
                      carefully-sized crossing, and stop using retired pools for new \
                      activity."
            .into(),
        txids: vec![],
        references: vec!["https://zips.z.cash/zip-0258".into()],
    }]
}

fn full_balance_migration(subject: &Subject) -> Vec<Finding> {
    let Some(wallet) = &subject.wallet else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for tx in &subject.txs {
        let Some((from, _, amount)) = tx.cross_pool_move() else {
            continue;
        };
        let total = wallet.total_notes_zat(from);
        let source_notes = wallet.notes.iter().filter(|n| n.pool == from).count();
        if source_notes < 2 || total <= 0 || amount as i128 != total {
            continue;
        }
        out.push(Finding {
            id: "LB-015".into(),
            title: "Full-balance migration".into(),
            severity: Severity::High,
            confidence: Confidence::Medium,
            category: Category::Leak,
            fatal: false,
            summary: format!(
                "The crossing equals this wallet's entire {} holding ({} across {} notes). \
                 Amount disclosure is one problem; disclosing that the amount is your whole \
                 balance is a second, worse one.",
                from.title(),
                format_zec(total),
                source_notes
            ),
            evidence: vec![
                Evidence::new("crossed", format_zec(amount as i128)),
                Evidence::new("wallet total in pool", format_zec(total)),
                Evidence::new("notes consumed", source_notes.to_string()),
                Evidence::new("txid", tx.txid.clone()),
            ],
            remediation: "Migrate in chunks that are individually unremarkable, and leave a \
                          remainder behind initially. A full-balance crossing links the new \
                          account to the entire history of the old one."
                .into(),
            txids: vec![tx.txid.clone()],
            references: vec!["https://zips.z.cash/zip-0258".into()],
        });
    }
    out
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn height_or_unknown(tx: &TxFacts) -> String {
    tx.block_height
        .map(format_height)
        .unwrap_or_else(|| "mempool".into())
}

fn roundness(zat: i128) -> Option<&'static str> {
    let a = zat.unsigned_abs();
    if a == 0 {
        return None;
    }
    if a % 100_000_000 == 0 {
        Some("Whole-ZEC")
    } else if a % 10_000_000 == 0 {
        Some("One-decimal-ZEC")
    } else {
        None
    }
}

pub fn short_hex(s: &str) -> String {
    if s.len() <= 22 {
        s.to_string()
    } else {
        format!("{}…{}", &s[..12], &s[s.len() - 6..])
    }
}

/// Convenience for the `heuristics` CLI command.
pub fn catalogue() -> Vec<(&'static str, &'static str, &'static str)> {
    RULES
        .iter()
        .map(|r| (r.id, r.title, r.description))
        .collect()
}
