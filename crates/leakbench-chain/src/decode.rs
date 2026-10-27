//! Map a node's decoded transaction JSON onto [`TxFacts`].
//!
//! The keys differ between `zcashd`, `zebrad` and `lightwalletd`-adjacent
//! tooling, and they change between releases. Rather than pin one shape, the
//! decoder accepts a set of aliases per field and reports what it could not
//! find, so a partial mapping degrades into lower-confidence findings instead
//! of a wrong one.

use leakbench_core::facts::{PoolFacts, Source, TxFacts};
use serde_json::Value;

use crate::ChainError;

fn num(v: &Value, keys: &[&str]) -> Option<i128> {
    for k in keys {
        if let Some(x) = v.get(*k) {
            if let Some(i) = x.as_i64() {
                return Some(i128::from(i));
            }
            if let Some(u) = x.as_u64() {
                return Some(i128::from(u));
            }
            if let Some(f) = x.as_f64() {
                return Some(f.round() as i128);
            }
            if let Some(s) = x.as_str() {
                if let Ok(parsed) = s.parse::<i128>() {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

/// A decimal number, without rounding. Needed for ZEC-denominated fields:
/// routing `0.00005` through the integer helper would round it to zero.
fn decimal(v: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(x) = v.get(*k) {
            if let Some(f) = x.as_f64() {
                return Some(f);
            }
            if let Some(i) = x.as_i64() {
                return Some(i as f64);
            }
            if let Some(s) = x.as_str() {
                if let Ok(f) = s.parse::<f64>() {
                    return Some(f);
                }
            }
        }
    }
    None
}

/// Values are given either as zats (`valueZat`, `valueBalanceZat`) or as ZEC
/// with eight decimals (`value`, `valueBalance`). The two are not
/// interchangeable: a ZEC field must be scaled, not rounded.
fn zats(v: &Value, zat_keys: &[&str], zec_keys: &[&str]) -> Option<i128> {
    if let Some(z) = num(v, zat_keys) {
        return Some(z);
    }
    decimal(v, zec_keys).map(|zec| (zec * 100_000_000.0).round() as i128)
}

fn count(v: &Value, keys: &[&str]) -> u32 {
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr.len() as u32;
        }
        if let Some(n) = num(v, &[*k]) {
            return n.max(0) as u32;
        }
    }
    0
}

fn addresses(vout: Option<&Vec<Value>>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(vout) = vout else { return out };
    for o in vout {
        // `zcashd` and `zebrad` nest addresses under scriptPubKey; some tools
        // hoist them to the output itself. Accept both.
        let script = o.get("scriptPubKey").unwrap_or(o);
        if let Some(list) = script.get("addresses").and_then(|a| a.as_array()) {
            for a in list {
                if let Some(s) = a.as_str() {
                    out.push(s.to_string());
                }
            }
        } else if let Some(s) = script.get("address").and_then(|a| a.as_str()) {
            out.push(s.to_string());
        }
    }
    out
}

/// Read the shielded sections, tolerating both the modern nested objects and
/// the older flat `vShieldedSpend` / `valueBalance` fields.
fn pool(v: &Value, names: &[&str], flat_spends: &[&str], flat_outs: &[&str]) -> PoolFacts {
    let mut facts = PoolFacts::default();
    let mut found = false;
    for name in names {
        let Some(section) = v.get(*name) else {
            continue;
        };
        if section.is_null() {
            continue;
        }
        found = true;
        facts.spends += count(section, &["spends", "actions", "vShieldedSpend"]);
        facts.outputs += count(section, &["outputs", "actions", "vShieldedOutput"]);
        if let Some(vb) = zats(
            section,
            &["valueBalanceZat", "value_balance_zat"],
            &["valueBalance", "value_balance"],
        ) {
            facts.value_balance_zat = Some(vb);
        }
    }
    if !found {
        facts.spends = count(v, flat_spends);
        facts.outputs = count(v, flat_outs);
    }
    facts
}

/// Convert one decoded node transaction into facts.
///
/// `block_height` and `block_time` are supplied by the caller because
/// `getrawtransaction` reports them only when the node has the block index.
pub fn tx_from_node_json(
    v: &Value,
    block_height: Option<u64>,
    block_time: Option<i64>,
) -> Result<TxFacts, ChainError> {
    let txid = v
        .get("txid")
        .and_then(|x| x.as_str())
        .ok_or_else(|| ChainError::Invalid("node JSON has no txid".into()))?
        .to_string();

    let version = num(v, &["version"]).map(|n| (n & 0xff) as u8).unwrap_or(5);

    let vout = v.get("vout").and_then(|x| x.as_array());
    let vin_count = v
        .get("vin")
        .and_then(|x| x.as_array())
        .map(|a| a.len() as u32)
        .unwrap_or(0);

    let value_out: i128 = vout
        .map(|outs| {
            outs.iter()
                .map(|o| zats(o, &["valueZat"], &["value"]).unwrap_or(0))
                .sum()
        })
        .unwrap_or(0);

    // Transparent input values are only known to the node for its own wallet,
    // so we leave them absent unless the node volunteered them.
    let value_in = zats(v, &["valueInZat"], &[]).or(if vin_count == 0 { Some(0) } else { None });

    let transparent = PoolFacts {
        spends: vin_count,
        outputs: vout.map(|o| o.len() as u32).unwrap_or(0),
        value_balance_zat: None,
        value_in_zat: value_in,
        value_out_zat: if value_out > 0 { Some(value_out) } else { None },
        addresses: addresses(vout),
    };

    let sapling = pool(v, &["sapling"], &["vShieldedSpend"], &["vShieldedOutput"]);
    let orchard = pool(v, &["orchard"], &["vOrchardSpend"], &["vOrchardOutput"]);
    let ironwood = pool(v, &["ironwood"], &["vIronwoodSpend"], &["vIronwoodOutput"]);

    // Older nodes report a single Sapling balance at the top level.
    let mut sapling = sapling;
    if sapling.value_balance_zat.is_none() {
        sapling.value_balance_zat = zats(v, &["valueBalanceZat"], &["valueBalance"]);
    }

    let fee_zat = zats(v, &["feeZat"], &["fee"]).map(|f| f as u64);
    let expiry_height = num(v, &["expiryheight", "expiryHeight"]).map(|e| e as u64);

    Ok(TxFacts {
        txid,
        version,
        block_height: block_height.or_else(|| num(v, &["height", "blockheight"]).map(|h| h as u64)),
        block_time: block_time.or_else(|| num(v, &["blocktime", "time"]).map(|t| t as i64)),
        fee_zat,
        transparent,
        sapling,
        orchard,
        ironwood,
        expiry_height,
        lock_time: num(v, &["locktime", "lockTime"]).map(|l| l as u32),
        anchor_age_blocks: None,
        memo_lens: vec![],
        label: None,
        source: Source::Rpc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use leakbench_core::Pool;
    use serde_json::json;

    #[test]
    fn maps_a_turnstile_crossing() {
        let v = json!({
            "txid": "ab".repeat(32),
            "version": 6,
            "locktime": 0,
            "expiryheight": 0,
            "vin": [],
            "vout": [],
            "orchard": { "actions": [1, 2], "valueBalanceZat": 500000000 },
            "ironwood": { "actions": [1], "valueBalanceZat": -500000000 },
            "fee": 0.00005
        });
        let tx = tx_from_node_json(&v, Some(3_500_000), Some(1_790_000_000)).unwrap();
        assert_eq!(tx.version, 6);
        assert_eq!(
            tx.cross_pool_move(),
            Some((Pool::Orchard, Pool::Ironwood, 500_000_000))
        );
        assert_eq!(tx.fee_zat, Some(5_000));
    }

    #[test]
    fn maps_legacy_flat_sapling_fields() {
        let v = json!({
            "txid": "cd".repeat(32),
            "version": 5,
            "vin": [{ "txid": "00" }],
            "vout": [
                { "value": 1.5, "scriptPubKey": { "addresses": ["t1abc"] } },
                { "value": 0.5, "scriptPubKey": { "addresses": ["t1def"] } }
            ],
            "vShieldedSpend": [1],
            "vShieldedOutput": [1, 2],
            "valueBalance": -2.0,
            "fee": 0.0001
        });
        let tx = tx_from_node_json(&v, None, None).unwrap();
        assert_eq!(tx.sapling.spends, 1);
        assert_eq!(tx.sapling.outputs, 2);
        assert_eq!(tx.sapling.value_balance_zat, Some(-200_000_000));
        assert_eq!(tx.transparent.value_out_zat, Some(200_000_000));
        assert_eq!(tx.transparent.addresses.len(), 2);
        assert_eq!(tx.transparent.value_in_zat, None);
    }

    #[test]
    fn node_json_and_hand_written_fixture_agree() {
        let node: Value =
            serde_json::from_str(include_str!("../../../fixtures/rpc/crossing-node.json")).unwrap();
        let decoded = tx_from_node_json(&node, None, None).unwrap();

        let subject: leakbench_core::Subject =
            serde_json::from_str(include_str!("../../../fixtures/example-crossing.json")).unwrap();
        let expected = &subject.txs[0];

        assert_eq!(decoded.txid, expected.txid);
        assert_eq!(decoded.version, expected.version);
        assert_eq!(decoded.block_height, expected.block_height);
        assert_eq!(decoded.block_time, expected.block_time);
        assert_eq!(decoded.fee_zat, expected.fee_zat);
        assert_eq!(decoded.orchard.spends, expected.orchard.spends);
        assert_eq!(decoded.orchard.outputs, expected.orchard.outputs);
        assert_eq!(
            decoded.orchard.value_balance_zat,
            expected.orchard.value_balance_zat
        );
        assert_eq!(
            decoded.ironwood.value_balance_zat,
            expected.ironwood.value_balance_zat
        );
        assert_eq!(decoded.cross_pool_move(), expected.cross_pool_move());
    }

    #[test]
    fn missing_txid_is_an_error() {
        let err = tx_from_node_json(&json!({}), None, None).unwrap_err();
        assert!(matches!(err, ChainError::Invalid(_)));
    }
}
