#!/usr/bin/env python3
"""plan.py — the single source of truth for the LeakBench film.

Writes timeline.json: scenes that start and end on bar lines, plus every event
on the musical grid. film.js draws from it and score.py plays from it, so every
hit in the picture is a hit in the sound.

96 BPM -> BEAT 0.625 s, BAR 2.5 s, 48 bars = 120 s.
"""
import json
import math
import pathlib

BPM = 96
BEAT = 60.0 / BPM          # 0.625
BAR = 4 * BEAT             # 2.5
EIGHTH = BEAT / 2
SIXTEENTH = BEAT / 4
THIRTY2 = BEAT / 8
FPS = 30
W, H = 1920, 1080
BARS = 48
DURATION = BARS * BAR      # 120.0


def at(bar, beat=0.0):
    return bar * BAR + beat * BEAT


def r(x):
    return round(x, 6)


# --- seeded randomness, spent only here, never per frame -------------------

def mulberry32(seed):
    a = seed & 0xFFFFFFFF

    def rnd():
        nonlocal a
        a = (a + 0x6D2B79F5) & 0xFFFFFFFF
        t = a
        t = ((t ^ (t >> 15)) * (t | 1)) & 0xFFFFFFFF
        t ^= (t + (((t ^ (t >> 7)) * (t | 61)) & 0xFFFFFFFF)) & 0xFFFFFFFF
        return ((t ^ (t >> 14)) & 0xFFFFFFFF) / 4294967296.0

    return rnd


RND = mulberry32(2610)

SCENES = []
EVENTS = []


def scene(sid, start_bar, end_bar, template, **data):
    s = {
        "id": sid,
        "start": r(at(start_bar)),
        "end": r(at(end_bar)),
        "template": template,
        **data,
    }
    SCENES.append(s)
    return s


def ev(kind, t, **kw):
    e = {"kind": kind, "t": r(t), **{k: (r(v) if isinstance(v, float) else v) for k, v in kw.items()}}
    EVENTS.append(e)
    return e


def wipe(cut):
    """The sweep that covers a scene change, starting 0.30 before the cut."""
    ev("sweep", cut - 0.30, dur=0.55, lo=380, hi=3400, up=True, gain=0.24, pan=0.0)


def slam(t, gain=0.22, pan=0.0, w=0.5, target="", scene_id=""):
    ev("slam", t, gain=gain, pan=pan, w=w, target=target, scene=scene_id)


def typed(t0, text, per_char, level=1.0, pan=0.0, gain=0.10, scene_id=""):
    """One event per character, on the grid; returns the end time."""
    t = t0
    for i, ch in enumerate(text):
        ev("typekey", t, level=r(level * (0.75 + 0.25 * RND())), gain=gain, pan=pan,
           char=ch, index=i, scene=scene_id)
        t += per_char
    return t


# =========================================================================
# 1 · OPEN — 0..10 s
# =========================================================================
s_open = scene(
    "open", 0, 4, "title",
    eyebrow="LEAKBENCH · ZECATHON 2026 · CORE & TOOLING",
    eyebrow_t=-0.18,
    lines=[
        {"text": "Every shielded", "cls": "display", "size": 150, "t": -0.18, "top": 246},
        {"text": "payment publishes", "cls": "display", "size": 150, "t": r(at(0, 2)), "top": 366},
        {"text": "an amount.", "cls": "display", "size": 150, "t": r(at(1)), "top": 486},
    ],
    sub={"text": "A leak oracle for Zcash. Facts in, findings out.", "t": r(at(2)), "top": 716},
    mascot={"enter": r(at(2, 2)), "x": 1530, "y": 730, "w": 250, "h": 108},
)
slam(at(0), target="eyebrow", scene_id="open")
for i, line in enumerate(s_open["lines"]):
    slam(line["t"], target=f"line{i}", scene_id="open")
    slam(line["t"], gain=0.10, pan=0.1, w=0.3)
ev("sweep", at(2, 2) - 0.15, dur=0.4, lo=200, hi=1400, up=True, gain=0.16, pan=0.2)
ev("bloop", at(2, 2) + 1.2, f0=800, f1=1300, dur=0.05, gain=0.12, pan=0.2)
ev("bloop", at(2, 2) + 1.2 + EIGHTH, f0=800, f1=1300, dur=0.05, gain=0.10, pan=0.25)
ev("bloop", at(3, 1), f0=420, f1=760, dur=0.09, gain=0.14, pan=0.2)
slam(s_open["sub"]["t"], gain=0.14, target="sub", scene_id="open")

# =========================================================================
# 2 · THE TURNSTILE — 10..20 s   (chapter word + redacted rows)
# =========================================================================
s_cross = scene(
    "crossing", 4, 8, "chapter",
    word="The turnstile",
    cues={
        "land": r(at(4)),
        "move": r(at(4) + 1.25 * BEAT),
        "sheet": r(at(4, 2)),
        "lines": [r(at(5, 2)), r(at(6))],
        "redact": r(at(6, 2)),
        "lift": r(at(7, 2)),
    },
    rows=[
        {"left": "orchard → ironwood", "right": "+5.00000000 ZEC"},
        {"left": "ironwood balance", "right": "-5.00000000 ZEC"},
        {"left": "height", "right": "3,500,041"},
    ],
    lines=[
        {"text": "valueBalance is a consensus field.", "t": r(at(5, 2)), "top": 258},
        {"text": "It publishes the amount. Every time.", "t": r(at(6)), "top": 322},
    ],
)
slam(s_cross["cues"]["land"], gain=0.4, w=0.6, target="word", scene_id="crossing")
ev("kick", s_cross["cues"]["land"], gain=0.6)
ev("stamp", s_cross["cues"]["land"] + 0.01, scale=1.3, pitch=0.8, gain=0.4)
ev("sweep", s_cross["cues"]["sheet"] - 0.05, dur=0.25, lo=500, hi=3000, up=False, gain=0.16, pan=0.25)
for i, row in enumerate(s_cross["rows"]):
    t = s_cross["cues"]["sheet"] + i * EIGHTH
    slam(t + 4 * SIXTEENTH, gain=0.3, pan=r(-0.3 + 0.08 * i), w=0.6, target=f"row{i}", scene_id="crossing")
for i, line in enumerate(s_cross["lines"]):
    slam(line["t"], gain=0.2, target=f"line{i}", scene_id="crossing")
for i in range(3):
    t = s_cross["cues"]["lift"] + i * EIGHTH
    ev("sweep", t - 0.12, dur=0.18, lo=900, hi=4200, up=True, gain=0.12, pan=r(-0.3 + 0.1 * i))

# =========================================================================
# 3 · FINDINGS — 20..30 s  (counter + dots)
# =========================================================================
s_count = scene(
    "findings", 8, 12, "counter",
    heading="23 findings",
    cues={
        "heading": r(at(8)),
        "count_start": r(at(8, 2)),
        "count_end": r(at(10)),
        "tiles": r(at(8, 1)),
    },
    count={"from": 0, "to": 23, "rows": 22},
    lines=[
        {"text": "Fourteen rules fire on one wallet.", "t": r(at(10, 1)), "top": 770},
        {"text": "Score 0. Grade F. Disqualified.", "t": r(at(10, 3)), "top": 828},
    ],
)
slam(s_count["cues"]["heading"], target="heading", scene_id="findings")
slam(s_count["cues"]["tiles"], gain=0.12, w=0.35)
# 28 accelerating ticks across the count window
_start, _end = s_count["cues"]["count_start"], s_count["cues"]["count_end"]
for x in range(28):
    ev("typekey", _start + (_end - _start) * (1 - (1 - x / 27) ** 1.8),
       level=r(0.5 + 0.5 * RND()), gain=0.07, pan=r(-0.4 + 0.03 * x))
for i, line in enumerate(s_count["lines"]):
    slam(line["t"], gain=0.18, target=f"line{i}", scene_id="findings")

# =========================================================================
# 4 · THE EVIDENCE — 30..40 s  (real screen + camera + stamp)
# =========================================================================
s_screen = scene(
    "evidence", 12, 16, "screen",
    heading="What it actually sees",
    cues={
        "heading": r(at(12)),
        "sheet": r(at(12, 1.5)),
        "box1": r(at(13)),
        "box2": r(at(13, 2)),
        "stamp": r(at(14, 2)),
    },
    image="../assets/art/screen-findings.png",
    sheet={"x": 830, "y": 250, "w": 980},
    boxes=[
        {"x": 690, "y": 1150, "w": 1600},
        {"x": 690, "y": 1500, "w": 1200},
    ],
    stamp={"text": "Not verified", "ok": False, "rot": -5},
    lines=[{"text": "One transaction. One public number.", "t": r(at(15)), "top": 980}],
)
slam(s_screen["cues"]["heading"], target="heading", scene_id="evidence")
ev("sweep", s_screen["cues"]["sheet"] - 0.05, dur=0.25, lo=500, hi=3000, up=False, gain=0.16, pan=0.25)
ev("sweep", s_screen["cues"]["box1"] - 0.15, dur=0.3, lo=300, hi=1800, up=True, gain=0.10)
ev("sweep", s_screen["cues"]["box2"] - 0.15, dur=0.3, lo=300, hi=1800, up=True, gain=0.10)
ev("stamp", s_screen["cues"]["stamp"], scale=1.6, pitch=1.0, gain=0.55)
ev("kick", s_screen["cues"]["stamp"] + 0.01, gain=0.45)
slam(s_screen["lines"][0]["t"], gain=0.2, target="line0", scene_id="evidence")

# =========================================================================
# 5 · COMPARE — 40..50 s  (two sheets, two stamps)
# =========================================================================
s_cmp = scene(
    "compare", 16, 20, "compare",
    heading="0 → 79",
    cues={
        "heading": r(at(16)),
        "sheetA": r(at(16, 1.5)),
        "sheetB": r(at(16, 2)),
        "stampA": r(at(17, 2)),
        "stampB": r(at(18)),
        "line": r(at(19)),
    },
    sheets=[
        {"label": "naive wallet", "score": "0", "grade": "F", "side": "left",
         "stamp": {"text": "Not verified", "ok": False}},
        {"label": "hardened wallet", "score": "79", "grade": "B", "side": "right",
         "stamp": {"text": "Verified", "ok": True}},
    ],
    lines=[{"text": "Same balance. Different crossing.", "t": r(at(19)), "top": 980}],
)
slam(s_cmp["cues"]["heading"], target="heading", scene_id="compare")
for key, pan in (("sheetA", -0.2), ("sheetB", 0.2)):
    ev("sweep", s_cmp["cues"][key] - 0.05, dur=0.25, lo=500, hi=3000, up=False, gain=0.16, pan=pan)
for key, pitch in (("stampA", 0.85), ("stampB", 1.0)):
    ev("stamp", s_cmp["cues"][key], scale=1.6, pitch=pitch, gain=0.55, pan=-0.2 if key == "stampA" else 0.2)
    ev("kick", s_cmp["cues"][key] + 0.01, gain=0.45)
slam(s_cmp["lines"][0]["t"], gain=0.2, target="line0", scene_id="compare")

# =========================================================================
# 6 · THE FIX — 50..60 s  (receipt printer)
# =========================================================================
s_receipt = scene(
    "fix", 20, 24, "receipt",
    heading="How to cross",
    cues={
        "heading": r(at(20)),
        "slot": r(at(20, 1.5)),
        "print": [r(at(21) + i * EIGHTH) for i in range(5)],
        "tear": r(at(22)),
        "line": r(at(23)),
    },
    receipt={
        "width": 660,
        "pitch": 44,
        "lines": [
            {"left": "chunk", "right": "1.1273 ZEC"},
            {"left": "chunk", "right": "0.8419 ZEC"},
            {"left": "chunk", "right": "1.4097 ZEC"},
            {"left": "fee", "right": "20,000 zats"},
            {"left": "transparent leg", "right": "none"},
        ],
        "foot": "migrate in five crossings",
    },
    lines=[{"text": "Not one number. Five unremarkable ones.", "t": r(at(23)), "top": 940}],
)
slam(s_receipt["cues"]["heading"], target="heading", scene_id="fix")
ev("sweep", s_receipt["cues"]["slot"] - 0.05, dur=0.25, lo=500, hi=3000, up=False, gain=0.16, pan=0.2)
for i, t in enumerate(s_receipt["cues"]["print"]):
    ev("printer", t, dur=0.12 if i % 2 == 0 else 0.22, gain=r(0.13 + 0.03 * (i % 2)), pan=0.1)
ev("tear", s_receipt["cues"]["tear"], gain=0.16)
slam(s_receipt["lines"][0]["t"], gain=0.2, target="line0", scene_id="fix")

# =========================================================================
# 7 · THE GATE — 60..70 s  (terminal)
# =========================================================================
CMD = "leakbench ci --report hardened.json --fail-on high"
s_term = scene(
    "gate", 24, 28, "terminal",
    heading="The gate",
    cues={
        "heading": r(at(24)),
        "window": r(at(24, 1)),
        "type_start": r(at(25)),
        "outputs": [r(at(26) + i * SIXTEENTH * 2) for i in range(5)],
        "ok": r(at(27)),
    },
    command=CMD,
    outputs=[
        "facts      13 transactions",
        "findings   5 (0 fatal)",
        "score      79/100  grade B",
        "verdict    acceptable",
        "pass       exit 0",
    ],
    ok_index=4,
    lines=[{"text": "A privacy regression test, not an audit.", "t": r(at(27, 2)), "top": 940}],
)
slam(s_term["cues"]["heading"], target="heading", scene_id="gate")
ev("sweep", s_term["cues"]["window"] - 0.12, dur=0.32, lo=400, hi=2600, up=True, gain=0.16, pan=0.2)
_type_end = typed(s_term["cues"]["type_start"], CMD, BEAT / 16, level=1.0, pan=0.15, gain=0.10, scene_id="gate")
for i, t in enumerate(s_term["cues"]["outputs"]):
    ev("bloop", t, f0=1500, f1=1200, dur=0.03, gain=0.10, pan=0.15)
    ev("typekey", t + 0.02, level=0.5, gain=0.08, pan=0.15)
ev("bell", s_term["cues"]["ok"], midi=88, decay=0.5, ratio=2.0, index=0.6, gain=0.18, pan=0.2)
slam(s_term["lines"][0]["t"], gain=0.2, target="line0", scene_id="gate")

# =========================================================================
# 8 · PEERS — 70..80 s  (pinned slips)
# =========================================================================
s_slips = scene(
    "peers", 28, 32, "slips",
    heading="Hidden in a crowd",
    cues={
        "heading": r(at(28)),
        "slips": [r(at(29) + i * EIGHTH) for i in range(5)],
        "lines": [r(at(30, 2)), r(at(31))],
    },
    slips=[
        {"value": "1.1273", "tilt": -2.1},
        {"value": "0.8419", "tilt": 1.4},
        {"value": "1.4097", "tilt": 2.3},
        {"value": "0.6183", "tilt": -1.7},
        {"value": "0.9999", "tilt": 0.9},
    ],
    note="20 crossings within ±25%",
    lines=[
        {"text": "Anonymity is a set, not a property.", "t": r(at(30, 2)), "top": 812},
        {"text": "So the tool measures the set.", "t": r(at(31)), "top": 866},
    ],
)
slam(s_slips["cues"]["heading"], target="heading", scene_id="peers")
for i, t in enumerate(s_slips["cues"]["slips"]):
    ev("bell", t, freq=1320, decay=0.18, ratio=1.0, index=0.4, gain=0.14, pan=r(-0.4 + 0.2 * i))
    ev("click", t, gain=0.10, pan=r(-0.4 + 0.2 * i))
for i, line in enumerate(s_slips["lines"]):
    slam(line["t"], gain=0.18, target=f"line{i}", scene_id="peers")

# =========================================================================
# 9 · THE LIMIT — 80..90 s  (key + refusal)
# =========================================================================
s_no = scene(
    "limit", 32, 36, "refusal",
    heading="Not a proof.",
    cues={
        "heading": r(at(32)),
        "key": r(at(33)),
        "stamp": r(at(34)),
        "jiggle": r(at(34)),
        "line": r(at(35)),
    },
    stamp={"text": "Not verified", "ok": False},
    key={"label": "heuristics ≠ proofs"},
    lines=[{"text": "It reports evidence you can check by hand.", "t": r(at(35)), "top": 900}],
)
slam(s_no["cues"]["heading"], target="heading", scene_id="limit")
ev("sweep", s_no["cues"]["key"] - 0.1, dur=0.3, lo=600, hi=2000, up=True, gain=0.14)
ev("bell", s_no["cues"]["key"] + 0.15, freq=220, decay=0.5, ratio=4.1, index=0.5, gain=0.12)
ev("stamp", s_no["cues"]["stamp"], scale=0.7, pitch=0.6, gain=0.4)
ev("stamp", s_no["cues"]["stamp"] + SIXTEENTH, scale=0.7, pitch=0.6, gain=0.32)
slam(s_no["lines"][0]["t"], gain=0.18, target="line0", scene_id="limit")

# =========================================================================
# 10 · CHECK IT — 90..100 s  (phone, QR, hand-over)
# =========================================================================
s_phone = scene(
    "check", 36, 40, "phone",
    heading="Check it yourself",
    cues={
        "heading": r(at(36)),
        "phone": r(at(36, 1)),
        "scroll": r(at(37)),
        "scan": [r(at(38) + i * 0.9) for i in range(2)],
        "tick": r(at(39)),
        "handover": r(at(39, 2)),
    },
    image="../assets/art/screen-phone.png",
    lines=[{"text": "leakbench scan --peers peers.json", "t": r(at(39, 2)), "top": 880, "mono": True}],
)
slam(s_phone["cues"]["heading"], target="heading", scene_id="check")
ev("sweep", s_phone["cues"]["phone"] - 0.12, dur=0.32, lo=400, hi=2600, up=True, gain=0.16, pan=0.2)
for t in s_phone["cues"]["scan"]:
    ev("sweep", t, dur=0.45, lo=1200, hi=5200, up=True, gain=0.12)
ev("bell", s_phone["cues"]["tick"], midi=81, decay=0.4, ratio=2.0, index=0.5, gain=0.16)
ev("bell", s_phone["cues"]["tick"] + EIGHTH, midi=86, decay=0.4, ratio=2.0, index=0.5, gain=0.14)
ev("bloop", s_phone["cues"]["handover"], f0=900, f1=1400, dur=0.05, gain=0.14, pan=-0.2)

# =========================================================================
# 11 · NAME — 100..110 s
# =========================================================================
s_name = scene(
    "name", 40, 44, "name",
    word="LEAKBENCH",
    cues={
        "riser": r(at(40)),
        "bars": [r(at(40, 1) + i * SIXTEENTH) for i in range(3)],
        "reveal": r(at(41)),
        "sub": r(at(42, 2)),
    },
    sub="A leak oracle for Zcash",
)
ev("riser", s_name["cues"]["riser"], dur=2 * BEAT)
for i, t in enumerate(s_name["cues"]["bars"]):
    slam(t, gain=0.4, w=0.4, pan=r(-0.3 + 0.3 * i), target=f"bar{i}", scene_id="name")
ev("kick", s_name["cues"]["reveal"], gain=0.5)
for i, midi in enumerate([62, 66, 69, 73, 78]):
    ev("bell", s_name["cues"]["reveal"] + i * 0.004, midi=midi, decay=1.0, ratio=1.0, index=0.8,
       gain=0.16, pan=r(-0.3 + 0.15 * i))
slam(s_name["cues"]["sub"], gain=0.2, target="sub", scene_id="name")

# =========================================================================
# 12 · END — 110..120 s
# =========================================================================
s_end = scene(
    "end", 44, 48, "title",
    eyebrow="LEAKBENCH",
    lines=[
        {"text": "Leaks are disqualifying,", "cls": "display", "size": 118, "t": r(at(44)), "top": 300},
        {"text": "not deductions.", "cls": "display", "size": 118, "t": r(at(45)), "top": 410},
    ],
    sub={"text": "raizo07.github.io/LeakBench", "t": r(at(46)), "top": 640, "mono": True},
    mascot={"enter": r(at(46, 2)), "x": 1500, "y": 700, "w": 250, "h": 108},
    out=118.2,
)
slam(s_end["lines"][0]["t"], target="line0", scene_id="end")
slam(s_end["lines"][1]["t"], target="line1", scene_id="end")
slam(s_end["sub"]["t"], gain=0.16, target="sub", scene_id="end")
ev("sweep", s_end["mascot"]["enter"] - 0.15, dur=0.4, lo=200, hi=1400, up=True, gain=0.16, pan=0.2)
ev("redact", s_end["out"], gain=0.2)

# --- wipes at every scene cut ---------------------------------------------
for a, b in zip(SCENES, SCENES[1:]):
    assert a["end"] == b["start"], (a["id"], b["id"])
    wipe(a["end"])

# --- music grid ------------------------------------------------------------
PROG_A = ["Dmaj9", "Bm9", "Gmaj9", "A7sus4"]
PROG_B = ["Em9", "F#m7", "Gmaj7", "A7"]
QUIET = set(range(32, 36)) | set(range(40, 44))
MUSIC = []
for b in range(BARS):
    if b < 16:
        chord = PROG_A[b % 4]
    elif b < 32:
        chord = PROG_B[b % 4]
    else:
        chord = PROG_A[b % 4]
    MUSIC.append({"bar": b, "chord": chord, "quiet": b in QUIET})

# --- assemble --------------------------------------------------------------
EVENTS.sort(key=lambda e: (e["t"], e["kind"]))
timeline = {
    "meta": {
        "bpm": BPM, "beat": BEAT, "bar": BAR, "bars": BARS,
        "fps": FPS, "width": W, "height": H, "duration": DURATION,
        "project": "LeakBench",
        "seed": 2610,
    },
    "scenes": SCENES,
    "events": EVENTS,
    "music": {"bars": MUSIC},
}

out = pathlib.Path(__file__).parent / "timeline.json"
out.write_text(json.dumps(timeline, indent=1))

# The page loads the same data as a script so it works from file:// as well as
# over the local server render.mjs starts.
js = pathlib.Path(__file__).parent / "src" / "timeline.js"
js.write_text("window.__TIMELINE__ = " + json.dumps(timeline, separators=(",", ":")) + ";\n")
print(f"scenes {len(SCENES)} · events {len(EVENTS)} · {DURATION:.0f}s · {out.name} "
      f"{out.stat().st_size // 1024} KB")
for kind in sorted({e["kind"] for e in EVENTS}):
    print(f"  {kind:<9} {sum(1 for e in EVENTS if e['kind'] == kind)}")
