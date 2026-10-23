#!/usr/bin/env python3
"""Check native ABI, ELF page alignment and offline assets in a packaged APK."""
import pathlib
import struct
import sys
import zipfile

path = pathlib.Path(sys.argv[1])
with zipfile.ZipFile(path) as apk:
    names = apk.namelist()
    libraries = [n for n in names if n.startswith('lib/') and n.endswith('.so')]
    assert 'lib/arm64-v8a/libhui_app.so' in libraries, 'Missing HUI native entry point'
    for name in libraries:
        assert name.startswith('lib/arm64-v8a/'), f'Unexpected ABI: {name}'
        info = apk.getinfo(name)
        if info.compress_type == zipfile.ZIP_STORED:
            with path.open('rb') as source:
                source.seek(info.header_offset + 26)
                filename_len, extra_len = struct.unpack('<HH', source.read(4))
            assert (info.header_offset + 30 + filename_len + extra_len) % 16384 == 0, f'APK library not 16 KiB aligned: {name}'
        data = apk.read(name)
        assert data[:6] == b'\x7fELF\x02\x01', f'Expected little-endian ELF64: {name}'
        assert struct.unpack_from('<H', data, 18)[0] == 183, f'Expected AArch64: {name}'
        phoff = struct.unpack_from('<Q', data, 32)[0]
        entsize, count = struct.unpack_from('<HH', data, 54)
        for i in range(count):
            header = struct.unpack_from('<IIQQQQQQ', data, phoff + i * entsize)
            if header[0] == 1:
                assert header[7] >= 16384, f'ELF LOAD segment not 16 KiB aligned: {name}'
    for required in ['AndroidManifest.xml', 'classes.dex', 'assets/katex/katex.min.css',
                     'assets/licenses/LICENSE', 'assets/licenses/Slint-LICENSE.md',
                     'assets/licenses/MathJax-LICENSE', 'assets/licenses/THIRD_PARTY.md']:
        assert required in names, f'Missing {required}'
    assert len([n for n in names if n.startswith('assets/katex/fonts/')]) >= 50, 'Missing math fonts'
    assert b'ic_launcher' in apk.read('resources.arsc'), 'Missing launcher logo resource'
print(f'PASS {path}: ARM64 native library, 16 KiB ELF/APK alignment, logo, fonts and licenses')
