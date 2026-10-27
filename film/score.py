#!/usr/bin/env python3
"""score.py — the soundtrack, synthesised from timeline.json.

48 kHz stereo. Every sound is built from the same event list film.js draws
from, so the picture and the score cannot drift apart. Peaks at -6 dBFS; the
final loudness pass lives in render.mjs (ffmpeg loudnorm to -14 LUFS).
"""
import json
import pathlib

import numpy as np

SR = 48_000
DUR = 120.0
N = int(SR * DUR)
RNG = np.random.default_rng(2610)

# --------------------------------------------------------------------------
# helpers
# --------------------------------------------------------------------------

def t_axis(seconds):
    return np.arange(int(seconds * SR)) / SR


def noise(seconds):
    return RNG.standard_normal(int(seconds * SR)).astype(np.float32)


def env_ad(n, attack, decay):
    t = np.arange(n) / SR
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    return (a * np.exp(-t / max(decay, 1e-4))).astype(np.float32)


def onepole_lp(x, cutoff):
    a = np.exp(-2 * np.pi * cutoff / SR)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc = (1 - a) * x[i] + a * acc
        y[i] = acc
    return y


def fft_filter(x, lo=None, hi=None, order=2.0):
    """Brickwall-ish band filter in the frequency domain (cheap and stable)."""
    n = len(x)
    spec = np.fft.rfft(x)
    freqs = np.fft.rfftfreq(n, 1 / SR)
    gain = np.ones_like(freqs)
    if lo:
        gain *= 1 / (1 + (lo / np.maximum(freqs, 1e-6)) ** (2 * order))
    if hi:
        gain *= 1 / (1 + (freqs / hi) ** (2 * order))
    return np.fft.irfft(spec * gain, n).astype(np.float32)


def env_sine(n, shape=1.0):
    t = np.linspace(0, 1, n, endpoint=False)
    return np.sin(np.pi * t) ** shape


# --------------------------------------------------------------------------
# instruments
# --------------------------------------------------------------------------

def slam(w=0.5):
    n = int(0.25 * SR)
    t = np.arange(n) / SR
    f = 100 * (60 / 100) ** (t / 0.09)
    thud = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.09)
    nz = fft_filter(noise(0.25), 350, 2600)
    body = nz * env_ad(n, 0.008, 0.06)
    sig = thud * w + body * 0.8 * w
    return sig.astype(np.float32)


def sweep(dur=0.3, lo=400, hi=2600, up=True):
    n = int((dur + 0.25) * SR)
    t = np.arange(n) / SR
    k = np.clip(t / dur, 0, 1)
    base = noise(dur + 0.25)
    out = np.zeros(n, dtype=np.float32)
    bands = np.exp(np.linspace(np.log(lo), np.log(hi), 8))
    for i, f in enumerate(bands):
        band = fft_filter(base, f / 1.18, f * 1.18)
        centre = (i + 0.5) / 8 if up else 1 - (i + 0.5) / 8
        bump = np.exp(-0.5 * ((k - centre) / 0.02) ** 2)
        out += band * bump
    out *= env_sine(n, 1.4)
    return out.astype(np.float32)


def stamp(scale=1.0, pitch=1.0):
    n = int(0.5 * SR)
    t = np.arange(n) / SR
    body = np.sin(2 * np.pi * 185 * pitch * t) * np.exp(-t / 0.06)
    crack = fft_filter(noise(0.5), 700, 2600) * np.exp(-t / 0.08)
    crunch = fft_filter(noise(0.5), 3000, 6500) * np.exp(-t / 0.025)
    thump = np.sin(2 * np.pi * 68 * pitch * t) * np.exp(-t / 0.12)
    sig = (body * 0.9 + crack * 0.7 + crunch * 0.5 + thump * 1.0) * scale
    return sig.astype(np.float32)


def typekey(level=1.0):
    n = int(0.04 * SR)
    t = np.arange(n) / SR
    click = fft_filter(noise(0.04), 2500, None) * np.exp(-t / 0.004)
    ping = np.sin(2 * np.pi * 3150 * t) * np.exp(-t / 0.012)
    return ((click * 0.8 + ping * 0.6) * level).astype(np.float32)


def printer(length=0.12):
    n = int(length * SR)
    t = np.arange(n) / SR
    buzz = np.tanh(3.5 * np.sin(2 * np.pi * 150 * t))
    gate = (np.sign(np.sin(2 * np.pi * 42 * t)) + 1) / 2
    whir = fft_filter(noise(length), 4200, 8000) * 0.35
    return ((buzz * gate * 0.5 + whir) * 0.8).astype(np.float32)


def bell(freq=1320.0, decay=0.4, ratio=1.0, index=0.6, ):
    n = int(max(0.3, decay * 2.2) * SR)
    t = np.arange(n) / SR
    idx = index * np.exp(-t / max(0.3 * decay, 1e-3)) if decay > 0 else index
    mod = np.sin(2 * np.pi * freq * ratio * t) * idx
    sig = np.sin(2 * np.pi * freq * t + mod) * np.exp(-t / max(decay, 1e-3))
    return sig.astype(np.float32)


def bloop(f0=900, f1=1400, dur=0.05):
    n = int((dur + 0.12) * SR)
    t = np.arange(n) / SR
    k = np.clip(t / dur, 0, 1)
    f = f0 * (f1 / f0) ** k
    return (np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.06)).astype(np.float32)


def riser(dur=1.25):
    n = int(dur * SR)
    t = np.arange(n) / SR
    k = t / dur
    base = fft_filter(noise(dur), 4500, None)
    return (base * (k ** 3)).astype(np.float32)


def kick(gain=0.8):
    n = int(0.4 * SR)
    t = np.arange(n) / SR
    f = 116 * (46 / 116) ** (t / 0.045)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.26)
    click = fft_filter(noise(0.4), 1800, 2600) * np.exp(-t / 0.01)
    return ((body + click * 0.25) * gain).astype(np.float32)


def click(gain=0.1):
    n = int(0.06 * SR)
    t = np.arange(n) / SR
    return (fft_filter(noise(0.06), 2000, 5000) * np.exp(-t / 0.04) * gain).astype(np.float32)


def tear():
    n = int(0.3 * SR)
    t = np.arange(n) / SR
    body = fft_filter(noise(0.3), 900, 6000)
    flutter = (np.sin(2 * np.pi * 38 * t) + 1) / 2
    return (body * (0.35 + 0.65 * flutter) * np.exp(-t / 0.07)).astype(np.float32)


def epiano(freq, decay=1.5, detune=0.0):
    n = int(decay * 2 * SR)
    t = np.arange(n) / SR
    idx = 2 * np.exp(-t / 0.32) + 0.22
    f = freq * (1 + detune)
    mod = np.sin(2 * np.pi * f * t) * idx
    sig = np.sin(2 * np.pi * f * t + mod)
    sig += 0.08 * np.sin(2 * np.pi * f * 7 * t) * np.exp(-t / 0.4)
    return (sig * np.exp(-t / decay)).astype(np.float32)


def bass(freq, dur=0.5):
    n = int(dur * SR)
    t = np.arange(n) / SR
    sig = np.sin(2 * np.pi * freq * t) + 0.22 * np.sin(2 * np.pi * freq * 2 * t)
    sig = np.tanh(1.3 * sig)
    return (sig * np.exp(-t / (dur * 0.5))).astype(np.float32)


# --------------------------------------------------------------------------
# the mix
# --------------------------------------------------------------------------
DRY = np.zeros((N, 2), dtype=np.float32)
SEND = np.zeros((N, 2), dtype=np.float32)


def add(sig, t, gain=0.3, pan=0.0, send=0.25):
    if sig is None or len(sig) == 0:
        return
    start = int(t * SR)
    if start < 0:
        sig = sig[-start:]
        start = 0
    end = min(N, start + len(sig))
    if end <= start:
        return
    chunk = sig[: end - start]
    l = np.cos((pan + 1) * np.pi / 4)
    r = np.sin((pan + 1) * np.pi / 4)
    DRY[start:end, 0] += chunk * gain * l
    DRY[start:end, 1] += chunk * gain * r
    SEND[start:end, 0] += chunk * gain * l * send
    SEND[start:end, 1] += chunk * gain * r * send


def midi_hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# --------------------------------------------------------------------------
# read the plan
# --------------------------------------------------------------------------
ROOT = pathlib.Path(__file__).parent
TL = json.loads((ROOT / "timeline.json").read_text())
META = TL["meta"]
BEAT = META["beat"]
BAR = META["bar"]

VOICINGS = {
    "Dmaj9": [50, 54, 57, 61, 64],
    "Bm9": [47, 50, 54, 57, 61],
    "Gmaj9": [43, 47, 50, 54, 57],
    "A7sus4": [45, 50, 52, 55],
    "Em9": [40, 43, 47, 50, 54],
    "F#m7": [42, 45, 49, 52],
    "Gmaj7": [43, 47, 50, 54],
    "A7": [45, 49, 52, 55],
}

# --- band ------------------------------------------------------------------
# The band is the sustained layer; the events are the hits. MUS keeps the bed
# under the picture without the whole mix leaning on the compressor later.
MUS = 2.1
for entry in TL["music"]["bars"]:
    bar = entry["bar"]
    chord = entry["chord"]
    quiet = entry["quiet"]
    t0 = bar * BAR
    notes = VOICINGS[chord]
    root = notes[0]

    if quiet:
        for m in notes:
            for det, pan in ((-0.0015, -0.35), (0.0015, 0.35)):
                add(epiano(midi_hz(m), decay=2.2, detune=det), t0,
                    gain=0.055 * MUS, pan=pan, send=0.5)
        for beat_at, g in ((0, 0.35), (1, 0.30), (2, 0.30), (3, 0.28)):
            add(kick(1.0), t0 + beat_at * BEAT, gain=g * 0.5)
        continue

    for beat_at, gain in ((0.0, 0.10), (1.5, 0.08)):
        for i, m in enumerate(notes):
            pan = -0.3 + 0.15 * i
            add(epiano(midi_hz(m), decay=1.5, detune=-0.0015), t0 + beat_at * BEAT,
                gain=gain * MUS, pan=pan - 0.1, send=0.35)
            add(epiano(midi_hz(m), decay=1.5, detune=0.0015), t0 + beat_at * BEAT,
                gain=gain * MUS, pan=pan + 0.1, send=0.35)
    if bar % 2 == 1:
        for i, m in enumerate(notes[:3]):
            add(epiano(midi_hz(m + 12), decay=1.0, detune=0.001), t0 + 3.5 * BEAT,
                gain=0.06 * MUS, pan=-0.2 + 0.2 * i, send=0.4)

    add(bass(midi_hz(root - 12)), t0 + 0.0 * BEAT, gain=0.30 * MUS)
    add(bass(midi_hz(root - 12)), t0 + 1.0 * BEAT, gain=0.24 * MUS)
    add(bass(midi_hz(root - 12), 0.35), t0 + 1.5 * BEAT, gain=0.14 * MUS)
    add(bass(midi_hz(root - 12 + 7), 0.4), t0 + 2.0 * BEAT, gain=0.20 * MUS)
    add(bass(midi_hz(root - 12)), t0 + 3.0 * BEAT, gain=0.22 * MUS)

    for beat_at, g in ((0, 1.0), (1, 0.62), (2, 0.92), (3, 0.62)):
        add(kick(1.0), t0 + beat_at * BEAT, gain=g * 0.5)

    add(stamp(0.5, 1.0), t0 + 1 * BEAT, gain=0.10 * MUS, pan=0.15, send=0.3)
    add(stamp(0.5, 1.0), t0 + 3 * BEAT, gain=0.10 * MUS, pan=-0.15, send=0.3)

    for i in range(8):
        swing = 0.06 * BEAT if i % 2 else 0.0
        add(typekey(0.35 if i % 2 else 0.5), t0 + i * BEAT / 2 + swing,
            gain=0.055 * MUS, pan=0.25, send=0.15)

# --- events ----------------------------------------------------------------
for e in TL["events"]:
    kind = e["kind"]
    t = e["t"]
    pan = e.get("pan", 0.0)
    gain = e.get("gain", 0.2)

    if kind == "slam":
        add(slam(e.get("w", 0.5)), t, gain=gain, pan=pan, send=0.3)
    elif kind == "sweep":
        sig = sweep(e.get("dur", 0.3), e.get("lo", 400), e.get("hi", 2600), e.get("up", True))
        add(sig, t, gain=gain, pan=pan, send=0.3)
    elif kind == "stamp":
        add(stamp(e.get("scale", 1.0), e.get("pitch", 1.0)), t, gain=gain, pan=pan, send=0.35)
    elif kind == "kick":
        add(kick(1.0), t, gain=gain, pan=pan, send=0.15)
    elif kind == "typekey":
        add(typekey(e.get("level", 1.0)), t, gain=gain, pan=pan, send=0.12)
    elif kind == "printer":
        add(printer(e.get("dur", 0.12)), t, gain=gain, pan=pan, send=0.2)
    elif kind == "tear":
        add(tear(), t, gain=gain, pan=pan, send=0.3)
    elif kind == "bell":
        freq = e.get("freq") or midi_hz(e.get("midi", 69))
        add(bell(freq, e.get("decay", 0.4), e.get("ratio", 1.0), e.get("index", 0.6)),
            t, gain=gain, pan=pan, send=0.45)
    elif kind == "bloop":
        add(bloop(e.get("f0", 900), e.get("f1", 1400), e.get("dur", 0.05)),
            t, gain=gain, pan=pan, send=0.2)
    elif kind == "riser":
        add(riser(e.get("dur", 1.25)), t, gain=0.18, pan=pan, send=0.4)
    elif kind == "click":
        add(click(1.0), t, gain=gain, pan=pan, send=0.1)
    elif kind == "redact":
        add(sweep(0.22, 900, 300, False), t, gain=gain, pan=pan, send=0.25)

# --- reverb ----------------------------------------------------------------
ir_len = int(1.3 * SR)
ir_t = np.arange(ir_len) / SR
ir = RNG.standard_normal(ir_len).astype(np.float32) * np.exp(-ir_t / 0.32)
ir = fft_filter(ir, None, 6000)
ir /= np.sqrt(np.sum(ir ** 2))
size = N + ir_len
IR = np.fft.rfft(ir, size)
for ch in range(2):
    wet = np.fft.irfft(np.fft.rfft(SEND[:, ch], size) * IR, size)[:N]
    DRY[:, ch] += wet.astype(np.float32) * 0.9

# --- master ----------------------------------------------------------------
mix = DRY
fade = np.ones(N, dtype=np.float32)
fade[-int(1.0 * SR):] = np.linspace(1, 0, int(1.0 * SR))
mix *= fade[:, None]
peak = np.max(np.abs(mix))
target = 10 ** (-6 / 20)
mix *= target / max(peak, 1e-9)

out = ROOT / "audio" / "score.wav"
out.parent.mkdir(exist_ok=True)
import wave

pcm = (np.clip(mix, -1, 1) * 32767).astype("<i2")
with wave.open(str(out), "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(pcm.tobytes())

rms = float(np.sqrt(np.mean(mix ** 2)))
print(f"{out.name}  {DUR:.0f}s  peak {-6:.1f} dBFS  rms {20 * np.log10(rms + 1e-9):.1f} dBFS  "
      f"{out.stat().st_size // 1024} KB  events {len(TL['events'])}")
