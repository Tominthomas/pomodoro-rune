#!/usr/bin/env python3
"""Makes the two bells in assets/sounds/ from sine partials, so the sound is the app's own. The
sunset bell rings when a focus block ends: low and soft. The rested bell rings when a break ends:
lighter and higher. Each is struck once and dies away in about 1.5 s. It writes a WAV with the
standard library, encodes it as AAC with macOS's afconvert, and prints each bell's length and peak.

    python3 tools/bell.py
"""

import math
import struct
import subprocess
import sys
import tempfile
import wave
from pathlib import Path
from typing import NamedTuple

OUT = Path(__file__).resolve().parent.parent / "assets/sounds"
RATE = 44_100
LENGTH = 1.6
RELEASE = 0.3
ATTACK = 0.012


class Partial(NamedTuple):
    ratio: float
    level: float
    decay: float


# A bell's upper partials are not whole multiples and die first, and the two close fundamentals
# beat slowly: that is what makes it sound struck rather than synthesized.
BELLS = {
    "sunset": {
        "hz": 220.0,
        "peak_db": -10.0,
        "partials": [
            Partial(1.0, 1.0, 0.55),
            Partial(1.004, 0.5, 0.5),
            Partial(2.0, 0.45, 0.35),
            Partial(2.76, 0.12, 0.2),
            Partial(5.4, 0.03, 0.08),
        ],
    },
    "rested": {
        "hz": 659.26,
        "peak_db": -14.0,
        "partials": [
            Partial(1.0, 1.0, 0.4),
            Partial(1.003, 0.4, 0.35),
            Partial(2.0, 0.25, 0.22),
            Partial(2.76, 0.06, 0.12),
        ],
    },
}


def attack(t):
    """A soft mallet: the note swells in instead of starting on a hard edge."""
    return 0.5 - 0.5 * math.cos(math.pi * min(t / ATTACK, 1.0))


def release(t):
    """The last part fades to silence, so the bell ends without a click wherever the decay is."""
    return 0.5 + 0.5 * math.cos(math.pi * max(0.0, (t - (LENGTH - RELEASE)) / RELEASE))


def samples(hz, partials, peak_db):
    """The bell as floats in -1..1, scaled so its loudest sample is at `peak_db` dBFS."""
    n = int(LENGTH * RATE)
    out = []
    for i in range(n):
        t = i / RATE
        v = sum(p.level * math.exp(-t / p.decay) * math.sin(2 * math.pi * hz * p.ratio * t) for p in partials)
        out.append(v * attack(t) * release(t))
    scale = 10 ** (peak_db / 20) / max(abs(v) for v in out)
    return [v * scale for v in out]


def write_wav(path, values):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", round(v * 32767)) for v in values))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for name, bell in BELLS.items():
            values = samples(bell["hz"], bell["partials"], bell["peak_db"])
            wav = Path(tmp) / f"{name}.wav"
            write_wav(wav, values)
            m4a = OUT / f"{name}.m4a"
            subprocess.run(["afconvert", "-f", "m4af", "-d", "aac", "-b", "96000", str(wav), str(m4a)], check=True)
            peak = 20 * math.log10(max(abs(v) for v in values))
            print(f"{m4a.relative_to(OUT.parent.parent)}: {len(values) / RATE:.2f} s, peak {peak:.1f} dBFS")


if __name__ == "__main__":
    sys.exit(main())
