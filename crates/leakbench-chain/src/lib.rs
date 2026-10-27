//! Fact sources for LeakBench.
//!
//! Three ways in, one fact model out:
//!
//! * [`fixtures`] — a JSON [`Subject`] on disk (the interchange format;
//!   hand-writable, diffable, no node required);
//! * [`rpc`] + [`decode`] — ask a local `zebrad`/`zcashd` for a decoded
//!   transaction and map it, using the node's own decoder rather than
//!   re-implementing consensus parsing here;
//! * [`devnet`] — documented local-network recipes for when you want to make
//!   your own shielded transactions instead of reading someone else's.
//!
//! LeakBench deliberately does **not** parse raw transaction bytes itself. A
//! privacy analyser that mis-parses a consensus encoding produces confident
//! nonsense, which is worse than no analyser.

pub mod decode;
pub mod devnet;
pub mod fixtures;
pub mod rpc;

pub use decode::tx_from_node_json;
pub use fixtures::{load_subject, load_wallet, merge_wallet};
pub use rpc::Rpc;

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Rpc(String),
    #[error("{0}")]
    Invalid(String),
}
