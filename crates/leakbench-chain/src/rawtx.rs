//! Parse raw transaction bytes into facts, using the reference implementation.
//!
//! LeakBench deliberately does not hand-roll a consensus decoder: a parser that
//! is 99% right produces confident false findings. This module uses
//! `zcash_primitives` — the same code a node uses to read transactions — so the
//! facts are the ones the protocol actually encodes.
//!
//! Input is the hex a node returns from `getrawtransaction <txid> 0`, or the
//! `raw_transaction` field of an explorer API. That makes the analyzer usable
//! without a synced node and without trusting anyone's decoded JSON.
//!
//! Known gap: transparent output addresses are not extracted here (that needs
//! script interpretation the node normally does), so LB-008 does not fire on
//! the raw path. Everything shielded is exact.

use std::io::Cursor;

use leakbench_core::facts::{PoolFacts, Source, TxFacts};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BranchId;

use crate::ChainError;

/// Decode a hex string (whitespace tolerated) into bytes.
fn unhex(input: &str) -> Result<Vec<u8>, ChainError> {
    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err(ChainError::Invalid("odd number of hex digits".into()));
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let mut hi: Option<u8> = None;
    for byte in clean {
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(ChainError::Invalid(format!("not hex: {:?}", byte as char))),
        };
        match hi {
            None => hi = Some(nibble),
            Some(h) => {
                out.push((h << 4) | nibble);
                hi = None;
            }
        }
    }
    Ok(out)
}

fn version_u8(tx: &Transaction) -> u8 {
    use zcash_primitives::transaction::TxVersion::*;
    match tx.version() {
        Sprout(_) | V3 => 4,
        V4 => 4,
        V5 => 5,
        V6 => 6,
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

/// Build [`TxFacts`] from raw transaction bytes.
pub fn tx_from_bytes(bytes: &[u8]) -> Result<TxFacts, ChainError> {
    // v5/v6 carry their own consensus branch id; the argument only matters for
    // v4, which the analyzer treats as a legacy shape anyway.
    let tx = Transaction::read(Cursor::new(bytes), BranchId::Nu6_3)
        .map_err(|e| ChainError::Invalid(format!("not a readable transaction: {e}")))?;

    let txid = tx.txid().to_string();

    let (t_spends, t_outputs, t_out) = match tx.transparent_bundle() {
        Some(bundle) => {
            let outputs = bundle.vout.len() as u32;
            let mut value: i128 = 0;
            for out in &bundle.vout {
                value += i128::from(u64::from(out.value()));
            }
            (bundle.vin.len() as u32, outputs, value)
        }
        None => (0, 0, 0),
    };

    let transparent = PoolFacts {
        spends: t_spends,
        outputs: t_outputs,
        value_balance_zat: None,
        // Input values are not in the transaction; without the prevouts the
        // analyzer keeps them absent rather than guessing.
        value_in_zat: if t_spends == 0 { Some(0) } else { None },
        value_out_zat: if t_out > 0 { Some(t_out) } else { None },
        addresses: Vec::new(),
    };

    let sapling = match tx.sapling_bundle() {
        Some(bundle) => shielded(
            bundle.shielded_spends().len() as u32,
            bundle.shielded_outputs().len() as u32,
            i64::from(bundle.value_balance()) as i128,
        ),
        None => PoolFacts::default(),
    };

    let orchard = match tx.orchard_bundle() {
        Some(bundle) => shielded(
            bundle.actions().len() as u32,
            bundle.actions().len() as u32,
            i64::from(bundle.value_balance()) as i128,
        ),
        None => PoolFacts::default(),
    };

    let ironwood = match tx.ironwood_bundle() {
        Some(bundle) => shielded(
            bundle.actions().len() as u32,
            bundle.actions().len() as u32,
            i64::from(bundle.value_balance()) as i128,
        ),
        None => PoolFacts::default(),
    };

    // Fully shielded transactions know their fee from the value balances; ones
    // with transparent inputs need prevouts we do not have.
    let fee_zat = tx
        .fee_paid(|_| Ok::<_, zcash_protocol::value::BalanceError>(None))
        .ok()
        .flatten()
        .map(u64::from);

    Ok(TxFacts {
        txid,
        version: version_u8(&tx),
        block_height: None,
        block_time: None,
        fee_zat,
        transparent,
        sapling,
        orchard,
        ironwood,
        expiry_height: {
            let h: u32 = tx.expiry_height().into();
            if h == 0 {
                None
            } else {
                Some(u64::from(h))
            }
        },
        lock_time: Some(tx.lock_time()),
        anchor_age_blocks: None,
        memo_lens: Vec::new(),
        label: None,
        source: Source::Raw,
    })
}

/// Hex in, facts out.
pub fn tx_from_hex(hex: &str) -> Result<TxFacts, ChainError> {
    let bytes = unhex(hex)?;
    tx_from_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_hex() {
        assert!(tx_from_hex("zz").is_err());
        assert!(tx_from_hex("abc").is_err());
    }

    #[test]
    fn decodes_whitespace_tolerantly() {
        assert_eq!(unhex("de ad\nbe ef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn rejects_non_transactions() {
        // 32 zero bytes is not a transaction.
        let err = tx_from_bytes(&[0u8; 32]).unwrap_err();
        assert!(err.to_string().contains("not a readable transaction"));
    }
}
