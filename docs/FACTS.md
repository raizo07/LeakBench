# Fact format (`leakbench.report.v1` facts)

LeakBench analyzes *observations*, not raw transaction bytes. This is the JSON it consumes. It is
also the `facts` section of every report, which means a report can be fed back in for re-analysis
after a fix.

```bash
leakbench scan --fixture crossing.json
```

## Minimal example

```json
{
  "label": "orchard -> ironwood crossing",
  "kind": "transaction",
  "network": "main",
  "description": "the migration transaction from block 3,500,041",
  "context": { "block_interval_secs": 75, "tip_height": 3500100 },
  "txs": [
    {
      "txid": "30cc9acb8b1b2c500ef5754112486810bc7e74bb7e94d2bd6b64019bb22d1d59",
      "version": 6,
      "block_height": 3500041,
      "block_time": 1790556600,
      "fee_zat": 10000,
      "transparent": { "spends": 0, "outputs": 0 },
      "sapling": {},
      "orchard":  { "spends": 2, "outputs": 2, "value_balance_zat": 500000000 },
      "ironwood": { "spends": 1, "outputs": 1, "value_balance_zat": -500000000 },
      "anchor_age_blocks": 130,
      "source": "manual"
    }
  ],
  "peers": []
}
```

That produces two findings: LB-001 (critical, fatal — a whole-ZEC crossing equal to the wallet's
balance is identifying) and LB-006 (high — a 130-block-stale anchor).

## `Subject`

| field | required | meaning |
|-------|----------|---------|
| `label` | yes | what to call this subject in the report |
| `kind` | yes | `transaction`, `wallet` or `scenario` |
| `network` | no | `main` / `test` / `regtest`; defaults to `main` |
| `description` | no | one line of context, shown in the UI |
| `context.block_interval_secs` | no | 75 before NU7, 25 after; defaults to 25. Used to convert block gaps into wall clock |
| `context.tip_height` | no | chain tip at observation time |
| `txs` | yes | the transactions |
| `wallet` | no | wallet-side facts (enables LB-008/012/013/014/015) |
| `peers` | no | other users' activity, for anonymity-set rules |

## `TxFacts`

| field | meaning |
|-------|---------|
| `txid` | transaction id |
| `version` | 4, 5 or 6 |
| `block_height`, `block_time` | where it landed; `block_time` is Unix seconds |
| `fee_zat` | fee in zatoshis, when known |
| `transparent` | `spends` = `vin` count, `outputs` = `vout` count, plus `value_in_zat` / `value_out_zat` |
| `sapling`, `orchard`, `ironwood` | `spends`, `outputs`, `value_balance_zat` |
| `expiry_height` | encoded `nExpiryHeight`; 0 or absent means unset |
| `anchor_age_blocks` | blocks between the spend anchor and the tip (wallet-side) |
| `memo_lens` | memo byte lengths for outputs this wallet can decrypt (wallet-side) |
| `source` | `fixture`, `rpc`, `simulated`, `manual` — shown in every report |

### Sign convention for `value_balance_zat`

It is the pool's net outflow: **inputs − outputs**.

- `+X` on a pool means `X` zatoshis left that pool.
- `−X` on a pool means `X` zatoshis entered that pool.
- A same-pool shielded transfer therefore has `value_balance_zat == fee`, which reveals nothing
  about the amount — this is why LB-001 only fires when two shielded pools move in opposite
  directions.
- The transparent bundle has no value balance; use `value_in_zat` / `value_out_zat`, which the
  analyzer reads separately.

If a field is unknown, omit it or set it to `null`. Do not guess: `null` lowers confidence, a wrong
number produces a confident false finding.

## `wallet`

```json
"wallet": {
  "label": "my wallet",
  "network": "main",
  "birthday_height": 3480000,
  "notes": [
    { "txid": "…", "pool": "orchard", "value_zat": 340000000,
      "received_height": 3500000, "memo_len": 512, "is_change": false }
  ],
  "addresses": [
    { "address": "t1…", "pool": "transparent", "uses": 2,
      "first_height": 3500043, "last_height": 3500044 }
  ]
}
```

The fastest way to build this file is CSV, which any wallet debug screen can produce:

```bash
leakbench wallet import --notes notes.csv --addresses addresses.csv --out wallet.json
```

```
notes.csv      txid,pool,value_zat,received_height[,memo_len,is_change]
addresses.csv  address,uses,first_height,last_height
```

Wallet facts are the only place LeakBench learns something an observer could not: which notes are
yours, which addresses you reused, and what your memos look like. Keep the file local. Reports
contain txids and amounts, never keys.

## `peers`

```json
"peers": [
  { "txid": "peer-1", "height": 3500038, "shape": "t0/0 s0/0 o1/0 i0/1",
    "amount_zat": 123450000, "kind": "migration" }
]
```

Peers are what turn "an amount is public" into "an amount is identifiable". The shipped way to get
them is a block range on your own node:

```bash
leakbench peers --rpc http://127.0.0.1:18232 --from 3500000 --to 3500100 --out peers.json
leakbench scan --fixture crossing.json --peers peers.json
```

`peers.json` is `{schema, network, from_height, to_height, block_interval_secs, peers[]}`, and
`scan --peers` also accepts a bare array. You can hand-write it, or feed it from an indexer. Without them, LB-002 and LB-013 report lower confidence and
LB-001 treats every crossing as identifying — conservative on purpose.

## Producing facts from a node

```bash
zcash-cli getrawtransaction <txid> 1 > tx.json
leakbench scan --decoded tx.json
```

The decoder accepts both `*Zat` integer fields and ZEC-decimal fields, nested or flat shielded
sections, and addresses under `scriptPubKey.addresses` or hoisted to the output. Unknown shapes
degrade to absent fields rather than wrong numbers.
