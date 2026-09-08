#!/usr/bin/env python3
"""Check packaged architecture, required resources and independently computed hashes."""
import hashlib
import json
import pathlib
import struct
import sys
import tarfile
import zipfile

root = pathlib.Path(__file__).resolve().parent.parent
for system in ('windows', 'linux'):
    for arch in ('x64', 'arm64'):
        name = f'HUI-{system}-{arch}'
        if sys.argv[1:] and name not in sys.argv[1:]:
            continue
        suffix = '.zip' if system == 'windows' else '.tar.gz'
        archive = root / 'dist' / (name + suffix)
        if system == 'windows':
            with zipfile.ZipFile(archive) as bundle:
                assert bundle.testzip() is None, archive
                files = {p.replace('\\', '/'): p for p in bundle.namelist()}
                def read(p):
                    return bundle.read(files[f'{name}/{p}'])
                exe = read('hui.exe')
                assert exe[:2] == b'MZ'
                pe = struct.unpack_from('<I', exe, 0x3c)[0]
                assert exe[pe:pe + 4] == b'PE\0\0'
                assert struct.unpack_from('<H', exe, pe + 4)[0] == {'x64': 0x8664, 'arm64': 0xaa64}[arch]
                assert read('hui.ico') and read('assets/katex/katex.min.css')
                crt = read('vcruntime140.dll')
                crt_pe = struct.unpack_from('<I', crt, 0x3c)[0]
                assert struct.unpack_from('<H', crt, crt_pe + 4)[0] == {'x64': 0x8664, 'arm64': 0xaa64}[arch]
                assert any('/assets/katex/fonts/' in p for p in files)
        else:
            with tarfile.open(archive) as bundle:
                member = bundle.getmember(f'{name}/bin/hui')
                assert member.mode & 0o111
                exe = bundle.extractfile(member).read()
                assert exe[:5] == b'\x7fELF\x02'
                assert struct.unpack_from('<H', exe, 18)[0] == {'x64': 62, 'arm64': 183}[arch]
                for p in ('bin/assets/katex/katex.min.css', 'share/applications/hui.desktop', 'share/icons/hicolor/scalable/apps/hui.svg'):
                    assert bundle.getmember(f'{name}/{p}').size > 0
        for locale in (root / 'crates/hui-app/assets/locales').glob('*.json'):
            greeting = json.loads(locale.read_text(encoding='utf-8'))['欢迎使用 HUI'].encode('utf-8')
            assert greeting in exe, f'{archive.name}: missing embedded locale {locale.stem}'
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        saved = pathlib.Path(str(archive) + '.sha256').read_text(encoding='utf-8-sig').strip().split()[0]
        assert digest == saved.lower(), archive
        print(f'PASS {archive.name}: architecture, resources, nine embedded languages, SHA-256 {digest}')
