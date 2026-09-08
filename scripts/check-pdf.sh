#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v pdftotext >/dev/null
mkdir -p .build/pdf-check
python3 - <<'PY'
from pathlib import Path
text = '\n\n'.join(f'## SECTION_{i:03d}\n\nBEGIN_{i:03d} ' + 'A sentence with a few words. ' * 12 + f' END_{i:03d}' for i in range(60))
Path('.build/pdf-check/input.md').write_text(text)
PY
"${HUI_BIN:-target/release/hui}" --render .build/pdf-check/input.md .build/pdf-check/output.pdf
pdftotext .build/pdf-check/output.pdf .build/pdf-check/output.txt
python3 - <<'PY'
from pathlib import Path
text = Path('.build/pdf-check/output.txt').read_text()
for i in range(60):
    for key in (f'SECTION_{i:03d}', f'BEGIN_{i:03d}', f'END_{i:03d}'):
        assert text.count(key) == 1, (key, text.count(key))
print('PASS: 180 PDF text sentinels present exactly once across all pages')
PY
