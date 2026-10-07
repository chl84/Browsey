"""Inspect/evict only generated file data pages; never clear global caches."""
import ctypes
import json
import os
from pathlib import Path
import re
import stat
import sys


def main():
    request = json.loads(sys.stdin.read(65537))
    assert set(request) == {'root', 'paths'}
    root = Path(request['root'])
    assert root.is_absolute() and '..' not in root.parts
    assert root.parts[-3] == 'ai_agent_testfolder'
    assert re.fullmatch(r'\.bnt-[0-9a-f]{32}', root.parts[-2])
    assert root.parts[-1] == 'files'
    paths = request['paths']
    assert 1 <= len(paths) <= 12 and len(set(paths)) == len(paths)
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mmap.restype = ctypes.c_void_p
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int,
                          ctypes.c_int, ctypes.c_int, ctypes.c_long]
    libc.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p]
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    page = os.sysconf('SC_PAGE_SIZE')
    result = []
    for raw in paths:
        path = Path(raw)
        rel = path.relative_to(root)
        assert len(rel.parts) == 2
        assert re.fullmatch(r'storage-sample-[0-4]', rel.parts[0])
        assert re.fullmatch(r'image-[0-9]{2}\.png', rel.parts[1])
        current = Path('/')
        for component in path.parts[1:]:
            current /= component
            assert not stat.S_ISLNK(current.lstat().st_mode)
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            meta = os.fstat(fd)
            assert stat.S_ISREG(meta.st_mode) and meta.st_uid == os.getuid()
            assert meta.st_nlink == 1 and 0 < meta.st_size <= 65536
            pages = (meta.st_size + page - 1) // page

            def resident():
                # PROT_NONE mappings do not fault/read file contents.
                address = libc.mmap(None, meta.st_size, 0, 2, fd, 0)
                assert address != ctypes.c_void_p(-1).value
                try:
                    vector = (ctypes.c_ubyte * pages)()
                    assert libc.mincore(address, meta.st_size, vector) == 0
                    return sum(value & 1 for value in vector)
                finally:
                    assert libc.munmap(address, meta.st_size) == 0

            before = resident()
            os.fsync(fd)
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
            after = resident()
            now = path.lstat()
            assert (now.st_dev, now.st_ino, now.st_size) == (meta.st_dev, meta.st_ino, meta.st_size)
            result.append({'name': str(rel), 'bytes': meta.st_size, 'pages': pages,
                           'residentBefore': before, 'residentAfter': after})
        finally:
            os.close(fd)
    print(json.dumps({'schema': 1, 'scope': 'owned host file-data pages only; metadata/device caches uncontrolled',
                      'verifiedDataCold': all(entry['residentAfter'] == 0 for entry in result),
                      'files': result}))


if __name__ == '__main__':
    main()
