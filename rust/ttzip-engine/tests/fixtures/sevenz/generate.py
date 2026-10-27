#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause
"""Create and verify owned 7z fixtures. CLI is never needed by the Rust tests."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parent
METHODS = {'Copy': 0, 'Delta': 3, 'ARM64': 10, 'LZMA2': 0x21,
           'LZMA': 0x030101, 'BCJ': 0x03030103, 'BCJ2': 0x0303011B,
           'PPMD': 0x030401, 'BZip2': 0x040202, 'LZ4': 0x04F71104,
           'BROTLI': 0x04F71102, '7zAES': 0x06F10701}
# LZ4MT container used by the p7zip 7z LZ4 coder: 12-byte header (magic, version,
# frame size) followed by one LZ4 frame. The magic is stored little-endian.
LZ4MT_SKIPPABLE_MAGIC = 0x184D2A50
LZ4_FRAME_MAGIC = 0x184D2204
BCJ2 = ['-m0=BCJ2', '-m1=LZMA2', '-m2=LZMA2', '-m3=LZMA2',
        '-mb0:1', '-mb0s1:2', '-mb0s2:3']
DELTA_BCJ2 = ['-m0=Delta', '-m1=BCJ2', '-m2=LZMA2', '-m3=LZMA2', '-m4=LZMA2',
              '-mb0:1', '-mb1:2', '-mb1s1:3', '-mb1s2:4']


def pattern(length, seed=7):
    return bytes((i * 29 + seed) % 251 for i in range(length))


def branch(length, arm=False):
    unit = bytes.fromhex('0100009400000014') if arm else bytes.fromhex('90e81000000090e920000000')
    prefix = b'MZ\x00\x00' if arm else b'MZ'
    return (prefix + unit * ((length + len(unit)) // len(unit)))[:length]


def delta_branch_source(length):
    # After Delta distance=1 encoding this becomes a branch-rich x86 stream.
    output = bytearray(); previous = 0
    for byte in branch(length):
        previous = (previous + byte) & 255; output.append(previous)
    return bytes(output)


def prose(length):
    unit = b'Owned TTZip codec compliance fixture. No third-party source or license text.\n'
    return (unit * ((length + len(unit) - 1) // len(unit)))[:length]


def specs():
    # archive, owned file map, CLI inputs, codec switches, expected methods, tool, password
    rows = [
        ('single_empty_file.7z', {'empty.txt': b''}, ['empty.txt'], ['-m0=LZMA'], [], '7zz', None),
        ('two_empty_file.7z', {'a.txt': b'', 'b.txt': b''}, ['a.txt', 'b.txt'], ['-m0=LZMA'], [], '7zz', None),
        ('single_file_with_content_lzma.7z', {'file.txt': b'this is a file\n'}, ['file.txt'], ['-m0=LZMA'], ['LZMA'], '7zz', None),
        ('two_files_with_content_lzma.7z', {'file1.txt': b'file one content\n', 'file2.txt': b'file two content\n'}, ['file1.txt', 'file2.txt'], ['-m0=LZMA'], ['LZMA'], '7zz', None),
        ('solid.7z', {'empty.txt': b'', 'test1.txt': pattern(3447), 'test2.txt': pattern(2654, 11)}, ['empty.txt', 'test1.txt', 'test2.txt'], ['-m0=LZMA2', '-ms=on'], ['LZMA2'], '7zz', None),
        ('non_solid.7z', {'empty.txt': b'', 'test1.txt': pattern(3447), 'test2.txt': pattern(2654, 11)}, ['empty.txt', 'test1.txt', 'test2.txt'], ['-m0=LZMA2', '-ms=off'], ['LZMA2'], '7zz', None),
        ('copy.7z', {'copy.txt': b'simple copy encoding'}, ['copy.txt'], ['-m0=Copy'], ['Copy'], '7zz', None),
        ('delta.7z', {'delta.txt': b'aaaabbbbcccc'}, ['delta.txt'], ['-m0=Delta', '-m1=LZMA2'], ['Delta', 'LZMA2'], '7zz', None),
        ('delta_bcj2.7z', {'code1.bin': delta_branch_source(9000), 'code2.bin': delta_branch_source(7000), 'wave.bin': pattern(16000)}, ['code1.bin', 'code2.bin', 'wave.bin'], DELTA_BCJ2, ['Delta', 'BCJ2', 'LZMA2'], '7zz', None),
        ('bcj2_x86.7z', {'synthetic/bin/branch.bin': branch(462336), 'synthetic/a.txt': b'a', 'synthetic/b.txt': b'b', 'synthetic/c.txt': b'c'}, ['synthetic'], BCJ2, ['BCJ2', 'LZMA2'], '7zz', None),
        ('bcj_arm64.7z', {'branch.bin': branch(357888, True)}, ['branch.bin'], ['-m0=ARM64', '-m1=LZMA2'], ['ARM64', 'LZMA2'], '7zz', None),
        ('bcj_x86.7z', {'branch.bin': branch(367104)}, ['branch.bin'], ['-m0=BCJ', '-m1=LZMA2'], ['BCJ', 'LZMA2'], '7zz', None),
        ('ppmd.7z', {'owned.txt': prose(11356)}, ['owned.txt'], ['-m0=PPMD'], ['PPMD'], '7zz', None),
        ('bzip2_file.7z', {'hello.txt': b'world\n', 'foo.txt': b'bar\n'}, ['hello.txt', 'foo.txt'], ['-m0=BZip2'], ['BZip2'], '7zz', None),
        # LZ4 needs -mmt>1: the upstream LZ4MT encoder only emits its 12-byte
        # container header when the codec runs multi-threaded, so a single
        # threaded run emits 12 stray bytes and the archive fails its own test.
        ('lz4.7z', {'owned.txt': prose(11556)}, ['owned.txt'], ['-m0=LZ4', '-mmt=2'], ['LZ4'], '7z', None),
        ('brotli.7z', {'owned.txt': prose(11556)}, ['owned.txt'], ['-m0=BROTLI'], ['BROTLI'], '7z', None),
        ('aes_data.7z', {'owned.bin': pattern(129421)}, ['owned.bin'], ['-m0=LZMA2', '-mhe=off'], ['LZMA2', '7zAES'], '7zz', 'owned-data-password'),
        ('aes_header.7z', {'docs/message.txt': pattern(2247), 'docs/pattern.bin': pattern(1417, 11)}, ['docs'], ['-m0=LZMA2', '-mhe=on'], ['LZMA2', '7zAES'], '7zz', 'owned-header-password'),
    ]
    return rows


def run(args, cwd=None, ok=True):
    proc = subprocess.run(args, cwd=cwd, capture_output=True)
    if ok and proc.returncode:
        raise RuntimeError(f'{args}: {proc.stderr.decode(errors="replace")} {proc.stdout.decode(errors="replace")}')
    return proc


def num(data, cursor):
    first = data[cursor]; cursor += 1
    value = 0; mask = 0x80
    for i in range(8):
        if not first & mask:
            return value | ((first & (mask - 1)) << (8 * i)), cursor
        value |= data[cursor] << (8 * i); cursor += 1; mask >>= 1
    return value, cursor


def digest(data):
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
            'crc32': zlib.crc32(data)}


def folder_coders(header):
    # Parse the uncompressed main/encoded-header StreamsInfo until UnpackInfo.
    cursor = 1
    if header[0] == 1:
        if header[cursor] == 5:
            return []
        assert header[cursor] == 4; cursor += 1
    elif header[0] != 0x17:
        raise AssertionError('expected Header or EncodedHeader')
    if header[cursor] == 6:
        cursor += 1
        _, cursor = num(header, cursor)
        count, cursor = num(header, cursor)
        assert header[cursor] == 9; cursor += 1
        for _ in range(count): _, cursor = num(header, cursor)
        if header[cursor] == 10:
            cursor += 1
            assert header[cursor] == 1; cursor += 1 + 4 * count
        assert header[cursor] == 0; cursor += 1
    if header[cursor] == 0:
        return []
    assert header[cursor] == 7; cursor += 1
    assert header[cursor] == 11; cursor += 1
    count, cursor = num(header, cursor)
    assert header[cursor] == 0; cursor += 1
    folders = []
    for _ in range(count):
        coder_count, cursor = num(header, cursor)
        coders = []; total_in = 0; total_out = 0
        for _ in range(coder_count):
            flags = header[cursor]; cursor += 1
            width = flags & 15
            method = int.from_bytes(header[cursor:cursor + width], 'big'); cursor += width
            ins = outs = 1
            if flags & 0x10:
                ins, cursor = num(header, cursor); outs, cursor = num(header, cursor)
            props = b''
            if flags & 0x20:
                size, cursor = num(header, cursor)
                props = header[cursor:cursor + size]; cursor += size
            coders.append({'id': f'0x{method:08x}', 'inputs': ins, 'outputs': outs, 'properties': props.hex()})
            total_in += ins; total_out += outs
        pairs = []
        for _ in range(total_out - 1):
            src, cursor = num(header, cursor); dst, cursor = num(header, cursor); pairs.append([src, dst])
        if total_in - (total_out - 1) > 1:
            for _ in range(total_in - total_out + 1): _, cursor = num(header, cursor)
        folders.append({'coders': coders, 'bind_pairs': pairs})
    return folders


def inspect_archive(path):
    data = path.read_bytes()
    assert data[:6] == b'7z\xbc\xaf\x27\x1c'
    assert zlib.crc32(data[12:32]) == int.from_bytes(data[8:12], 'little')
    offset, size, crc = struct.unpack('<QQI', data[12:32])
    header = data[32 + offset:32 + offset + size]
    assert len(header) == size and zlib.crc32(header) == crc
    return folder_coders(header)


def pack_info(header):
    # PackInfo of an uncompressed main header only; encoded headers are skipped
    # so an outer (encrypted) pack stream is never mistaken for the data layer.
    cursor = 1
    if header[0] == 1:
        if header[cursor] == 5:
            return None
        assert header[cursor] == 4
        cursor += 1
    elif header[0] == 0x17:
        return None
    if header[cursor] != 6:
        return None
    cursor += 1
    pack_pos, cursor = num(header, cursor)
    count, cursor = num(header, cursor)
    assert header[cursor] == 9
    cursor += 1
    sizes = []
    for _ in range(count):
        size, cursor = num(header, cursor)
        sizes.append(size)
    return pack_pos, sizes


def archive_pack_payload(path):
    data = path.read_bytes()
    assert data[:6] == b'7z\xbc\xaf\x27\x1c'
    offset, size, crc = struct.unpack('<QQI', data[12:32])
    header = data[32 + offset:32 + offset + size]
    assert len(header) == size and zlib.crc32(header) == crc
    info = pack_info(header)
    assert info is not None, 'archive carries no main-streams pack info'
    pack_pos, sizes = info
    start = 32 + pack_pos
    return data[start:start + sum(sizes)]


def lz4mt_frames(payload):
    frames = []
    cursor = 0
    while cursor < len(payload):
        magic = int.from_bytes(payload[cursor:cursor + 4], 'little')
        assert magic == LZ4MT_SKIPPABLE_MAGIC, f'unexpected LZ4MT magic 0x{magic:08x}'
        version = int.from_bytes(payload[cursor + 4:cursor + 8], 'little')
        size = int.from_bytes(payload[cursor + 8:cursor + 12], 'little')
        frames.append({'version': version, 'frame_size': size,
                       'frame': payload[cursor + 12:cursor + 12 + size]})
        cursor += 12 + size
    assert cursor == len(payload), 'trailing bytes after the LZ4MT container'
    return frames


def lz4_block_decode(block, history):
    # Independent LZ4 block decoder (spec: token, literal run, offset, match run).
    out = bytearray(history)
    base = len(out)
    cursor = 0
    while cursor < len(block):
        token = block[cursor]
        cursor += 1
        literal_len = token >> 4
        if literal_len == 15:
            while True:
                extra = block[cursor]
                cursor += 1
                literal_len += extra
                if extra != 255:
                    break
        out += block[cursor:cursor + literal_len]
        cursor += literal_len
        if cursor >= len(block):
            break
        offset = int.from_bytes(block[cursor:cursor + 2], 'little')
        cursor += 2
        match_len = token & 0x0F
        if match_len == 15:
            while True:
                extra = block[cursor]
                cursor += 1
                match_len += extra
                if extra != 255:
                    break
        match_len += 4
        assert 0 < offset <= len(out), f'invalid LZ4 match offset {offset}'
        for _ in range(match_len):
            out.append(out[len(out) - offset])
    return bytes(out[base:])


def lz4_frame_decode(frame):
    # Independent LZ4 frame reader: descriptor, block loop, 64KB linked window.
    assert int.from_bytes(frame[:4], 'little') == LZ4_FRAME_MAGIC, 'not an LZ4 frame'
    flg = frame[4]
    cursor = 6
    content_size = None
    if (flg >> 3) & 1:
        content_size = int.from_bytes(frame[cursor:cursor + 8], 'little')
        cursor += 8
    if flg & 1:
        cursor += 4
    cursor += 1
    history = b''
    plain = bytearray()
    while True:
        header = int.from_bytes(frame[cursor:cursor + 4], 'little')
        cursor += 4
        if header == 0:
            break
        size = header & 0x7FFFFFFF
        raw = frame[cursor:cursor + size]
        cursor += size
        if (flg >> 4) & 1:
            cursor += 4
        if header & 0x80000000:
            decoded = raw
        else:
            decoded = lz4_block_decode(raw, history)
        plain += decoded
        history = (history + decoded)[-65536:]
    if content_size is not None:
        assert len(plain) == content_size, (len(plain), content_size)
    return bytes(plain)


def lz4_cli_decode(frame, expected):
    # Reference LZ4 implementation: also verifies the frame content checksum.
    tool = shutil.which('lz4')
    if not tool:
        raise RuntimeError('lz4 CLI required for independent LZ4 frame verification')
    with tempfile.TemporaryDirectory(prefix='ttzip-lz4-verify-') as tmp:
        source = Path(tmp) / 'frame.lz4'
        target = Path(tmp) / 'frame.out'
        source.write_bytes(frame)
        run([tool, '-d', '-f', str(source), str(target)])
        assert target.read_bytes() == expected, 'lz4 CLI decoded different bytes'


def verify_lz4_payload(path, files):
    payload = archive_pack_payload(path)
    frames = lz4mt_frames(payload)
    assert len(frames) == 1, f'expected one LZ4MT container, got {len(frames)}'
    frame = frames[0]['frame']
    plain = lz4_frame_decode(frame)
    expected = files['owned.txt']
    assert plain == expected, 'independent LZ4 frame decode differs from the plaintext'
    lz4_cli_decode(frame, expected)
    return {'container': 'lz4mt-skippable', 'version': frames[0]['version'],
            'frame_bytes': frames[0]['frame_size'], 'decoded_bytes': len(plain),
            'independent_decoders': ['python-lz4-frame-decoder', 'lz4-cli']}


def create(row, tool, path):
    name, files, inputs, switches, _, _, password = row
    with tempfile.TemporaryDirectory(prefix='ttzip-owned-7z-') as tmp:
        work = Path(tmp)
        for filename, data in files.items():
            target = work / filename; target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(data)
        args = ['a', '-t7z', '-y', '-mmt=1', '-mtm=off', '-mta=off', '-mtc=off', '-mhc=off', '-mqs=off'] + switches
        if password: args += ['-p' + password]
        run([tool] + args + [str(path)] + inputs, work)
        return args + ['<archive>'] + inputs


def check_row(row, tool, path):
    _, files, _, _, methods, _, password = row
    pwd = ['-p' + password] if password else []
    run([tool, 't', '-y'] + pwd + [str(path)])
    listing = run([tool, 'l', '-slt'] + pwd + [str(path)]).stdout.decode()
    method_lines = [line for line in listing.splitlines() if line.startswith('Method = ')]
    for method in methods:
        assert any(any(token.casefold() == method.casefold() or token.casefold().startswith(method.casefold() + ':') for token in line.split(' = ', 1)[1].split()) for line in method_lines), (path.name, method, method_lines)
    for filename, plaintext in files.items():
        actual = run([tool, 'x', '-so', '-y'] + pwd + [str(path), filename]).stdout
        assert actual == plaintext, (path.name, filename)
    if password:
        assert run([tool, 't', '-y', '-pwrong-password', str(path)], ok=False).returncode != 0
    return inspect_archive(path), method_lines


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--create', action='store_true', help='create missing archives and freeze manifest')
    args = parser.parse_args()
    previous = json.loads((ROOT / 'manifest.json').read_text()) if (ROOT / 'manifest.json').is_file() else {}
    manifest = {}
    for row in specs():
        name, files, _, _, methods, tool_name, password = row
        tool = shutil.which(tool_name)
        if not tool: raise RuntimeError(f'{tool_name} required for offline fixture verification')
        path = ROOT / name
        if not path.exists():
            if not args.create: raise FileNotFoundError(path)
            command = create(row, tool, path)
        else:
            command = ['a', '-t7z', '-y', '-mmt=1', '-mtm=off', '-mta=off', '-mtc=off', '-mhc=off', '-mqs=off'] + row[3]
            if password: command += ['-p' + password]
            command += ['<archive>'] + row[2]
        coders, listing = check_row(row, tool, path)
        verification = verify_lz4_payload(path, files) if 'LZ4' in methods else None
        expected = {f'0x{METHODS[m]:08x}' for m in methods}
        actual = {coder['id'] for folder in coders for coder in folder['coders']}
        # Encrypted headers expose the outer header chain; plaintext listing verifies data codecs.
        assert (expected <= actual) if name != 'aes_header.7z' else '0x06f10701' in actual
        plain_root = ROOT / 'plain' / path.stem
        for filename, data in files.items():
            plain = plain_root / filename
            if args.create:
                plain.parent.mkdir(parents=True, exist_ok=True); plain.write_bytes(data)
            else: assert plain.read_bytes() == data
        version = run([tool, 'i']).stdout.decode().splitlines()
        version = [line for line in version if 'Version ' in line or line.startswith('7-Zip')]
        manifest[name] = dict(digest(path.read_bytes()), command=command, tool=tool_name,
                              tool_version=version, methods=methods, required_data_coder_ids=sorted(expected),
                              coder_graph_scope='encoded_header' if name == 'aes_header.7z' else 'main_streams', coder_folders=coders,
                              method_listing=listing, password=password,
                              payload_verification=verification,
                              plaintext={f: digest(b) for f, b in files.items()})
        if not password:
            with tempfile.TemporaryDirectory(prefix='ttzip-7z-repro-') as tmp:
                recreated = Path(tmp) / name; create(row, tool, recreated)
                assert recreated.read_bytes() == path.read_bytes(), f'{name} is not reproducible'
    if args.create:
        (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    else: assert manifest == previous, 'fixture/tool provenance drift'
    print(f'{len(manifest)} owned archives verified: coder graph, plaintext, CRC/hash and CLI extraction')


if __name__ == '__main__':
    main()
