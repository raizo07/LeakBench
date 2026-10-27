//! LeakBench core: facts in, findings out. No I/O, no node, no network.
//!
//! ```no_run
//! use leakbench_core::{analyze, Subject, SubjectKind};
//! let subject = Subject::new("my wallet", SubjectKind::Wallet, "main");
//! let report = analyze(&subject);
//! assert_eq!(report.schema, "leakbench.report.v1");
//! ```

pub mod engine;
pub mod facts;
pub mod heuristics;
pub mod model;
pub mod score;
pub mod time;

pub use engine::{analyze, describe, VERSION};
pub use facts::{
    format_height, format_zat, format_zec, AddressFacts, ChainContext, NoteFacts, PeerSummary,
    Pool, PoolFacts, Source, Subject, SubjectKind, TxFacts, WalletFacts,
};
pub use heuristics::{catalogue, RULES};
pub use model::{
    Category, Confidence, Evidence, Finding, Report, Score, Severity, Verdict, DISCLAIMER, SCHEMA,
};
pub use score::score_findings;
