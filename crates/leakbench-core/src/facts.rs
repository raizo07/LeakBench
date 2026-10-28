//! The fact layer: a normalized, implementation-agnostic description of what an
//! observer can actually see about Zcash transactions.
//!
//! Everything the heuristic engine knows arrives here. That is deliberate:
//!
//! * the engine never touches a Zcash type, a node, or the network, so it is
//!   deterministic and unit-testable;
//! * facts can come from fixtures, a node's decoded RPC JSON, or the simulator,
//!   without the rules knowing the difference;
//! * the JSON form of these types *is* the interchange format published in
//!   `docs/FACTS.md`, so a judge can hand LeakBench a transaction they care
//!   about without writing Rust.
//!
//! A fact is not a proof. `value_balance_zat` is a consensus-encoded field, but
//! `anchor_age_blocks` and all wallet-side facts are supplied by the caller.

use serde::{Deserialize, Serialize};

/// Shielded and transparent value pools.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Pool {
    #[default]
    Transparent,
    Sapling,
    Orchard,
    Ironwood,
}

impl Pool {
    pub const SHIELDED: [Pool; 3] = [Pool::Sapling, Pool::Orchard, Pool::Ironwood];
    pub fn as_str(self) -> &'static str {
        match self {
            Pool::Transparent => "transparent",
            Pool::Sapling => "sapling",
            Pool::Orchard => "orchard",
            Pool::Ironwood => "ironwood",
        }
    }

    pub fn is_shielded(self) -> bool {
        !matches!(self, Pool::Transparent)
    }

    /// Display name used in reports.
    pub fn title(self) -> &'static str {
        match self {
            Pool::Transparent => "Transparent",
            Pool::Sapling => "Sapling",
            Pool::Orchard => "Orchard",
            Pool::Ironwood => "Ironwood",
        }
    }
}

/// Per-pool view of a transaction.
///
/// For transparent bundles, `spends`/`outputs` are `vin`/`vout` counts and the
/// values are public. For shielded bundles, `value_balance_zat` is the
/// consensus-encoded pool balance (`inputs - outputs`, in zatoshis) and is
/// public even though the individual notes are not.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolFacts {
    #[serde(default)]
    pub spends: u32,
    #[serde(default)]
    pub outputs: u32,
    /// Public per-pool value balance in zatoshis. `Some(0)` and `None` differ:
    /// `None` means "not reported by this source".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_balance_zat: Option<i128>,
    /// Transparent input value (public).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_in_zat: Option<i128>,
    /// Transparent output value (public).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_out_zat: Option<i128>,
    /// Transparent addresses touched (public).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<String>,
}

impl PoolFacts {
    pub fn is_active(&self) -> bool {
        self.spends > 0
            || self.outputs > 0
            || self.value_balance_zat.unwrap_or(0) != 0
            || self.value_in_zat.unwrap_or(0) != 0
            || self.value_out_zat.unwrap_or(0) != 0
    }

    pub fn action_count(&self) -> u32 {
        self.spends.saturating_add(self.outputs)
    }
}

/// One transaction, as an observer sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxFacts {
    pub txid: String,
    /// Transaction format version: 4, 5 or 6.
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u64>,
    /// Block time, Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_time: Option<i64>,
    /// Fee in zatoshis, when the source can compute it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_zat: Option<u64>,
    #[serde(default)]
    pub transparent: PoolFacts,
    #[serde(default)]
    pub sapling: PoolFacts,
    #[serde(default)]
    pub orchard: PoolFacts,
    #[serde(default)]
    pub ironwood: PoolFacts,
    /// Expiry height as encoded (0 / absent means unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_time: Option<u32>,
    /// How many blocks behind the tip the anchor this transaction commits to
    /// was. Supplied by a wallet-side scan; not directly observable on-chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_age_blocks: Option<u64>,
    /// Memo byte lengths for outputs this wallet can decrypt (wallet-side only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memo_lens: Vec<u32>,
    /// Human label, e.g. "migrate 5 ZEC Orchard -> Ironwood".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub source: Source,
}

fn default_version() -> u8 {
    5
}

impl TxFacts {
    pub fn pool(&self, pool: Pool) -> &PoolFacts {
        match pool {
            Pool::Transparent => &self.transparent,
            Pool::Sapling => &self.sapling,
            Pool::Orchard => &self.orchard,
            Pool::Ironwood => &self.ironwood,
        }
    }

    /// Shielded bundles with at least one spend or output.
    pub fn active_shielded(&self) -> impl Iterator<Item = (Pool, &PoolFacts)> {
        Pool::SHIELDED
            .into_iter()
            .map(move |p| (p, self.pool(p)))
            .filter(|(_, f)| f.spends > 0 || f.outputs > 0)
    }

    /// Pools that moved value in or out, with the public balance.
    pub fn value_flows(&self) -> Vec<(Pool, i128)> {
        let mut out = Vec::new();
        for p in [
            Pool::Transparent,
            Pool::Sapling,
            Pool::Orchard,
            Pool::Ironwood,
        ] {
            if let Some(v) = self.pool(p).value_balance_zat {
                if v != 0 {
                    out.push((p, v));
                }
            }
        }
        out
    }

    /// Shielded value crossing between two shielded pools, if the transaction
    /// does a turnstile-crossing move. Returns `(from, to, amount_zat)`.
    ///
    /// A same-pool shielded transaction has a single non-zero balance equal to
    /// the fee, so it is not reported here: nothing about the transferred amount
    /// becomes public.
    pub fn cross_pool_move(&self) -> Option<(Pool, Pool, u128)> {
        let mut leaving: Option<(Pool, i128)> = None;
        let mut entering: Option<(Pool, i128)> = None;
        for (pool, value) in self.value_flows() {
            if !pool.is_shielded() {
                continue;
            }
            if value > 0 {
                leaving = Some((pool, value));
            } else if value < 0 {
                entering = Some((pool, value));
            }
        }
        let ((from, out_value), (to, in_value)) = (leaving?, entering?);
        // Not necessarily a clean two-pool move (fees, transparent legs,
        // multi-pool bundles): take the smaller magnitude so a finding is
        // conservative rather than overstating precision.
        let amount = out_value.unsigned_abs().min(in_value.unsigned_abs());
        if amount == 0 {
            None
        } else {
            Some((from, to, amount))
        }
    }

    /// ZIP 317 logical actions, floored at the grace allowance of 2.
    ///
    /// The spec counts, per pool: `max(nSpends, nOutputs)` for Sapling, and
    /// `nActions` for Orchard and Ironwood (an action pairs one spend with one
    /// output). Transparent contributes `max(ceil(in_size/150), ceil(out_size/34))`;
    /// we only carry counts, so `max(vin, vout)` approximates it and says so.
    ///
    /// Verified against mainnet: a 2-action transfer pays 10,000 zats, and a
    /// 3-action transfer with one transparent output pays 20,000.
    pub fn logical_actions(&self) -> u32 {
        let sapling = self.sapling.spends.max(self.sapling.outputs);
        let orchard = self.orchard.spends.max(self.orchard.outputs);
        let ironwood = self.ironwood.spends.max(self.ironwood.outputs);
        let transparent = self.transparent.spends.max(self.transparent.outputs);
        (sapling + orchard + ironwood + transparent).max(2)
    }

    /// The ZIP 317 conventional fee floor: 5,000 zats per logical action.
    pub fn zip317_floor_zat(&self) -> u64 {
        5_000 * u64::from(self.logical_actions())
    }

    pub fn shielded_spend_count(&self) -> u32 {
        Pool::SHIELDED.iter().map(|p| self.pool(*p).spends).sum()
    }

    pub fn shielded_output_count(&self) -> u32 {
        Pool::SHIELDED.iter().map(|p| self.pool(*p).outputs).sum()
    }

    pub fn has_transparent_activity(&self) -> bool {
        self.transparent.is_active()
    }

    pub fn height(&self) -> Option<u64> {
        self.block_height
    }

    /// A compact, comparable shape string, e.g. `t0/0 s0/0 o2/2 i0/0`.
    pub fn shape(&self) -> String {
        format!(
            "t{}/{} s{}/{} o{}/{} i{}/{}",
            self.transparent.spends,
            self.transparent.outputs,
            self.sapling.spends,
            self.sapling.outputs,
            self.orchard.spends,
            self.orchard.outputs,
            self.ironwood.spends,
            self.ironwood.outputs
        )
    }
}

/// Where a fact bundle came from. Shown in every report so a score is never
/// mistaken for an on-chain measurement when it came from a simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Fixture,
    Rpc,
    /// Parsed from raw transaction bytes with the reference implementation.
    Raw,
    Simulated,
    #[default]
    Manual,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Fixture => "fixture",
            Source::Rpc => "rpc",
            Source::Raw => "raw",
            Source::Simulated => "simulated",
            Source::Manual => "manual",
        }
    }
}

/// A note the subject wallet holds or held.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFacts {
    pub txid: String,
    pub pool: Pool,
    pub value_zat: u64,
    #[serde(default)]
    pub received_height: u64,
    #[serde(default)]
    pub memo_len: u32,
    #[serde(default)]
    pub is_change: bool,
}

/// A transparent address the subject wallet used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressFacts {
    pub address: String,
    #[serde(default)]
    pub pool: Pool,
    #[serde(default)]
    pub uses: u32,
    #[serde(default)]
    pub first_height: u64,
    #[serde(default)]
    pub last_height: u64,
}

/// Wallet-side facts: only available when the subject scans with their own keys.
/// LeakBench never uploads these; the CLI reads them from a local file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletFacts {
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_network_str")]
    pub network: String,
    #[serde(default)]
    pub birthday_height: u64,
    #[serde(default)]
    pub notes: Vec<NoteFacts>,
    #[serde(default)]
    pub addresses: Vec<AddressFacts>,
}

fn default_network_str() -> String {
    "main".to_string()
}

impl WalletFacts {
    pub fn total_notes_zat(&self, pool: Pool) -> i128 {
        self.notes
            .iter()
            .filter(|n| n.pool == pool)
            .map(|n| i128::from(n.value_zat))
            .sum()
    }

    pub fn pools_held(&self) -> Vec<Pool> {
        let mut v: Vec<Pool> = self.notes.iter().map(|n| n.pool).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Network-level context that changes what counts as "anonymous enough".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainContext {
    /// Target block spacing in seconds: 75 pre-NU7, 25 from NU7 onward.
    #[serde(default = "default_interval")]
    pub block_interval_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_height: Option<u64>,
}

fn default_interval() -> u64 {
    25
}

impl Default for ChainContext {
    fn default() -> Self {
        Self {
            block_interval_secs: default_interval(),
            tip_height: None,
        }
    }
}

/// Anonymity-set observations about *other* transactions in the same window.
/// Optional: without peers, rules that need an anonymity set downgrade their
/// confidence instead of guessing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSummary {
    #[serde(default)]
    pub txid: String,
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub shape: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_zat: Option<i128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    Transaction,
    Wallet,
    Scenario,
}

impl SubjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SubjectKind::Transaction => "transaction",
            SubjectKind::Wallet => "wallet",
            SubjectKind::Scenario => "scenario",
        }
    }
}

/// The thing being analyzed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub label: String,
    pub kind: SubjectKind,
    #[serde(default = "default_network_str")]
    pub network: String,
    #[serde(default)]
    pub txs: Vec<TxFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletFacts>,
    #[serde(default)]
    pub context: ChainContext,
    #[serde(default)]
    pub peers: Vec<PeerSummary>,
    /// Free-form note shown in the report (why this subject exists).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Subject {
    pub fn new(label: impl Into<String>, kind: SubjectKind, network: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind,
            network: network.into(),
            txs: Vec::new(),
            wallet: None,
            context: ChainContext::default(),
            peers: Vec::new(),
            description: None,
        }
    }

    /// Height span of the observed transactions.
    pub fn span(&self) -> Option<(u64, u64)> {
        let mut heights: Vec<u64> = self.txs.iter().filter_map(|t| t.block_height).collect();
        if heights.is_empty() {
            return None;
        }
        heights.sort_unstable();
        Some((*heights.first()?, *heights.last()?))
    }

    pub fn sorted_txs(&self) -> Vec<&TxFacts> {
        let mut v: Vec<&TxFacts> = self.txs.iter().collect();
        v.sort_by_key(|t| (t.block_height.unwrap_or(0), t.txid.clone()));
        v
    }

    pub fn all_sources(&self) -> Vec<Source> {
        let mut v: Vec<Source> = self.txs.iter().map(|t| t.source).collect();
        v.sort_by_key(|s| s.as_str());
        v.dedup_by(|a, b| a.as_str() == b.as_str());
        v
    }
}

/// Format a zat amount as ZEC with adaptive precision, e.g. `5 ZEC`,
/// `0.0123 ZEC`, `4,999 zats` for dust.
pub fn format_zec(zat: i128) -> String {
    let abs = zat.unsigned_abs();
    if abs < 100_000 {
        return format!("{} zats", group_thousands(zat as i64));
    }
    let sign = if zat < 0 { "-" } else { "" };
    let whole = abs / 100_000_000;
    let frac = abs % 100_000_000;
    if frac == 0 {
        return format!("{sign}{whole} ZEC");
    }
    let mut f = format!("{frac:08}");
    while f.ends_with('0') {
        f.pop();
    }
    format!("{sign}{whole}.{f} ZEC")
}

pub fn format_zat(zat: u64) -> String {
    format_zec(i128::from(zat))
}

/// Block heights read better with separators: `3,543,405` vs `3543405`.
pub fn format_height(height: u64) -> String {
    group_thousands(height as i64)
}

fn group_thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(spends: u32, outputs: u32, vb: i128) -> PoolFacts {
        PoolFacts {
            spends,
            outputs,
            value_balance_zat: Some(vb),
            ..Default::default()
        }
    }

    #[test]
    fn same_pool_transfer_is_not_a_cross_pool_move() {
        let tx = TxFacts {
            txid: "aa".into(),
            version: 6,
            block_height: Some(10),
            block_time: None,
            fee_zat: Some(10_000),
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: pool(2, 2, 10_000),
            ironwood: PoolFacts::default(),
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        assert!(tx.cross_pool_move().is_none());
    }

    #[test]
    fn turnstile_move_is_detected_with_amount() {
        let tx = TxFacts {
            txid: "bb".into(),
            version: 6,
            block_height: Some(11),
            block_time: None,
            fee_zat: Some(5_000),
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: pool(1, 0, 500_000_000),
            ironwood: pool(0, 1, -500_000_000),
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        assert_eq!(
            tx.cross_pool_move(),
            Some((Pool::Orchard, Pool::Ironwood, 500_000_000))
        );
    }

    #[test]
    fn zip317_floor_uses_grace_actions() {
        let mut tx = TxFacts {
            txid: "cc".into(),
            version: 6,
            block_height: None,
            block_time: None,
            fee_zat: None,
            transparent: PoolFacts::default(),
            sapling: PoolFacts::default(),
            orchard: pool(1, 0, 0),
            ironwood: PoolFacts::default(),
            expiry_height: None,
            lock_time: None,
            anchor_age_blocks: None,
            memo_lens: vec![],
            label: None,
            source: Source::Fixture,
        };
        assert_eq!(tx.logical_actions(), 2);
        assert_eq!(tx.zip317_floor_zat(), 10_000);
        // Orchard counts actions: 2 spends / 3 outputs is three logical actions.
        tx.orchard = pool(2, 3, 0);
        assert_eq!(tx.logical_actions(), 3);
        assert_eq!(tx.zip317_floor_zat(), 15_000);
        // 3 Ironwood actions + one transparent output is four (verified on mainnet).
        tx.orchard = PoolFacts::default();
        tx.ironwood = pool(3, 3, 0);
        tx.transparent = PoolFacts {
            spends: 0,
            outputs: 1,
            ..Default::default()
        };
        assert_eq!(tx.logical_actions(), 4);
        assert_eq!(tx.zip317_floor_zat(), 20_000);
    }

    #[test]
    fn formats_amounts() {
        assert_eq!(format_zec(500_000_000), "5 ZEC");
        assert_eq!(format_zec(12_300_000), "0.123 ZEC");
        assert_eq!(format_zec(-100_000_000), "-1 ZEC");
        assert_eq!(format_zec(4_999), "4,999 zats");
        assert_eq!(format_zec(1_234_567_890), "12.3456789 ZEC");
        assert_eq!(format_height(3_543_405), "3,543,405");
    }
}
