#!/usr/bin/env python3
"""Freeze reviewed backend runtime inputs into a deterministic local tarball."""
import argparse
import gzip
import io
import os
from pathlib import Path
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from runtime_manifest import AnchoredRoot, MAX_TOTAL, capture_tree


def write_snapshot(entries, output):
    with AnchoredRoot(output.parent) as target:
        held_fd = None
        try:
            with target.create_file(output.name) as raw:
                held_fd = os.dup(raw.fileno())
                with gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0) as compressed:
                    with tarfile.open(fileobj=compressed, mode='w') as archive:
                        for path, data in sorted(entries.items()):
                            info = tarfile.TarInfo('archy-source/' + path)
                            info.size = len(data)
                            info.mode = 0o644
                            info.mtime = 0
                            archive.addfile(info, io.BytesIO(data))
            target.verify_outputs(require_exact_tree=False)
            return target.verified_output_digest(output.name, held_fd, MAX_TOTAL)
        finally:
            if held_fd is not None:
                os.close(held_fd)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    entries = capture_tree(args.source)
    with AnchoredRoot(Path(os.path.abspath(__file__)).parent) as tools:
        for name in ('runtime_snapshot.py', 'runtime_manifest.py'):
            if entries['packaging/arch/' + name] != tools.read_bytes(name):
                raise ValueError(f'snapshot tool differs from reviewed source: {name}')
    digest = write_snapshot(entries, args.output)
    print(f'{digest}  {args.output}')


if __name__ == '__main__':
    main()
