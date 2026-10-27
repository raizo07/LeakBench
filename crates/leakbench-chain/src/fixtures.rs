//! Loading fact bundles from disk.

use std::path::Path;

use leakbench_core::{Subject, WalletFacts};

use crate::ChainError;

/// Load a [`Subject`] from a JSON file.
///
/// The file is the same shape as the `facts` section of a report, so a report
/// can be fed straight back in for a re-analysis after a fix.
pub fn load_subject(path: impl AsRef<Path>) -> Result<Subject, ChainError> {
    let text = std::fs::read_to_string(path)?;
    let subject: Subject = serde_json::from_str(&text)?;
    Ok(subject)
}

pub fn load_wallet(path: impl AsRef<Path>) -> Result<WalletFacts, ChainError> {
    let text = std::fs::read_to_string(path)?;
    let wallet: WalletFacts = serde_json::from_str(&text)?;
    Ok(wallet)
}

/// Attach wallet-side facts to a subject built from chain data.
pub fn merge_wallet(subject: &mut Subject, wallet: WalletFacts) {
    subject.wallet = Some(wallet);
}

#[cfg(test)]
mod tests {
    use super::*;
    use leakbench_core::SubjectKind;

    #[test]
    fn round_trips_a_subject() {
        let dir = std::env::temp_dir().join("leakbench-fixture-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("subject.json");
        let subject = Subject::new("x", SubjectKind::Transaction, "main");
        std::fs::write(&path, serde_json::to_string_pretty(&subject).unwrap()).unwrap();
        let back = load_subject(&path).unwrap();
        assert_eq!(back.label, "x");
        assert_eq!(back.network, "main");
    }
}
