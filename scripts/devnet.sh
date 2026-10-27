#!/usr/bin/env bash
# Local-chain helpers. Read `leakbench devnet` first: it prints the same
# recipe with the caveats attached.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
  cat <<'EOF'
usage: scripts/devnet.sh <command>

  config            write zebrad.toml (Regtest, internal miner, RPC on 18232)
  start             run zebrad with that config
  doctor [url]      check that LeakBench can read the node's RPC
  recipe            print the full recipe and the caveats

Notes
  - Building zebrad with the internal miner takes a while:
      cargo install --git https://github.com/ZcashFoundation/zebra --locked zebrad \
        --features internal-miner
  - `zcash-devtool` has no regtest/local-network mode; it is a wallet for public
    light wallet servers. It is the wrong tool for standing up a chain.
EOF
}

case "${1:-usage}" in
  config)
    cargo run -q -p leakbench-cli -- devnet --config > zebrad.toml
    echo "wrote zebrad.toml"
    ;;
  start)
    [ -f zebrad.toml ] || { echo "no zebrad.toml; run: $0 config"; exit 1; }
    command -v zebrad >/dev/null || {
      echo "zebrad not found. build it with:"
      echo "  cargo install --git https://github.com/ZcashFoundation/zebra --locked zebrad --features internal-miner"
      exit 1
    }
    exec zebrad -c zebrad.toml start
    ;;
  doctor)
    cargo run -q -p leakbench-cli -- doctor --rpc "${2:-http://127.0.0.1:18232}"
    ;;
  recipe)
    cargo run -q -p leakbench-cli -- devnet
    ;;
  usage|help|-h|--help)
    usage
    ;;
  *)
    usage
    exit 1
    ;;
esac
