# Architecture

## Shape of the thing

LeakBench is a pipeline with one hard boundary in the middle:

```
                 ┌──────────────────────────────────────────────┐
   chain data →  │  fact sources                                │
                 │    fixtures/*.json     (hand-written)        │
                 │    node RPC + decode   (zebrad / zcashd)     │
                 │    leakbench-sim       (seeded scenarios)    │
                 └───────────────────┬──────────────────────────┘
                                     │  Subject { TxFacts, WalletFacts, peers, context }
                                     ▼
                 ┌──────────────────────────────────────────────┐
                 │  leakbench-core                              │
                 │    facts   ── normalized observation set     │
                 │    heuristics ── 15 pure functions           │
                 │    score   ── weighted risk → 0..100         │
                 │    engine  ── Report                         │
                 └───────────────────┬──────────────────────────┘
                                     │  Report (JSON, versioned schema)
                    ┌────────────────┴─────────────────┐
                    ▼                                  ▼
             leakbench-cli                      web/ (React)
             render · serve · ci                findings · timeline · pool map · compare
```

The boundary is the point. Everything to the left of the line knows about Zcash; everything to the
right of it knows about *observations*. The rules cannot accidentally encode a parsing assumption,
and the simulator cannot accidentally depend on a node.

## Crates

| crate | responsibility | dependencies |
|-------|----------------|--------------|
| `leakbench-core` | facts, model, heuristics, scoring, report assembly | `serde` only — no I/O, no chain crates, no clock except `SystemTime` |
| `leakbench-sim` | deterministic scenarios (`naive`, `hardened`) and peers | `leakbench-core` |
| `leakbench-chain` | fixture loading, node-JSON decoder, localhost JSON-RPC, block-range peer sampling, devnet recipes | `leakbench-core`, `thiserror` |
| `leakbench-cli` | `scan`, `simulate`, `demo`, `serve`, `ci`, `schema`, `doctor`, `devnet` | everything, plus `clap` and `tiny_http` |

Why this split rather than one crate: the engine is the part that must be reviewable, and a
reviewer should be able to read `leakbench-core` in one sitting without a node, a database or a
Tokio runtime in the room. It also means a rule can be tested with six lines of Rust instead of a
fixture harness.

`leakbench-chain` deliberately has **no HTTP client dependency**. It speaks HTTP/1.1 to `127.0.0.1`
by hand: no TLS stack, no `reqwest`, no certificate store, and a much smaller supply chain for a
tool that asks for your viewing keys' outputs. Remote nodes are a non-goal; use an SSH tunnel.

## Why facts instead of raw transaction bytes

A privacy analyzer's worst failure is not "no findings", it is *confident wrong findings*. Zcash
v5/v6 transaction encodings are long, versioned and unforgiving; a hand-rolled parser that is 99%
right will misplace a value balance in the remaining 1% and report a leak that does not exist.

So LeakBench does not parse raw bytes. It maps the **node's own decoded JSON** onto `TxFacts`, with
aliases for the field-name differences between `zcashd`, `zebrad` and tooling, and it degrades
rather than guesses: an unknown field becomes `None`, and `None` lowers confidence.

`TxFacts` is also the interchange format. You can write one by hand (`docs/FACTS.md`), generate it
from your own indexer, or hand the tool a report's `facts` section — reports are re-analyzable.

## The fact that makes this tool necessary

In Zcash, each shielded bundle carries a public `valueBalance` — the net value entering or leaving
that pool. For a normal same-pool transfer it equals the fee, so nothing about the amount is
revealed. But:

- **Orchard → Ironwood** (the turnstile migration) puts `+X` in the Orchard balance and `−X` in the
  Ironwood balance. `X` is the migrated amount, public to anyone with a block explorer.
- **Shielded → transparent** puts `+X` in the shielded balance and pays `X` out. Public.
- **Transparent → shielded** puts `−X` in the shielded balance. Public.

So the disclosure is structural, not a wallet bug. What a wallet *can* control is whether the
disclosed number identifies anyone. That distinction drives LB-001:

```
identifying  =  round amount
             ∨  amount == wallet's entire balance in the source pool
             ∨  fewer than 3 peer crossings in ±25% size band
             ∨  no peer sample at all (cannot claim otherwise)
```

`identifying` → critical + fatal (60 risk points). Otherwise → medium, no fatality, and the summary
says why: *"20 other crossings sit within ±25% of it, so the number does not identify you on its
own: it is a disclosure, not a signature."*

This is the difference between a linter that flags everything and one you can act on.

## Scoring

```
risk   = Σ_findings  base(severity or fatal) × confidence × damping(occurrence)
score  = clamp(100 − risk, 0, 100) → rounded

base:  fatal 60 · critical 34 · high 20 · medium 8 · low 3 · info 1
conf:  high 1.0 · medium 0.7 · low 0.4
damp:  1 / (1 + 0.7 × (n−1))      within the same rule id
```

Why these choices:

- **Fatal dominates.** The brief says leaks are disqualifying, not deductions. One fatal finding
  lands at 40/100 and the verdict is `disqualified` regardless of everything else.
- **Fatal is not confidence-scaled.** A fatal disclosure costs the same whether we are certain or
  not, because the policy is "leaks are disqualifying", not "leaks are disqualifying with
  probability p". Scaling it by confidence had a perverse effect: supplying better evidence raised
  the risk and *lowered* the score. Non-fatal findings still scale by confidence, where it means
  what it should.
- **Damping, not multiplication.** A wallet with forty round-number payments is worse than one with
  two, but not twenty times worse. Undamped sums saturate at zero and hide the shape of the problem.
- **Linear and inspectable.** Any judge can recompute the score from the findings table in ten
  seconds, which is the point. `score.risk_points` exposes the unclamped sum.

Verdict bands: `disqualified` (any fatal) · `at-risk` (< 60) · `acceptable` (< 85) · `clean` (≥ 85).

## Report contract

`leakbench.report.v1` — the schema lives in `crates/leakbench-cli/src/schema.rs` and is printed by
`leakbench schema`. The web UI and the CI gate both consume it, and the `facts` section is a valid
input to `leakbench scan --fixture`. Versioning rule: additive fields only within v1; a breaking
change bumps the string and ships a migration note.

## Extending with a rule

1. Add a function `fn check(subject: &Subject) -> Vec<Finding>` in `crates/leakbench-core/src/heuristics.rs`.
2. Register it in `RULES` with an id, title and one-line description.
3. Write a positive test (a crafted `TxFacts` that trips it) and a negative test (the same facts with
   the observable removed, asserting silence).
4. Decide its `Category` and whether it can ever be `fatal`. If you cannot point at the observable
   in one sentence, it is not ready.
5. Update `docs/HEURISTICS.md` with false-positive behaviour and a remediation.

Rules that need an anonymity set take `subject.peers`. If `peers` is empty, lower confidence and say
so — never infer an anonymity set you were not given.

## Diffing, gates and reports

Three outputs share one model:

- `leakbench ci --fail-on high` turns a report into an exit code;
- `leakbench diff before.json after.json` compares structurally — by rule id, count, severity and
  confidence — so a reordered report is not a change, and it detects regressions using **unclamped
  risk** (`score.risk_points`), because the 0–100 score saturates and would hide new findings on an
  already-disqualified wallet;
- `leakbench sarif` emits SARIF 2.1.0 with `partialFingerprints`, so a finding keeps its identity
  across runs and shows up in code scanning.

## Testing

- Unit tests per crate, including the score model's edge cases and the JSON decoder's field aliases.
- Scenario tests: `naive` must be disqualified with two fatal findings; `hardened` must have zero.
- Golden tests pin the exact finding-id multiset and score, so a rule change cannot silently move
  the demo numbers.
- `web` is typechecked with `tsc --noEmit` as part of `pnpm build`.

## Build plan for the hackathon window

Assumes ~9 working days. The repo you are reading is the end of day 0–1.

| day | work | done when |
|-----|------|-----------|
| 0 | facts model, 15 rules, scoring, scenarios, CLI skeleton | `cargo test` green, both scenarios report |
| 1 | report JSON + web UI + self-contained build | screenshot of both subjects |
| 2 | node path: `doctor`, `scan --rpc`, decoder aliases | a real mainnet tx produces a report |
| 3 | real fixtures: 3 mainnet txs (a turnstile crossing, a v6 payment, a v5 legacy) committed with provenance | `scan --fixture` matches the RPC result |
| 4 | peer sampling from a block range (gives LB-002/LB-013 real anonymity sets) | k-anonymity is measured, not assumed |
| 5 | wallet facts importer (notes + addresses from a local wallet export) | wallet rules active on real data |
| 6 | false-positive pass: run over 50 real transactions, tune, document misses | `docs/HEURISTICS.md` updated with observed behaviour |
| 7 | CI action + `ci --fail-on high` in a workflow, README polish | a PR fails on a fatal report |
| 8 | demo video (3 min), submission text, screenshots, tag `v0.1` | submitted |

**If time collapses, cut in this order:** peer sampling → wallet importer → CI action. Never cut:
the honest `disclaimer`, the evidence tables, or the negative tests. Those are what make the tool
credible rather than decorative.

## Threat model of LeakBench itself

- Wallet facts are read from a local file; nothing is uploaded and no telemetry exists.
- RPC is localhost-only and unauthenticated by design; a remote node requires a tunnel.
- Reports contain transaction ids and amounts, never keys. A report is safe to publish; a *wallet
  export* is not, and the README says so.
- The UI is static and makes exactly one kind of request: `api/index.json` plus report JSON.
