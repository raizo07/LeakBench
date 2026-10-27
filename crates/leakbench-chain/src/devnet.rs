//! Local-network recipes, verified against upstream docs.
//!
//! Two honest facts shape this module:
//!
//! 1. `zcash-devtool` is a *wallet* for public servers (`-s zecrocks`); it has
//!    no local-network or regtest subcommand. It is excellent for making
//!    shielded transactions on testnet, not for standing up a chain.
//! 2. Zebra **does** support Regtest, including an experimental internal miner,
//!    so a local chain is real — see Zebra's `book/src/user/regtest.md`.
//!
//! LeakBench therefore does not need a devnet to run: fixtures and the
//! simulator produce reports with no infrastructure. The recipes below are for
//! when you want your *own* transactions in the analyzer.

pub struct Step {
    pub title: &'static str,
    pub command: Option<&'static str>,
    pub note: &'static str,
}

pub const ZEBRAD_REGTEST_TOML: &str = r#"# zebrad.toml -- local chain for LeakBench experiments.
# Proof-of-work validation is skipped on Regtest; blocks are mined locally.
[mining]
miner_address = 't27eWDgjFYJGVXmzrXeVjnb5J3uXDM9xH9v'
# Requires a Zebra build with `--features internal-miner`.
internal_miner = true

[network]
network = "Regtest"

[state]
ephemeral = true

[rpc]
listen_addr = "127.0.0.1:18232"
"#;

/// How to get a local node answering `getrawtransaction`.
pub fn regtest_recipe() -> Vec<Step> {
    vec![
        Step {
            title: "Build Zebra with the experimental miner",
            command: Some(
                "cargo install --git https://github.com/ZcashFoundation/zebra --locked zebrad \\\n  --features internal-miner",
            ),
            note: "Regtest skips proof-of-work validation, so blocks can be produced locally. \
                   This is a long build; start it early.",
        },
        Step {
            title: "Write the node config",
            command: Some("leakbench devnet config > zebrad.toml"),
            note: "Regtest, ephemeral state, RPC on 127.0.0.1:18232, internal miner on.",
        },
        Step {
            title: "Start the node",
            command: Some("zebrad -c zebrad.toml start"),
            note: "Look for `successfully mined a new block` and net=\"Regtest\" in the logs.",
        },
        Step {
            title: "Confirm LeakBench can see it",
            command: Some("leakbench doctor --rpc http://127.0.0.1:18232"),
            note: "Prints the tip height and whether the RPC shape is usable.",
        },
        Step {
            title: "Create a shielded transaction",
            command: None,
            note: "Regtest needs a wallet that can build shielded transactions against a local \
                   node. `zcash-devtool` targets public servers and cannot do this; use a \
                   wallet library (zcash_client_backend) or submit a pre-built transaction \
                   with `send_raw_transaction`. LeakBench analyzes whatever lands on chain.",
        },
        Step {
            title: "Analyze it",
            command: Some("leakbench scan --rpc http://127.0.0.1:18232 --txid <txid>"),
            note: "Maps the node's decoded JSON onto facts, then runs the catalogue.",
        },
    ]
}

/// The zero-infrastructure path: fixtures and the simulator.
pub fn offline_recipe() -> Vec<Step> {
    vec![
        Step {
            title: "Run the built-in scenarios",
            command: Some("leakbench demo --out reports"),
            note: "Writes naive.json and hardened.json plus an index for the UI.",
        },
        Step {
            title: "Open the visual report",
            command: Some("leakbench serve --dir reports --static web/dist"),
            note: "Serves the SPA at http://127.0.0.1:8787 with the reports as an API.",
        },
        Step {
            title: "Scan any transaction you have JSON for",
            command: Some("leakbench scan --decoded tx.json"),
            note: "`zcash-cli getrawtransaction <txid> 1` output goes straight in.",
        },
    ]
}

pub fn verify_notes() -> Vec<&'static str> {
    vec![
        "Zebra Regtest docs: https://github.com/ZcashFoundation/zebra/blob/main/book/src/user/regtest.md",
        "NU7 activates at mainnet height 3,543,405 on 5 November 2026 (ZIP 259); block spacing becomes 25s (ZIP 218).",
        "Ironwood is NU6.3, a new shielded pool with a turnstile from Orchard (ZIP 258).",
        "Memo bundles (ZIP 231) are Draft and not deployed: memos stay at 512 bytes/output.",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regtest_config_is_parseable_toml_shape() {
        assert!(ZEBRAD_REGTEST_TOML.contains("network = \"Regtest\""));
        assert!(ZEBRAD_REGTEST_TOML.contains("listen_addr"));
        assert!(ZEBRAD_REGTEST_TOML.contains("internal_miner = true"));
    }

    #[test]
    fn recipes_are_populated() {
        assert!(regtest_recipe().len() >= 4);
        assert!(offline_recipe().len() >= 3);
    }
}
