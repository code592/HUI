#!/usr/bin/env python3
"""Validate bundled UI catalogs and their named placeholders without build tooling."""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parents[1]
locales = root / "crates/hui-app/assets/locales"
base = json.loads((locales / "zh-Hans.json").read_text(encoding="utf-8"))
for path in sorted(locales.glob("*.json")):
    catalog = json.loads(path.read_text(encoding="utf-8"))
    assert catalog.keys() == base.keys(), f"Missing/extra keys: {path.name}"
    for key, value in catalog.items():
        assert value.strip(), f"Empty translation: {path.name}: {key}"
        assert sorted(re.findall(r"\{[^}]+\}", key)) == sorted(re.findall(r"\{[^}]+\}", value)), f"Placeholders: {path.name}: {key}"
    assert (locales / f"welcome-{path.stem}.md").exists()
ui = (root / "crates/hui-app/ui/app.slint").read_text(encoding="utf-8")
for key in re.findall(r'I18n.translate\("([^"\n]*)", I18n.locale\)', ui):
    assert key in base, f"Missing UI key: {key}"
assert len(list(locales.glob("*.json"))) == 9
print(f"PASS: 9 locale catalogs, {len(base)} keys each, placeholders and welcome documents")
