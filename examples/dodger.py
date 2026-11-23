#!/usr/bin/env python3
"""Dodger: an Icewire bot in Python.

In a race it heads for the next checkpoint and lets its nose ranger set
the pace. In a battle it finds the other rover by radar, keeps three to
five metres off, weaves from side to side so pellets miss, fires when it
is aimed, lobs slush when it is far, and backs away when its charge runs
low.

Try it:   icewire --tournament python3 examples/dodger.py kettle
The lines it reads and writes are in PROTOCOL.md. Anything printed to
standard error shows up beside the arena, which is handy for debugging.
"""

import sys

PROTOCOL = "icewire-bot 1"


def pairs(words):
    """'heading 90 bearing -12.5' as {'heading': 90.0, 'bearing': -12.5}."""
    return {words[i]: float(words[i + 1]) for i in range(0, len(words) - 1, 2)}


def clamp(x, lo=-1.0, hi=1.0):
    return max(lo, min(hi, x))


def think(r):
    """One tick: the readings in, the commands out."""
    time = float(r["time"][0])
    ranger = pairs(r["ranger"])
    compass = pairs(r["compass"])
    radar = pairs(r["radar"])
    charge = float(r["charge"][0])
    loaded = pairs(r["loaded"])

    if radar["distance"] >= 99:
        # a race or a maze: nobody else about
        bearing = compass["bearing"]
        speed = clamp(ranger["nose"] / 3, 0.3, 1.0)
        return [f"gyro {clamp(bearing / 30):.3f}", f"tracks {speed:.3f} {speed:.3f}"]

    bearing = radar["bearing"]
    distance = radar["distance"]
    out = []

    # weave: lean one way, then the other, every second and a half
    weave = 0.0
    if 2.5 < distance < 7 and charge > 30:
        weave = 0.35 if int(time / 1.5) % 2 == 0 else -0.35
    out.append(f"gyro {clamp(bearing / 25 + weave):.3f}")

    if charge < 30:
        speed = -0.7
    elif distance > 5:
        speed = 0.8
    elif distance < 3:
        speed = -0.5
    else:
        speed = 0.3
    if ranger["nose"] < 0.5 and speed > 0:
        speed = -0.4
    out.append(f"tracks {speed:.3f} {speed:.3f}")

    if abs(bearing) < 8 and loaded["pellet"]:
        out.append("fire")
    if distance > 3 and abs(bearing) < 12 and loaded["lobber"]:
        out.append(f"lob {distance:.2f}")
    return out


def main():
    hello = sys.stdin.readline().strip()
    if hello != PROTOCOL:
        print(f"Dodger speaks {PROTOCOL}, not {hello!r}", file=sys.stderr)
        return
    opening = {}
    for line in sys.stdin:
        words = line.split()
        if words == ["end"]:
            break
        if words:
            opening[words[0]] = " ".join(words[1:])
    print(PROTOCOL, flush=True)
    print(f"Dodger ready for {opening.get('challenge', 'anything')}", file=sys.stderr, flush=True)

    readings = {}
    for line in sys.stdin:
        words = line.split()
        if not words:
            continue
        if words[0] != "end":
            readings[words[0]] = words[1:]
            continue
        print("\n".join(think(readings) + ["end"]), flush=True)
        readings = {}


if __name__ == "__main__":
    main()
