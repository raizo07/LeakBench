//! Import wallet-side facts from CSV.
//!
//! The wallet rules (LB-008, LB-012, LB-013, LB-014, LB-015) need to know which
//! notes and addresses are yours. Wallets do not agree on an export format, so
//! LeakBench defines the smallest thing a human can produce from any debug
//! screen: two CSVs.
//!
//! `notes.csv`     txid,pool,value_zat,received_height[,memo_len,is_change]
//! `addresses.csv` address,uses,first_height,last_height

use anyhow::{bail, Context, Result};
use leakbench_core::{AddressFacts, NoteFacts, Pool, WalletFacts};

pub fn parse_pool(text: &str) -> Result<Pool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "transparent" | "t" => Ok(Pool::Transparent),
        "sapling" | "s" => Ok(Pool::Sapling),
        "orchard" | "o" => Ok(Pool::Orchard),
        "ironwood" | "i" => Ok(Pool::Ironwood),
        other => bail!("unknown pool {other:?} (transparent|sapling|orchard|ironwood)"),
    }
}

fn rows(text: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    text.lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(n, line)| (n, line.split(',').map(str::trim).collect::<Vec<_>>()))
        .filter(|(_, cols)| {
            // A header row can follow comments, so match on content, not line number.
            !matches!(
                cols.first().map(|c| c.to_ascii_lowercase()).as_deref(),
                Some("txid") | Some("address")
            )
        })
}

pub fn parse_notes(text: &str) -> Result<Vec<NoteFacts>> {
    let mut notes = Vec::new();
    for (line, cols) in rows(text) {
        if cols.len() < 4 {
            bail!("line {line}: expected txid,pool,value_zat,received_height[,memo_len,is_change]");
        }
        notes.push(NoteFacts {
            txid: cols[0].to_string(),
            pool: parse_pool(cols[1]).with_context(|| format!("line {line}"))?,
            value_zat: cols[2]
                .parse()
                .with_context(|| format!("line {line}: value_zat must be an integer"))?,
            received_height: cols[3]
                .parse()
                .with_context(|| format!("line {line}: received_height must be an integer"))?,
            memo_len: cols.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
            is_change: cols
                .get(5)
                .map(|s| matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
        });
    }
    Ok(notes)
}

pub fn parse_addresses(text: &str) -> Result<Vec<AddressFacts>> {
    let mut out = Vec::new();
    for (line, cols) in rows(text) {
        if cols.len() < 4 {
            bail!("line {line}: expected address,uses,first_height,last_height");
        }
        out.push(AddressFacts {
            address: cols[0].to_string(),
            pool: Pool::Transparent,
            uses: cols[1].parse().with_context(|| format!("line {line}"))?,
            first_height: cols[2].parse().with_context(|| format!("line {line}"))?,
            last_height: cols[3].parse().with_context(|| format!("line {line}"))?,
        });
    }
    Ok(out)
}

pub fn build(
    label: &str,
    network: &str,
    birthday_height: u64,
    notes: Vec<NoteFacts>,
    addresses: Vec<AddressFacts>,
) -> WalletFacts {
    WalletFacts {
        label: label.to_string(),
        network: network.to_string(),
        birthday_height,
        notes,
        addresses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_notes_with_header_comments_and_defaults() {
        let csv = "\
# exported from wallet debug screen
txid,pool,value_zat,received_height,memo_len,is_change
aa11,orchard,340000000,3500000,512,false
bb22,ironwood,150000000,3500042,0,true
";
        let notes = parse_notes(csv).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].pool, Pool::Orchard);
        assert_eq!(notes[0].memo_len, 512);
        assert!(!notes[0].is_change);
        assert!(notes[1].is_change);
        assert_eq!(notes[1].value_zat, 150_000_000);
    }

    #[test]
    fn short_rows_default_memo_and_change() {
        let notes = parse_notes("cc33,sapling,20000000,3400000").unwrap();
        assert_eq!(notes[0].memo_len, 0);
        assert!(!notes[0].is_change);
    }

    #[test]
    fn rejects_bad_input_with_a_line_number() {
        let err = parse_notes("dd44,dogecoin,1,2").unwrap_err().to_string();
        assert!(err.contains("line 1"), "{err}");
        let err = parse_notes("dd44,orchard,not-a-number,2")
            .unwrap_err()
            .to_string();
        assert!(err.contains("value_zat"), "{err}");
    }

    #[test]
    fn addresses_are_parsed() {
        let addrs = parse_addresses("t1abc,3,3500043,3500099").unwrap();
        assert_eq!(addrs[0].uses, 3);
        assert_eq!(addrs[0].last_height, 3_500_099);
    }
}
