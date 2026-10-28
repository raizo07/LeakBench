# LeakBench — the film

A two-minute demo film for [LeakBench](https://github.com/raizo07/LeakBench), built on a
deterministic motion-and-sound framework: paper `#F3F3F0`, ink `#141414`, and black redaction bars.
96 BPM, 48 bars, 120 seconds, 1920×1080 at 30 fps.

Every number in the picture comes from the finished film. Timings are on the musical grid, written
once by `plan.py`, and read by both the renderer and the score, so every hit you see is a hit you
hear.

```
plan.py ──► timeline.json ──┬──► film.js  (window.seek(t)) ──► render.mjs ──► out/leakbench.mp4
                            └──► score.py (48 kHz stereo) ──► audio/score.wav ──┘
```

## Build

```bash
npm install              # puppeteer-core, pointed at the system Chrome
python3 plan.py          # writes timeline.json and src/timeline.js
python3 score.py         # writes audio/score.wav
node render.mjs          # frames -> out/leakbench.mp4 (H.264 + AAC, -14 LUFS)
```

`node render.mjs --check` revisits frames out of order and compares them byte-for-byte: `seek(t)` is a
pure function of `t`, so a revisit must be identical. `--still 42.5` renders a single PNG.

## The clock

`BEAT = 0.625 s`, `BAR = 2.5 s`, 48 bars. `at(bar, beat) = bar*BAR + beat*BEAT`. Scenes start and end
on bar lines; events sit on beats, eighths, sixteenths and thirty-seconds. Typing runs at
`BEAT/12` (words) or `BEAT/16` (commands). A beat counts as reached at `t + 0.5/fps`.

## The look

| layer | z | notes |
|---|---|---|
| paper + grain | 0–1 | grain is a 128 px tile at 0.07, behind the content, never over it |
| scene | 2 | sheets, type, bars |
| exit bars | 6 | scene ledges |
| mascot | 7 | the brand's bar with two eyes |
| wipe | 80 | 9 bars, 108 tall, every 120 px from y 6 |

Type: Archivo 800 at 72% width for display (`-0.012em`, line-height 0.9), 78% for words, IBM Plex
Mono for data. Everything scales from its `(0, 60%)` point. Sheets are white with a 1.5 px `#DAD9D3`
border, radius 10 and a two-part shadow. Bars are black, radius 6 (8 for wipe bars).

Colour is a vocabulary: green `#0B6B3A` only for a yes, red `#B3261E` only for the refusal.

## Rebuilding the frame

```
prog(t, start, dur) = clamp((t - start) / dur)
out3(k) = 1 - (1-k)^3      in3(k) = k^3
inout(k) = 4k^3 | 1 - (-2k+2)^3/2
back(k, s=1.7) = 1 + (s+1)(k-1)^3 + s(k-1)^2
pulse(t, c, h) = max(0, 1 - |t - c| / h)
```

Randomness is `mulberry32(2610)`, spent while building, never per frame.

## The moves

Each is implemented in `src/film.js` against the events in `timeline.json`.

| move | where |
|---|---|
| text in (slam) / text out (redact, one bar per measured line box) | every scene |
| scene change (9-bar wipe with per-bar lag) | all 11 cuts |
| chapter word (300 px at 150,330 → 126 px at 160,68) | "The turnstile" |
| redacting data (two bars per row) | the crossing sheet |
| name reveal (3 bars, 92 tall, 104 apart) | LEAKBENCH |
| sheet in / sheet out | screens, cards, receipt |
| camera on a real screen (2× captures, source boxes) | the findings screen |
| stamp (7 px border, radius 14, multiply) | Verified / Not verified |
| receipt printer (slot, torn top, tear-off) | the fix |
| typing (one event per character, block caret) | the gate |
| counter and dots (22 px, 30 apart, 22 per row) | 20 findings |
| pop-in tiles | shrinking into the first dots |
| bars lifting off rows | the crossing sheet leaving |
| pinned slips (26 px pin, seeded tilt) | anonymity set |
| key (slides in, turns, jiggles on refusal) | not a proof |
| windows (terminal, phone) + phone-to-camera scan | the gate, check it yourself |
| hand-over slip (arc 120 px, tilt -8° → -2°) | phone → words |
| mascot: wake, blink, wink, land, hop | opening and close |

## The score

`score.py` synthesises everything with numpy at 48 kHz: `slam`, `sweep`, `stamp`, `typekey`,
`printer`, `bell` (FM), `bloop`, `riser`, `kick`, `tear`, noise reverb (1.3 s, decay 0.32, 6 kHz
low-pass) on sends, and a band built from the chord map in `timeline.json`:

- Dmaj9 · Bm9 · Gmaj9 · A7sus4, with Em9 · F#m7 · Gmaj7 · A7 for the middle chapters;
- FM electric piano (ratio 1, index `2e^(-t/0.32) + 0.22`, a faint 7× partial, ±0.15% detune);
- stabs on beat 1 and the and-of-2, an octave-up on the and-of-4 every other bar, held chords in
  quiet bars;
- sub bass (sine + 0.22 of the second harmonic through `tanh(1.3x)`) on 1, 2, the and-of-2, the
  fifth on 3, and 4;
- a kick on every beat (1 and 3 louder), the band's stamp on 2 and 4, hats on eighths swung by 0.06.

Peaks at −6 dBFS; `render.mjs` runs ffmpeg `loudnorm` to −14 LUFS and muxes AAC 256 kbps.

## What is in the film

| bars | scene | |
|---|---|---|
| 0–4 | open | "Every shielded payment publishes an amount." |
| 4–8 | the turnstile | a crossing sheet: +5.00000000 ZEC, height 3,500,041 |
| 8–12 | 20 findings | counter + dots |
| 12–16 | what it actually sees | the real findings screen, camera to the fatal card, "Not verified" |
| 16–20 | 0 → 79 | naive vs hardened, red and green stamps |
| 20–24 | how to cross | receipt printer, five chunks, torn off |
| 24–28 | the gate | `leakbench ci …` typed, a green pass |
| 28–32 | hidden in a crowd | pinned slips, 20 crossings within ±25% |
| 32–36 | not a proof | key turns, refusal stamp |
| 36–40 | check it yourself | phone, scan line, tick, hand-over slip |
| 40–44 | name | LEAKBENCH |
| 44–48 | end | "Leaks are disqualifying, not deductions." |

## Specified, implemented, and not used in this cut

The framework implements the moves this film needs. Three are described in the spec but have no
place in a two-minute product demo, and are deliberately absent rather than faked:

- **pop-in tiles** (scale 0.5 → 1 from −200 px on sixteenths, with FM bells) — the counter uses the
  sibling move "tiles shrink into the first dots" instead;
- **mascot peek / push / peel** — the film has no card for it to peek over, no pile for it to push,
  and no bar that becomes the mascot, so those three would have been animation with nothing to do;
- **camera push-in that lands on a hit** — the camera drifts and zooms, but every move ends on a
  drift rather than a beat landing.

## What the numbers are

The two wallets in the film are the deterministic simulator from
[`crates/leakbench-sim`](../crates/leakbench-sim): a designed A/B that cannot exist on chain. Every
report in the repo labels them `simulated`, and the UI prints `simulated` in its header.

The analyzer is separately exercised against **real mainnet transactions** — an Orchard→Ironwood
crossing, an unshielding, and a round 8 ZEC unshield — in [`fixtures/mainnet`](../fixtures/mainnet)
with provenance. Those are the numbers to quote when someone asks whether it works on the chain.

## Notes

- Capture screenshots at 2× so the camera zooms stay sharp (`assets/art/screen-*.png`).
- Moving screenshot sheets get `will-change: transform` and images `decoding="sync"`; without them a
  revisited frame can land a pixel off.
- Terminal lines stay within the window: 24 px Plex Mono fits 58 characters in 850 px.
- The page is loaded over a local HTTP server, not `file://`, so fonts and relative paths resolve the
  same way every run.
