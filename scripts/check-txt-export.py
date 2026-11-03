#!/usr/bin/env python3
"""Exercise a packaged executable's full-document UTF-8 TXT export."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

app = Path(sys.argv[1]).resolve(strict=True)
tokens = [
    "简体中文", "繁體中文", "日本語", "한국어", "English", "Français",
    "Deutsch", "Русский", "Español", "👩‍💻", "cafe\u0301",
]
paragraph = "Paragraph with Unicode 中文 and emoji 👋🏽.\n\n"
source = (
    "# TXT export\n\n**Hello** [link](https://example.com) "
    "![image description](missing.png)\n\n"
    + " ".join(tokens)
    + '\n\n```rust\nlet value = 42; // keep code\n```\n\n'
    + '```mermaid\nunsupported diagram kept as plain source\n```\n\n'
    + '<div>HTML &amp; <b>plain text</b></div>\n\n'
    + paragraph * 300
    + 'LAST_DOCUMENT_MARKER\n'
)
assert len(source) > 8192
with tempfile.TemporaryDirectory(prefix="hui-txt-") as directory:
    work = Path(directory)
    document = work / "multilingual document.md"
    document.write_text(source, encoding="utf-8")
    env = os.environ.copy()
    env.pop("HUI_ASSETS", None)
    env["HUI_DATA_DIR"] = str(work / "profile")
    outputs = []
    for name in ("plain text.txt", "uppercase.TXT"):
        output = work / name
        subprocess.run(
            [str(app), "--render", str(document), str(output)],
            cwd=work, env=env, check=True, timeout=120,
        )
        outputs.append(output.read_bytes())
    assert outputs[0] == outputs[1], "TXT extension case changed the output"
    text = outputs[0].decode("utf-8")
    assert text.startswith("TXT export\n\nHello link image description\n")
    assert text.count("Paragraph with Unicode") == 300
    assert text.rstrip().endswith("LAST_DOCUMENT_MARKER")
    for token in tokens + ["let value = 42; // keep code",
                            "unsupported diagram kept as plain source",
                            "HTML & plain text"]:
        assert token in text, repr(token)
    assert "**Hello**" not in text and "```" not in text
print("PASS TXT: full document, nine languages, emoji, code, HTML text, "
      "uppercase suffix and paths with spaces")
