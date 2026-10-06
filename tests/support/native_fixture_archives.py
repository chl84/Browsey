"""Generate small synthetic archives in memory. No personal/sample archives."""
import base64
import bz2
import gzip
import io
import json
import lzma
import struct
import subprocess
import tarfile
import zipfile
import zlib

PAYLOAD = b'generated archive payload\n'
NAME = 'spaced file.txt'


def tar_bytes(malicious=False):
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode='w', format=tarfile.PAX_FORMAT) as archive:
        names = [NAME, 'nested/second.txt']
        if malicious:
            names += ['../sentinel.txt', '/forbidden-native-archive.txt', 'nested/../../sentinel.txt']
        for name in names:
            item = tarfile.TarInfo(name)
            item.mode = 0o600
            item.size = len(PAYLOAD)
            archive.addfile(item, io.BytesIO(PAYLOAD))
        if malicious:
            item = tarfile.TarInfo('link')
            item.type = tarfile.SYMTYPE
            item.linkname = '/forbidden-native-archive.txt'
            archive.addfile(item)
    return out.getvalue()


def zip_bytes(malicious=False):
    out = io.BytesIO()
    with zipfile.ZipFile(out, 'w') as archive:
        names = [NAME, 'nested/second.txt']
        if malicious:
            names += ['../sentinel.txt', '/forbidden-native-archive.txt', 'nested/../../sentinel.txt']
        for name in names:
            info = zipfile.ZipInfo(name)
            info.external_attr = (0o100600 << 16)
            archive.writestr(info, PAYLOAD)
    return out.getvalue()


def command(argv, data):
    return subprocess.run(argv, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=True, timeout=10).stdout


def rar_bytes():
    # Generated RAR4 stored records; CRC16 is the low half of header CRC32.
    def header(kind, flags, body):
        value = struct.pack('<BHH', kind, flags, 7 + len(body)) + body
        return struct.pack('<H', zlib.crc32(value) & 0xffff) + value
    name = NAME.encode()
    file_body = struct.pack('<IIBIIBBHI', len(PAYLOAD), len(PAYLOAD), 3,
                            zlib.crc32(PAYLOAD), 0, 20, 0x30, len(name), 0o100600) + name
    return (b'Rar!\x1a\x07\x00' + header(0x73, 0, b'\0' * 6)
            + header(0x74, 0x8000, file_body) + PAYLOAD + header(0x7b, 0, b''))


def main():
    tar = tar_bytes()
    fixtures = {'zip': zip_bytes(), 'tar': tar, 'tar.gz': gzip.compress(tar, mtime=0),
                'tar.bz2': bz2.compress(tar), 'tar.xz': lzma.compress(tar),
                'tar.zst': command(['/usr/bin/zstd', '-q', '-c'], tar),
                '7z': command(['/usr/bin/bsdtar', '-cf', '-', '--format=7zip', '@-'], tar),
                'rar': rar_bytes(), 'txt.gz': gzip.compress(PAYLOAD, mtime=0),
                'txt.bz2': bz2.compress(PAYLOAD), 'txt.xz': lzma.compress(PAYLOAD),
                'txt.zst': command(['/usr/bin/zstd', '-q', '-c'], PAYLOAD),
                'malicious.zip': zip_bytes(True), 'malicious.tar': tar_bytes(True),
                'broken.zip': b'PK\x03\x04generated truncated archive'}
    assert len(fixtures) == 15 and all(len(data) <= 65536 for data in fixtures.values())
    print(json.dumps({key: base64.b64encode(value).decode('ascii') for key, value in fixtures.items()}))


if __name__ == '__main__':
    main()
