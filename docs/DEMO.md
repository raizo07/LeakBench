# Demo script (3 minutes)

Run once before you present:

```bash
cargo run -p leakbench-cli -- demo --out reports
cargo run -p leakbench-cli -- serve --dir reports
```

Open <http://127.0.0.1:8787>. The page prefers the live API and falls back to the bundled reports, so
it works even if the server dies mid-demo. Offline screenshots live in `docs/`.

---

## 0:00 — the claim, in one sentence

> "Zcash hides the sender, the recipient and the amount inside a pool. It does not hide the edges.
> LeakBench tells you exactly what your wallet published at those edges — and whether it identified
> you."

Land on the **naive wallet**, which the page opens by default.

## 0:20 — the fatal finding

Point at the hero: `0 / 100`, **Leaks on chain**, `2 fatal`.

Scroll to LB-001 and read the evidence out loud:

- crossing `orchard -> ironwood`
- amount revealed `5 ZEC`
- orchard value balance `5 ZEC`, ironwood `-5 ZEC`
- same-size-band crossings `0 others within ±25%`
- wallet total in source pool `5 ZEC`

> "Nothing here is broken cryptography. `valueBalance` is a consensus field. This wallet moved its
> entire balance across the turnstile one block after being paid, in a round number, in a window
> where almost nobody else was migrating. That is not a metadata leak. That is a press release."

## 0:50 — the second fatal, and the pattern

LB-003: unshielding to an exchange. One line:

> "Everything the shielded pool was protecting is now public, with an address attached."

## 1:05 — the compare tab

Click **Compare** (or `#hardened/compare`).

> "Same starting position. The only difference is how the money crossed. Five non-round chunks,
> busy windows, conventional fees, no transparent legs."

Read the numbers: `0 → 79`, `2 fatal → 0`. Then the two lists:

- removed by hardening: **14 rules**
- still present: **LB-001**

> "This is the part I care about. The crossing still publishes its amount — that is the protocol.
> What changed is that the amount no longer identifies anyone: twenty other crossings sit in the same
> size band. The tool is not pretending the leak is gone. It is measuring the difference between a
> disclosure and a signature."

## 1:40 — the pool map

Click **Pool map** (or `#naive/graph`).

> "Rows are transactions, columns are pools. The line is the value balance — the thing consensus
> publishes. You can see the entire story of this wallet in one picture: two payroll receipts, one
> crossing, one invoice, one unshield, one reused address."

## 2:00 — it is a tool, not a mockup

Switch to the terminal.

```bash
leakbench heuristics                       # 15 rules, one line each
leakbench ci --report reports/naive.json --fail-on high; echo "exit=$?"
leakbench schema | head -20                # versioned report contract
leakbench doctor --rpc http://127.0.0.1:18232   # works or fails loudly
```

> "`ci` exits non-zero on a fatal report, so this drops into a wallet's pipeline as a privacy
> regression test. That is the difference between an audit and a habit."

## 2:30 — the honest close

> "Two things this is not. It is not a deanonymizer: every finding names the observable it came from,
> and the report says heuristics are not proofs. And it does not need your viewing key — nothing
> leaves the machine, the RPC is localhost-only, and the demo runs with no chain at all.
>
> The Zcash ecosystem is asking every user to cross a turnstile right now, and nobody is telling them
> what the crossing costs. This is that number."

---

## If a judge asks

**"Isn't every turnstile crossing a leak? Shouldn't everything be fatal?"**
Every crossing publishes an amount. That is why LB-001 only escalates when the amount is
identifying — round, the full balance, or outside the crowd. If everything were fatal the score
would be noise and the tool would be useless for choosing between designs.

**"Where do the peers come from?"**
A block range, an indexer, or the simulator, supplied as `Subject.peers`. Without a peer sample the
rule lowers confidence and says the anonymity set is unmeasured. The next milestone is chain-wide
sampling; the interface is already there.

**"Why not parse raw transaction bytes?"**
Because a parser that is 99% correct produces confident false findings, and a privacy tool that cries
wolf is worse than no tool. LeakBench maps the node's own decoder output instead, and treats an
unknown field as unknown.

**"How is this different from an explorer's 'privacy score'?"**
Those are UI badges. This is a versioned JSON contract with per-finding evidence, a CI gate, and a
catalogue you can read and argue with.

**"Does the demo need a chain?"**
No. Both scenarios are deterministic facts, which is why the numbers are reproducible. The RPC path
analyzes real transactions when you have a node; `leakbench devnet` prints a verified Zebra Regtest
recipe if you want a local chain.
