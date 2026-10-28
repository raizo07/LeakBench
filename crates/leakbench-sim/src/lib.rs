//! Deterministic Zcash scenarios.
//!
//! A scanner is only as credible as the transactions you point it at, and a
//! hackathon demo cannot depend on someone else's testnet faucet. So LeakBench
//! ships two fully specified wallets with real shapes:
//!
//! * **naive** — migrates an entire Orchard balance across the Ironwood
//!   turnstile in one transaction, one block after being paid, then unshields
//!   to a transparent address and reuses it. This is what a wallet written by
//!   someone who thinks "shielded = private" does.
//! * **hardened** — the same starting position, migrated in five non-round
//!   chunks during busy windows, with conventional fees and no transparent legs.
//!
//! The hardened wallet still scores below 100, on purpose: the turnstile
//! publishes the amount of every crossing. The point of the tool is to tell you
//! which disclosures are *identifying* and which are merely *public*.
//!
//! Everything is generated from a fixed seed, so reports diff cleanly.

use leakbench_core::facts::{ChainContext, Pool, PoolFacts, Source, SubjectKind};
use leakbench_core::{AddressFacts, NoteFacts, PeerSummary, Subject, TxFacts, WalletFacts};

/// Mainnet height used as the scenario origin. Roughly late September 2026,
/// after Ironwood (NU6.3) and before NU7 at height 3,543,405.
pub const BASE_HEIGHT: u64 = 3_500_000;
/// 2026-09-28T00:00:00Z, the wall-clock time of `BASE_HEIGHT`.
pub const BASE_TIME: i64 = 1_790_553_600;
/// Pre-NU7 target spacing. NU7 (5 Nov 2026) drops mainnet to 25s.
pub const BLOCK_INTERVAL: u64 = 75;

const ZEC: i128 = 100_000_000;

pub struct Scenario {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub subject: Subject,
}

pub fn all() -> Vec<Scenario> {
    vec![naive(), hardened()]
}

pub fn get(id: &str) -> Option<Scenario> {
    all().into_iter().find(|s| s.id == id)
}

// ---------------------------------------------------------------------------
// naive
// ---------------------------------------------------------------------------

pub fn naive() -> Scenario {
    let h = BASE_HEIGHT;
    let mut rng = Rng::new(0x1EA6_B3C0);
    let mut txs = Vec::with_capacity(8);

    // Two payroll receipts, both plain 2-in/2-out Orchard transfers.
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        orchard: shielded(2, 2, 10_000),
        label: Some("payroll #2211".into()),
        anchor_age_blocks: Some(1),
        ..tx("payroll-2211", h, &mut rng)
    });
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        orchard: shielded(2, 2, 10_000),
        label: Some("payroll #2237".into()),
        anchor_age_blocks: Some(1),
        ..tx("payroll-2237", h + 40, &mut rng)
    });

    // The mistake: move everything, one block later, in one crossing.
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        orchard: shielded(2, 2, 5 * ZEC),
        ironwood: shielded(1, 1, -5 * ZEC),
        label: Some("migrate full balance".into()),
        anchor_age_blocks: Some(130),
        ..tx("migrate-all", h + 41, &mut rng)
    });

    // A payment from the new pool, at a fee the wallet hard-coded.
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        ironwood: shielded(2, 2, 10_000),
        expiry_height: Some(h + 41 + 40),
        label: Some("merchant invoice #114".into()),
        anchor_age_blocks: Some(1),
        ..tx("invoice-114", h + 42, &mut rng)
    });

    // Unshielding to an exchange deposit address: amount, address, timing public.
    const DEPOSIT: &str = "t1UnExChangeDepositAddr9kQ";
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        ironwood: shielded(1, 1, ZEC + 10_000),
        transparent: PoolFacts {
            spends: 0,
            outputs: 1,
            value_in_zat: Some(0),
            value_out_zat: Some(ZEC),
            addresses: vec![DEPOSIT.to_string()],
            ..Default::default()
        },
        label: Some("exchange deposit".into()),
        anchor_age_blocks: Some(2),
        ..tx("unshield-deposit", h + 43, &mut rng)
    });

    // Then it reuses the same transparent address for change.
    txs.push(TxFacts {
        fee_zat: Some(10_000),
        transparent: PoolFacts {
            spends: 1,
            outputs: 2,
            value_in_zat: Some(ZEC),
            value_out_zat: Some(99_990_000),
            addresses: vec![DEPOSIT.to_string(), "t1FreshMerchantPayAddr2x".to_string()],
            ..Default::default()
        },
        label: Some("transparent change".into()),
        ..tx("transparent-change", h + 44, &mut rng)
    });

    // A pre-NU7 v4 transaction in the scan window.
    txs.push(TxFacts {
        version: 4,
        fee_zat: Some(10_000),
        transparent: PoolFacts {
            spends: 1,
            outputs: 2,
            value_in_zat: Some(200_000_000),
            value_out_zat: Some(199_990_000),
            addresses: vec!["t1LegacyHotWalletAddr77".to_string()],
            ..Default::default()
        },
        label: Some("legacy v4 spend".into()),
        ..tx("legacy-v4", h - 1_000, &mut rng)
    });

    let wallet = WalletFacts {
        label: "ZODL-shaped demo wallet".into(),
        network: "main".into(),
        birthday_height: h - 2_000,
        notes: vec![
            note(&txs[0].txid, Pool::Orchard, 34 * ZEC / 10, h, 512),
            note(&txs[1].txid, Pool::Orchard, 16 * ZEC / 10, h + 40, 216),
            note(
                &fake_txid("sapling-2019", &mut rng),
                Pool::Sapling,
                20_000_000,
                h - 18_000,
                0,
            ),
            note(&txs[3].txid, Pool::Ironwood, 1_500_000_000, h + 42, 0),
        ],
        addresses: vec![AddressFacts {
            address: DEPOSIT.to_string(),
            pool: Pool::Transparent,
            uses: 2,
            first_height: h + 43,
            last_height: h + 44,
        }],
    };

    // A thin migration wave: two other users crossed in the same window, with
    // amounts that do not overlap the 5 ZEC being moved here.
    let mut peers = Vec::with_capacity(48);
    for (i, amount) in [123_450_000i128, 777_000_000, 240_000_000]
        .iter()
        .enumerate()
    {
        peers.push(PeerSummary {
            txid: format!("peer-naive-{i}"),
            height: h + 38 + i as u64,
            shape: "t0/0 s0/0 o1/1 i1/1".into(),
            amount_zat: Some(*amount),
            kind: Some("migration".into()),
        });
    }

    let mut subject = Subject::new("naive wallet", SubjectKind::Scenario, "main");
    subject.description = Some(
        "A wallet that moved its whole Orchard balance across the turnstile in one \
         transaction, one block after being paid, then unshielded to an exchange."
            .into(),
    );
    subject.txs = txs;
    subject.wallet = Some(wallet);
    subject.peers = peers;
    subject.context = ChainContext {
        block_interval_secs: BLOCK_INTERVAL,
        tip_height: Some(h + 200),
    };

    Scenario {
        id: "naive",
        title: "Naive migration",
        description: "Full-balance turnstile crossing, immediate unshield, address reuse.",
        subject,
    }
}

// ---------------------------------------------------------------------------
// hardened
// ---------------------------------------------------------------------------

pub fn hardened() -> Scenario {
    let h = BASE_HEIGHT;
    let mut rng = Rng::new(0x600D_B33F);
    let mut txs = Vec::with_capacity(8);

    // Five irregular payroll receipts, spread over two weeks.
    let receipts: [(u64, i128); 5] = [
        (0, 112_730_000),
        (1_500, 84_190_000),
        (3_100, 140_970_000),
        (5_400, 61_830_000),
        (8_800, 99_990_000),
    ];
    for (i, (offset, _value)) in receipts.iter().enumerate() {
        txs.push(TxFacts {
            fee_zat: Some(10_000),
            orchard: shielded(2, 2, 10_000),
            label: Some(format!("payroll #{}", 5100 + i)),
            anchor_age_blocks: Some(1),
            ..tx(&format!("payroll-{i}"), h + *offset, &mut rng)
        });
    }

    // Five crossings, 1-in/1-out, non-round amounts, months apart in blocks,
    // each during a busy window with peers in the same size band.
    let crossings: [(u64, i128); 5] = [
        (12_000, 112_730_000),
        (13_800, 84_190_000),
        (16_300, 140_970_000),
        (21_600, 61_830_000),
        (27_200, 99_990_000),
    ];
    let mut peers = Vec::with_capacity(48);
    for (i, (offset, value)) in crossings.iter().enumerate() {
        txs.push(TxFacts {
            fee_zat: Some(10_000),
            orchard: shielded(1, 1, *value),
            ironwood: shielded(1, 1, -*value),
            label: Some(format!("crossing {}", i + 1)),
            anchor_age_blocks: Some(0),
            ..tx(&format!("crossing-{i}"), h + *offset, &mut rng)
        });
        // Five other users crossed in the same window, in the same size band.
        for k in 0..5 {
            let jitter = 90 + (k as i128 * 3) - 6; // 84..=95%
            peers.push(PeerSummary {
                txid: format!("peer-hard-{i}-{k}"),
                height: h + *offset + (k as u64),
                shape: "t0/0 s0/0 o1/1 i1/1".into(),
                amount_zat: Some(value * jitter / 100),
                kind: Some("migration".into()),
            });
        }
        // And some peers hold exactly the same note values, so the amount is
        // not a unique denomination.
        for k in 0..3 {
            peers.push(PeerSummary {
                txid: format!("peer-note-{i}-{k}"),
                height: h + *offset - 5_000 + k,
                shape: "t0/0 s0/0 o2/2 i0/0".into(),
                amount_zat: Some(*value),
                kind: Some("payment".into()),
            });
        }
    }

    // Ordinary shielded payments from Ironwood, well separated in time.
    for (i, (offset, value)) in [
        (31_000u64, 300_000_000i128),
        (34_000, 150_000_000),
        (39_000, 450_000_000),
    ]
    .iter()
    .enumerate()
    {
        txs.push(TxFacts {
            fee_zat: Some(10_000),
            ironwood: shielded(2, 2, *value - 10_000),
            label: Some(format!("merchant invoice #{}", 900 + i)),
            anchor_age_blocks: Some(1),
            ..tx(&format!("invoice-{i}"), h + *offset, &mut rng)
        });
    }

    let mut notes: Vec<NoteFacts> = receipts
        .iter()
        .enumerate()
        .map(|(i, (offset, value))| note(&txs[i].txid, Pool::Orchard, *value, h + *offset, 512))
        .collect();
    // Change from the payments lands back in Ironwood.
    notes.push(note(
        &txs[10].txid,
        Pool::Ironwood,
        84_190_000,
        h + 31_000,
        512,
    ));

    let wallet = WalletFacts {
        label: "hardened demo wallet".into(),
        network: "main".into(),
        birthday_height: h - 2_000,
        notes,
        addresses: vec![],
    };

    let mut subject = Subject::new("hardened wallet", SubjectKind::Scenario, "main");
    subject.description = Some(
        "The same starting position, migrated as five non-round chunks during busy \
         windows, with conventional fees and no transparent legs."
            .into(),
    );
    subject.txs = txs;
    subject.wallet = Some(wallet);
    subject.peers = peers;
    subject.context = ChainContext {
        block_interval_secs: BLOCK_INTERVAL,
        tip_height: Some(h + 40_000),
    };

    Scenario {
        id: "hardened",
        title: "Hardened migration",
        description: "Chunked, non-round, busy-window crossings. No transparent leg.",
        subject,
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn note(txid: &str, pool: Pool, value_zat: i128, height: u64, memo_len: u32) -> NoteFacts {
    NoteFacts {
        txid: txid.to_string(),
        pool,
        value_zat: value_zat as u64,
        received_height: height,
        memo_len,
        is_change: false,
    }
}

fn shielded(spends: u32, outputs: u32, value_balance_zat: i128) -> PoolFacts {
    PoolFacts {
        spends,
        outputs,
        value_balance_zat: Some(value_balance_zat),
        ..Default::default()
    }
}

fn tx(seed: &str, height: u64, rng: &mut Rng) -> TxFacts {
    TxFacts {
        txid: fake_txid(seed, rng),
        version: 6,
        block_height: Some(height),
        block_time: Some(BASE_TIME + (height as i64 - BASE_HEIGHT as i64) * BLOCK_INTERVAL as i64),
        fee_zat: None,
        transparent: PoolFacts::default(),
        sapling: PoolFacts::default(),
        orchard: PoolFacts::default(),
        ironwood: PoolFacts::default(),
        expiry_height: None,
        lock_time: None,
        anchor_age_blocks: None,
        memo_lens: vec![],
        label: None,
        source: Source::Simulated,
    }
}

/// A stable 64-hex pseudo-txid derived from a label. Deterministic across
/// runs and platforms, which keeps report diffs readable.
pub fn fake_txid(seed: &str, rng: &mut Rng) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    let a = h ^ rng.next_u64();
    let b = a.rotate_left(17) ^ rng.next_u64();
    let c = b.rotate_left(29) ^ 0x9e37_79b9_7f4a_7c15;
    let d = c.wrapping_mul(0xff51_afd7_ed55_8ccd);
    format!("{a:016x}{b:016x}{c:016x}{d:016x}")
}

/// splitmix64: tiny, deterministic, good enough for scenario jitter.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.next_u64() % (hi - lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leakbench_core::{analyze, Verdict};

    #[test]
    fn scenarios_are_deterministic() {
        let a = naive().subject.txs[0].txid.clone();
        let b = naive().subject.txs[0].txid.clone();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn naive_wallet_is_disqualified() {
        let report = analyze(&naive().subject);
        assert_eq!(report.verdict, Verdict::Disqualified);
        assert!(report.findings.iter().any(|f| f.fatal));
        assert!(report.findings.iter().any(|f| f.id == "LB-001"));
        assert!(report.findings.iter().any(|f| f.id == "LB-003"));
    }

    #[test]
    fn hardened_wallet_has_no_fatal_findings() {
        let report = analyze(&hardened().subject);
        assert!(
            !report.findings.iter().any(|f| f.fatal),
            "{:?}",
            report.findings
        );
        assert_ne!(report.verdict, Verdict::Disqualified);
        assert!(report.score.value > 70, "score {}", report.score.value);
    }

    #[test]
    fn hardened_beats_naive_by_a_wide_margin() {
        let n = analyze(&naive().subject).score.value;
        let h = analyze(&hardened().subject).score.value;
        assert!(h > n + 40, "naive {n} vs hardened {h}");
    }

    #[test]
    fn scenario_txids_are_unique() {
        for s in all() {
            let mut ids: Vec<&str> = s.subject.txs.iter().map(|t| t.txid.as_str()).collect();
            let before = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(before, ids.len(), "duplicate txid in {}", s.id);
        }
    }
}
