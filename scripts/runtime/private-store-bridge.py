#!/usr/bin/env python3
"""Node private writer adapter; text travels on stdin, never on argv."""
import json
import base64
import os
from pathlib import Path
import runpy
import sys

PRIVATE = runpy.run_path(str(Path(__file__).with_name('private_store_io.py')))


def main():
    request = json.load(sys.stdin)
    mode = request['operation']
    if mode == 'redact':
        sys.stdout.write(PRIVATE['redact_text'](request['text']))
    elif mode == 'replace-bytes':
        PRIVATE['replace_bytes'](request['path'], base64.b64decode(request['body'], validate=True))
    elif mode == 'replace':
        PRIVATE['replace_text'](request['path'], request['text'])
    elif mode == 'append':
        clean = PRIVATE['redact_text'](request['text']).encode('utf-8')
        with PRIVATE['private_append'](request['path']) as fd:
            offset = 0
            while offset < len(clean):
                written = os.write(fd, clean[offset:])
                if written <= 0:
                    raise OSError('short private write')
                offset += written
            os.fsync(fd)
    elif mode == 'copy':
        scanner = runpy.run_path(str(Path(__file__).with_name('trajectory-redact.py')))
        body = scanner['safe_read'](request['source'])
        if '\0' in body or any(0xDC80 <= ord(c) <= 0xDCFF for c in body):
            PRIVATE['replace_bytes'](request['path'], body.encode('utf-8', errors='surrogateescape'))
        else:
            PRIVATE['replace_text'](request['path'], body)
    else:
        raise ValueError('unknown operation')
    if request.get('executable') and mode != 'redact':
        with PRIVATE['private_parent'](request['path']) as (directory, name):
            fd = PRIVATE['_checked'](directory, name, os.O_RDONLY)
            try:
                os.fchmod(fd, 0o700)
            finally:
                os.close(fd)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RecursionError):
        print('private write refused: invalid text, credentials in sealed data, or unsafe path', file=sys.stderr)
        sys.exit(2)
