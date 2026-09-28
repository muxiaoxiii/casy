"""Report exactly which imported DLL/symbol fails before a Rust test can start."""
import ctypes
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.SetErrorMode(0x0001 | 0x0002 | 0x8000)
kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint]
kernel.LoadLibraryExW.restype = ctypes.c_void_p
kernel.GetProcAddress.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
kernel.GetProcAddress.restype = ctypes.c_void_p
kernel.GetModuleFileNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint]

def imports(path):
    data = path.read_bytes()
    pe = struct.unpack_from('<I', data, 60)[0]
    count = struct.unpack_from('<H', data, pe + 6)[0]
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    optional = pe + 24
    wide = struct.unpack_from('<H', data, optional)[0] == 0x20b
    sections = optional + optional_size
    def offset(rva):
        for i in range(count):
            size, address, raw_size, raw = struct.unpack_from('<IIII', data, sections + 40 * i + 8)
            if address <= rva < address + max(size, raw_size):
                return raw + rva - address
        return rva
    def string(pos):
        return data[pos:data.index(b'\0', pos)].decode('ascii')
    rva = struct.unpack_from('<I', data, optional + (120 if wide else 104))[0]
    if not rva:
        return []
    pos = offset(rva)
    result = []
    while any(data[pos:pos + 20]):
        original, _, _, name, first = struct.unpack_from('<IIIII', data, pos)
        thunk = offset(original or first)
        symbols = []
        while True:
            value = struct.unpack_from('<Q' if wide else '<I', data, thunk)[0]
            if not value:
                break
            if value & (1 << (63 if wide else 31)):
                symbols.append(value & 0xffff)
            else:
                symbols.append(string(offset(value) + 2))
            thunk += 8 if wide else 4
        result.append((string(offset(name)), symbols))
        pos += 20
    return result

root = Path(__file__).resolve().parent.parent
binary = next((root / 'src-tauri/target/debug/deps').glob('casy_lib-*.exe'))
sdk = root / 'src-tauri/runtime/zvec'
paths = [binary.parent, Path(os.environ['SystemRoot']) / 'System32', Path(os.environ['SystemRoot']), root, sdk]
paths += [Path(value) for value in os.environ.get('PATH', '').split(';') if value]
report = []
visited = set()
def inspect(path):
    if path.name.lower() in visited:
        return
    visited.add(path.name.lower())
    for name, symbols in imports(path):
        resolved = next((directory / name for directory in paths if (directory / name).is_file()), None)
        handle = kernel.LoadLibraryExW(str(resolved or name), None, 0x8 if resolved else 0)
        record = {'from': path.name, 'dll': name, 'path': str(resolved), 'loadError': ctypes.get_last_error() if not handle else None}
        if handle:
            actual = ctypes.create_unicode_buffer(32768)
            kernel.GetModuleFileNameW(handle, actual, len(actual))
            record['loadedPath'] = actual.value
            missing = []
            for symbol in symbols:
                argument = ctypes.cast(ctypes.c_void_p(symbol), ctypes.c_char_p) if isinstance(symbol, int) else symbol.encode('ascii')
                if not kernel.GetProcAddress(handle, argument):
                    missing.append(symbol)
            record['missingSymbols'] = missing
        report.append(record)
        # Recurse through application dependencies, not the Windows API graph.
        if resolved and str(resolved).lower().startswith(str(root).lower()):
            inspect(resolved)
inspect(binary)
output = root / 'outputs/release-loader'
output.mkdir(parents=True, exist_ok=True)
(output / 'imports.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
for item in report:
    if item.get('loadError') or item.get('missingSymbols'):
        print(json.dumps(item))
for label, path in [('inherited', os.environ['PATH']), ('isolated', ';'.join(map(str, [binary.parent, sdk, Path(os.environ['SystemRoot']) / 'System32', Path(os.environ['SystemRoot'])])))]:
    result = subprocess.run([str(binary), '--list'], env={**os.environ, 'PATH': path}, capture_output=True, text=True, timeout=90)
    print(f'{label} startup: {result.returncode:#x}\n{result.stderr}')
    (output / f'{label}.txt').write_text(f'Exit: {result.returncode:#x}\n{result.stdout}\n{result.stderr}', encoding='utf-8')
