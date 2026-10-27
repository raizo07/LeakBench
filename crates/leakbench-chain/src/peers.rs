//! Anonymity-set sampling.
//!
//! Rules LB-002 and LB-013 need to know what *other* people were doing in the
//! same window. Without that, they can only say "unmeasured". This module turns a
//! block range into a [`PeerSet`]: one summary per transaction, with the shape,
//! the visible amount and the kind of move.
//!
//! Sampling is deliberately local and explicit. There is no background crawl and
//! no remote service: you point it at your own node and a range you choose.

use leakbench_core::facts::{format_zec, PeerSummary, TxFacts};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{decode, ChainError};

pub const PEER_SCHEMA: &str = "leakbench.peers.v1";

/// Refuse absurd ranges rather than hammering a node for an hour.
pub const MAX_BLOCKS: u64 = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSet {
    pub schema: String,
    pub network: String,
    pub from_height: u64,
    pub to_height: u64,
    pub block_interval_secs: u64,
    pub peers: Vec<PeerSummary>,
}

impl PeerSet {
    pub fn count(&self, kind: &str) -> usize {
        self.peers
            .iter()
            .filter(|p| p.kind.as_deref() == Some(kind))
            .count()
    }

    pub fn amounts(&self, kind: &str) -> Vec<i128> {
        let mut v: Vec<i128> = self
            .peers
            .iter()
            .filter(|p| p.kind.as_deref() == Some(kind))
            .filter_map(|p| p.amount_zat)
            .collect();
        v.sort_unstable();
        v
    }
}

/// Anything that can answer "give me block N". Kept as a trait so peer
/// collection can be tested against canned blocks with no node in sight.
pub trait BlockSource {
    /// Block JSON at verbosity 2 (transactions decoded).
    fn block_at(&self, height: u64) -> Result<Value, ChainError>;
}

impl BlockSource for crate::rpc::Rpc {
    fn block_at(&self, height: u64) -> Result<Value, ChainError> {
        let hash = self.block_hash(height)?;
        let block = self.block(&hash, 2)?;
        Ok(block)
    }
}

pub fn collect<S: BlockSource + ?Sized>(
    source: &S,
    from: u64,
    to: u64,
    network: &str,
    block_interval_secs: u64,
) -> Result<PeerSet, ChainError> {
    if to < from {
        return Err(ChainError::Invalid(format!(
            "--from {from} is after --to {to}"
        )));
    }
    let span = to - from + 1;
    if span > MAX_BLOCKS {
        return Err(ChainError::Invalid(format!(
            "range covers {span} blocks; the limit is {MAX_BLOCKS}. Narrow the window: \
             anonymity sets are local anyway."
        )));
    }

    let mut peers = Vec::new();
    for height in from..=to {
        let block = source.block_at(height)?;
        let tx_field = block.get("tx").ok_or_else(|| {
            ChainError::Invalid(format!(
                "block {height} has no `tx` field; ask for verbosity 2"
            ))
        })?;
        let Some(txs) = tx_field.as_array() else {
            return Err(ChainError::Invalid(format!(
                "block {height} returned non-array `tx`; verbosity 2 is required"
            )));
        };
        for tx in txs {
            if !tx.is_object() {
                return Err(ChainError::Invalid(
                    "block returned transaction ids instead of objects; use getblock verbosity 2"
                        .into(),
                ));
            }
            let facts = decode::tx_from_node_json(tx, Some(height), None)?;
            peers.push(peer_from_tx(&facts, height));
        }
    }

    Ok(PeerSet {
        schema: PEER_SCHEMA.to_string(),
        network: network.to_string(),
        from_height: from,
        to_height: to,
        block_interval_secs,
        peers,
    })
}

fn peer_from_tx(tx: &TxFacts, height: u64) -> PeerSummary {
    let (amount_zat, kind) = if let Some((_, _, amount)) = tx.cross_pool_move() {
        (Some(amount as i128), "migration")
    } else if tx.shielded_spend_count() > 0 || tx.shielded_output_count() > 0 {
        (tx.transparent.value_out_zat, "payment")
    } else {
        (tx.transparent.value_out_zat, "transparent")
    };
    PeerSummary {
        txid: tx.txid.clone(),
        height,
        shape: tx.shape(),
        amount_zat,
        kind: Some(kind.to_string()),
    }
}

/// A short text histogram of crossing amounts, in powers of two. Used by the CLI
/// to show why an amount is or is not hidden in a crowd.
pub fn size_bands(amounts: &[i128]) -> Vec<(String, usize)> {
    const ZEC: i128 = 100_000_000;
    let mut bands = vec![
        ("< 0.1 ZEC", 0usize),
        ("0.1 – 0.5", 0),
        ("0.5 – 1", 0),
        ("1 – 2", 0),
        ("2 – 5", 0),
        ("5 – 10", 0),
        ("≥ 10", 0),
    ];
    for amount in amounts {
        let zec = *amount as f64 / ZEC as f64;
        let index = if zec < 0.1 {
            0
        } else if zec < 0.5 {
            1
        } else if zec < 1.0 {
            2
        } else if zec < 2.0 {
            3
        } else if zec < 5.0 {
            4
        } else if zec < 10.0 {
            5
        } else {
            6
        };
        bands[index].1 += 1;
    }
    bands
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(label, n)| (label.to_string(), n))
        .collect()
}

/// Human summary used by `leakbench peers`.
pub fn describe(set: &PeerSet) -> String {
    let migrations = set.count("migration");
    let payments = set.count("payment");
    let transparent = set.count("transparent");
    let mut out = format!(
        "{} peers over blocks {}–{}: {migrations} migration(s), {payments} shielded payment(s), \
         {transparent} transparent\n",
        set.peers.len(),
        set.from_height,
        set.to_height
    );
    let crossing_amounts = set.amounts("migration");
    if !crossing_amounts.is_empty() {
        let total: i128 = crossing_amounts.iter().sum();
        out.push_str(&format!(
            "migration amounts: median {} · total {}\n",
            format_zec(crossing_amounts[crossing_amounts.len() / 2]),
            format_zec(total)
        ));
        for (band, count) in size_bands(&crossing_amounts) {
            out.push_str(&format!("  {band:>12}  {}\n", "▇".repeat(count.min(40))));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fake {
        blocks: Vec<Value>,
        from: u64,
    }

    impl BlockSource for Fake {
        fn block_at(&self, height: u64) -> Result<Value, ChainError> {
            let index = (height - self.from) as usize;
            self.blocks
                .get(index)
                .cloned()
                .ok_or_else(|| ChainError::Invalid("no such block".into()))
        }
    }

    fn crossing_tx() -> Value {
        json!({
            "txid": "aa".repeat(32),
            "version": 6,
            "vin": [], "vout": [],
            "orchard": { "actions": [1, 2], "valueBalanceZat": 500000000 },
            "ironwood": { "actions": [1], "valueBalanceZat": -500000000 },
            "fee": 0.0001
        })
    }

    fn payment_tx() -> Value {
        json!({
            "txid": "bb".repeat(32),
            "version": 6,
            "vin": [],
            "vout": [{ "valueZat": 25000000, "scriptPubKey": { "addresses": ["t1abc"] } }],
            "orchard": { "actions": [1], "valueBalanceZat": 25010000 },
            "fee": 0.0001
        })
    }

    #[test]
    fn collects_and_classifies() {
        let fake = Fake {
            from: 100,
            blocks: vec![json!({ "tx": [crossing_tx(), payment_tx()] })],
        };
        let set = collect(&fake, 100, 100, "main", 75).unwrap();
        assert_eq!(set.peers.len(), 2);
        assert_eq!(set.count("migration"), 1);
        assert_eq!(set.count("payment"), 1);
        assert_eq!(set.amounts("migration"), vec![500_000_000]);
        assert!(set.peers[0].height == 100);
    }

    #[test]
    fn rejects_verbose_one_blocks() {
        let fake = Fake {
            from: 1,
            blocks: vec![json!({ "tx": ["aa".repeat(32)] })],
        };
        let err = collect(&fake, 1, 1, "main", 75).unwrap_err();
        assert!(err.to_string().contains("verbosity 2"));
    }

    #[test]
    fn rejects_backwards_and_huge_ranges() {
        let fake = Fake {
            from: 1,
            blocks: vec![],
        };
        assert!(collect(&fake, 10, 5, "main", 75).is_err());
        assert!(collect(&fake, 1, 1 + MAX_BLOCKS, "main", 75).is_err());
    }

    #[test]
    fn size_bands_bucket_amounts() {
        let bands = size_bands(&[5_000_000, 300_000_000, 1_200_000_000]);
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0], ("< 0.1 ZEC".to_string(), 1));
        assert_eq!(bands[1], ("2 – 5".to_string(), 1));
        assert_eq!(bands[2], ("≥ 10".to_string(), 1));
    }
}
