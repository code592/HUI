#!/usr/bin/env python3
"""Reassemble in-process rendering artifacts, without Android screen capture."""
import base64
from collections import defaultdict
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
parts = defaultdict(list)
for line in (root / ".build/android-smoke.log").read_text().splitlines():
    match = re.fullmatch(
        r"INSTRUMENTATION_STATUS: image-(snapshot-[a-z-]+)-(\d{4})=(.*)", line
    )
    if match:
        name, index, data = match.groups()
        parts[name].append((index, data))
output = root / ".build/android-touch-images"
output.mkdir(parents=True, exist_ok=True)
for name, chunks in parts.items():
    (output / f"{name}.png").write_bytes(
        base64.b64decode("".join(data for _, data in sorted(chunks)), validate=True)
    )
print(f"Saved {len(parts)} app-rendered selection test images to {output}")
