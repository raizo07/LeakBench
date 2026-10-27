# Heuristic catalogue

Each rule answers one question: *what does this transaction, or this wallet, tell an observer that
it should not?* Rules are pure functions of the fact bundle — no node, no clock, no randomness — so
every finding is reproducible from the JSON in the report.

Two fields matter for reading this document:

- **severity** is the impact if the observation holds: `critical`, `high`, `medium`, `low`, `info`.
- **confidence** is how much evidence we actually have: `high` when the observable is consensus data
  or supplied directly, `medium` when it depends on a supplied but unverified field, `low` when the
  anonymity set is unmeasured.

`fatal` is reserved for disclosures that are disqualifying rather than deductible. It is only set on
`leak` findings, and only when the disclosed value identifies someone.

---

## LB-001 · Turnstile amount disclosure — `leak`

**Observable.** Per-pool `valueBalance`. On a crossing it is `+X` in the source pool and `−X` in the
destination pool.

**Why it leaks.** `X` is consensus data. Any explorer shows it; no cryptography is broken to get it.
For a same-pool transfer the balance equals the fee, so nothing is revealed — the problem exists only
at the edges.

**Rating.** `critical` + `fatal` when the amount is identifying: a whole-ZEC or 0.1-ZEC amount, the
wallet's entire balance in the source pool, fewer than three peer crossings within ±25% of the value,
or no peer sample at all. Otherwise `medium`, not fatal, and the summary states the band count.

**False positives.** A crossing whose amount sits inside a dense band is still a disclosure; the
rule reports it as such and does not inflate it to fatal. A cross-pool transaction that also has a
transparent leg reports the smaller magnitude, so the number is conservative.

**Fix.** Never cross with an amount you would not publish. Chunk; use non-round amounts; keep every
chunk inside the common size band; avoid crossing your whole balance.

**Refs.** [ZIP 258](https://zips.z.cash/zip-0258) · [protocol §4.5](https://zips.z.cash/protocol/protocol.pdf)

---

## LB-002 · Turnstile timing anonymity set — `linkage`

**Observable.** The count of peer crossings inside ±20 blocks of this crossing (kind, shape or
amount match).

**Why it leaks.** Anonymity is a set, not a property of a transaction. A crossing alone in its hour
is a crossing with an anonymity set of one, no matter how carefully the amount was chosen.

**Rating.** `high` for 0–1 peers, `medium` for 2–4, silent at 5+. `confidence: low` when no peer
sample was supplied — at that point the rule says the set is *unmeasured* instead of pretending.

**False positives.** Busy windows make everything look safe: if 200 users cross in the same hour,
that is genuinely better for you. The rule does not try to detect wash crossings.

**Fix.** Randomise delay in hours, not blocks, and avoid known migration waves (upgrade days,
exchange announcements) where your crossing is one of very few.

---

## LB-003 · Transparent leg — `leak`

**Observable.** A transaction with shielded spends and transparent outputs (unshielding), or
transparent inputs and shielded outputs (shielding).

**Why it leaks.** The transparent pool is public: amount, addresses and timing. One transparent leg
undoes the shielded set for that payment, and links the shielded side to a public identifier.

**Rating.** Unshielding is `critical` + `fatal`: value that was hidden is now published with an
address. Shielding is `high`, not fatal: the amount entering the pool becomes public, which is bad
but is not a link to whoever receives it.

**False positives.** Exchange deposits are often unavoidable, and the rule does not know whether the
recipient forced it. The finding says so in the remediation.

**Fix.** Pay shielded recipients shielded. If a venue forces a transparent deposit address, treat
that amount as public and keep it separate from the rest of the balance.

**Refs.** [ZIP 320](https://zips.z.cash/zip-0320) (transparent-source-only addresses)

---

## LB-004 · Fee fingerprint — `fingerprint`

**Observable.** `fee` versus the ZIP 317 conventional fee for the transaction's logical actions.

**Why it leaks.** Wallets hard-code fee policies. A fee that is exactly 10,000 zats on a
three-action transaction, or a round 100,000, groups every transaction that shares it into one
software cohort. A unique fee is worse than a common one: it isolates you.

**Rating.** `medium` + high confidence when the fee is a round number (`fee % 10_000 == 0`),
`low` otherwise. Paying exactly the ZIP 317 floor is silence.

**False positives.** Fee sniping and deliberate overpayment exist, but they are rare enough to
report.

**Fix.** Pay exactly `5,000 × max(grace_actions, logical_actions)` zats. Do not randomise upward.

**Refs.** [ZIP 317](https://zips.z.cash/zip-0317)

---

## LB-005 · Action-count disclosure — `fingerprint`

**Observable.** Per-bundle spend/output counts, and total logical actions.

**Why it leaks.** Counts are public. Three spends in one bundle says "this wallet held at least three
notes and consumed them together". A 1-in/1-out shape usually means a whole note was spent at once.

**Rating.** `medium` for more than two spends in a bundle; `low` for a minimal 1-in/1-out transfer.
Crossings are skipped here because LB-001 already covers them and 1-in/1-out is the normal crossing
shape.

**False positives.** Padding to 2-in/2-out costs block space; the rule does not model the cost, only
the disclosure.

**Fix.** Pad to ordinary shapes: 2/2 when change is needed, 1/2 otherwise. Consolidate notes in a
transaction that is not adjacent to an identifying payment.

**Refs.** [ZIP 225](https://zips.z.cash/zip-0225)

---

## LB-006 · Anchor staleness — `fingerprint`

**Observable.** `anchor_age_blocks`: how far behind the tip the spend's anchor was. Supplied by a
wallet-side scan; not directly observable on-chain.

**Why it leaks.** Sync cadence is behavioural. Wallets that sync on open produce anchors a block or
two old; wallets that sync hourly produce a lag that clusters. An unusually stale anchor is rare.

**Rating.** `medium` above 10 blocks, `high` above 100. `confidence: medium` because the caller
supplies the value.

**False positives.** A long-offline wallet legitimately has a stale anchor. The finding is still
true.

**Fix.** Sync to the tip immediately before spending.

---

## LB-007 · Temporal clustering — `linkage`

**Observable.** Three or more transactions from the subject inside eight blocks.

**Why it leaks.** Grouping is free. A burst then shares the anonymity set of its weakest member, and
at 25-second blocks (NU7) eight blocks is three and a half minutes.

**Rating.** `medium`, high confidence — the heights are consensus data.

**False positives.** Batch payroll is a legitimate burst; it is still a group.

**Fix.** Separate unrelated actions by hours. Queue and release on a randomised schedule.

---

## LB-008 · Transparent address reuse — `linkage`

**Observable.** The same transparent address appearing in two or more transactions, or a wallet
address whose `uses` field is at least 2.

**Why it leaks.** Every transaction touching a t-address is linked, and the linkage survives any
amount of shielding afterwards. It is the cheapest heuristic an observer has.

**Rating.** `high`, high confidence.

**Fix.** One fresh transparent address per transaction, or stay shielded.

**Refs.** [ZIP 316](https://zips.z.cash/zip-0316) (unified addresses)

---

## LB-009 · Round-number amounts — `fingerprint`

**Observable.** Publicly visible amounts: transparent inputs/outputs and turnstile crossings.

**Why it leaks.** Real payments have change and fees; whole-ZEC public amounts are rare, so they
stand out in any amount histogram. "Someone moved exactly 5 ZEC across the turnstile at height
3,500,041" is a searchable sentence.

**Rating.** `medium`, high confidence. Only whole- and one-decimal-ZEC amounts are reported;
0.01-ZEC granularity is too common to be informative.

**Fix.** Do not send or migrate whole-ZEC amounts from an account you care about.

---

## LB-010 · Expiry-height fingerprint — `fingerprint`

**Observable.** `nExpiryHeight` set to a non-zero value.

**Why it leaks.** Most wallets leave it unset. Setting it — or worse, setting it to a distinctive
value — separates those transactions from the crowd, and a fixed offset groups them together.

**Rating.** `low`, medium confidence, aggregated across the subject rather than one finding per
transaction.

**Fix.** Match the ecosystem default unless expiry is needed, and never use it as a marker.

**Refs.** [ZIP 203](https://zips.z.cash/zip-0203)

---

## LB-011 · Legacy transaction version — `hygiene`

**Observable.** `version == 4`.

**Why it leaks.** Version 4 transactions become invalid at NU7 (mainnet height 3,543,405,
5 November 2026), and until then they are a pre-NU7 fingerprint: a wallet that still builds them is
a wallet of a particular vintage. Sprout had no post-activation replacement format at all.

**Rating.** `medium`, high confidence.

**Fix.** Build v5/v6. Re-create pending v4 transactions; signatures do not carry over.

**Refs.** [ZIP 259](https://zips.z.cash/zip-0259) · [ZIP 2003](https://zips.z.cash/zip-2003)

---

## LB-012 · Memo-length fingerprint — `fingerprint`

**Observable.** Distinct non-empty memo lengths across the wallet's notes (wallet-side only).

**Why it leaks.** Memo *content* is encrypted; the size pattern is not. A wallet that sends 512-byte
invoices and 216-byte chat messages has two classes of payment, visible in aggregate.

**Rating.** `low`, low confidence — it needs several notes to be meaningful.

**Fix.** Pad memos to a fixed length. Note that memo bundles (ZIP 231) are still Draft and not
deployed, so 512 bytes per output remains the ceiling.

**Refs.** [ZIP 302](https://zips.z.cash/zip-0302) · [ZIP 231](https://zips.z.cash/zip-0231) (Draft)

---

## LB-013 · Unique note value — `linkage`

**Observable.** A wallet note whose value appears in fewer than three peer observations.

**Why it leaks.** When a value is unique, an observer can follow the amount instead of the address.
Chunking a balance into irregular amounts can create exactly this problem while solving another one.

**Rating.** `high` at zero peers, `medium` at one or two. Skipped entirely without a peer sample,
because an unmeasured histogram cannot support the claim.

**Fix.** Break unique denominations: split into a small set of common denominations, or receive into
an account whose value histogram resembles everyone else's.

---

## LB-014 · Pool fragmentation — `fingerprint`

**Observable.** Wallet notes spanning three or more pools.

**Why it leaks.** Each pool's activity is separately observable, and a wallet present in Sapling,
Orchard and Ironwood documents its own migration path and its age.

**Rating.** `medium`, high confidence.

**Fix.** Consolidate into the current pool through one carefully-sized crossing; stop using retired
pools for new activity.

---

## LB-015 · Full-balance migration — `leak`

**Observable.** A crossing amount equal to the sum of the wallet's notes in the source pool, with at
least two notes consumed.

**Why it leaks.** LB-001 already says the amount is public. This says something worse: the amount is
your *entire* holding, and the new account inherits the old account's whole history rather than one
note's worth of it.

**Rating.** `high`, medium confidence (it depends on wallet-side facts being complete).

**Fix.** Migrate in chunks that are individually unremarkable, and leave a remainder behind
initially.

---

## What the catalogue does not cover

Deliberate omissions, so the gaps are legible rather than accidental:

- **Network-level correlation** (IP, peer graph, light-client queries). Out of scope: this reads
  chain data, not packet captures.
- **Cross-chain linkage.** The tool stops at the Zcash boundary; a bridge's inventory analysis is a
  different project.
- **Anonymity-set estimation over the whole chain.** The peer sample is caller-supplied. A full
  chain-wide histogram is the obvious next step and would make LB-002 and LB-013 much sharper.
- **Proof of no-leak.** Absence of findings is not a proof. The report says so in the disclaimer,
  and the tool is named after what it does, not what it promises.
