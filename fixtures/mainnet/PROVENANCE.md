# Mainnet fixtures

These are real Zcash mainnet transactions. The `.hex` files are the raw serialized transactions;
the `.json` files are fact bundles derived from those bytes by LeakBench with
`zcash_primitives` — **not** copied from an explorer's decoded output.

Captured **2026-10-28** from Blockchair's public Zcash API, which serves the raw transaction
(`/zcash/raw/transaction/<txid>`). The explorer does not decode v6/Ironwood bundles per pool, which
is precisely why the analyzer parses the bytes itself.

| fixture | txid | block | what it shows |
|---|---|---|---|
| `crossing-986cfd40` | `986cfd4042614e841fc21187fce3ba254a24c24aa8fdec63fb5b12133b81ec2a` | 3,533,241 | Orchard → Ironwood turnstile crossing: +63,400 zats leaving Orchard, 43,400 arriving in Ironwood, 20,000 fee |
| `unshield-ddd1dbe5` | `ddd1dbe52211888a268bb539062bf14e6280320669bbbd4d4c59a9234eb4c84f` | 3,533,276 | Ironwood → transparent unshielding: 132,000 zats to a transparent output, 20,000 fee |
| `round-unshield-776edbb3` | `776edbb3e57b09de6ae9695346b9256f6949ed74b6fc60f5ff180e8faaf1ee71` | 3,533,278 | 8.00000000 ZEC unshielded from a 3-action Ironwood bundle: a round, public amount |

## Reproduce a capture

```bash
curl -s "https://api.blockchair.com/zcash/raw/transaction/<txid>" \
  | jq -r '.data[].raw_transaction' > fixtures/mainnet/<name>.hex

leakbench scan --raw fixtures/mainnet/<name>.hex --height <block>
```

## What the tests assert

`crates/leakbench-chain/tests/mainnet.rs`:

1. the reference parser turns the hex into exactly the committed facts;
2. the parsed numbers close the consensus value equation
   `Σ shielded value balances − transparent outputs == fee`;
3. the analyzer produces the expected findings and verdict on real data.

The second check is the interesting one: it is an independent arithmetic constraint from the
protocol, so it would fail if the parser or the fact model drifted.

## Rate limits

Blockchair's free tier allows roughly ten requests per minute. Capturing a new fixture costs two
requests; a batch of twenty needs a few minutes of waiting.
