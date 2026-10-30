#!/usr/bin/env python3
"""Build a pinned libXt source candidate into a new private directory.

This produces evidence, not a runtime payload or release approval.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


SOURCE_SHA256 = 'e0a774b33324f4d4c05b199ea45050f87206586d81655f8bef4dba434d931288'
COPYING_SHA256 = '75c5574ca04731d739b1420f55f2b7b47f30df895817f1b03d0d7f5c1fbee534'
NEEDED = {'libSM.so.6', 'libICE.so.6', 'libX11.so.6', 'libc.so.6'}


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def run(*argv, cwd=None, env=None):
    subprocess.run(argv, cwd=cwd, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tarball', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--host-probe', action='store_true',
                        help='allow XT_CFLAGS/XT_LIBS overrides for a non-target feasibility build')
    args = parser.parse_args()
    tarball = args.tarball.resolve(strict=True)
    if digest(tarball) != SOURCE_SHA256:
        raise ValueError('libXt source SHA-256 mismatch')
    if digest(Path(__file__).resolve().parent / 'libxt-source/COPYING') != COPYING_SHA256:
        raise ValueError('committed libXt COPYING SHA-256 mismatch')
    output = args.output.absolute()
    output.mkdir(mode=0o700)
    run('tar', '-xJf', str(tarball), '-C', str(output))
    source = output / 'libXt-1.3.1'
    notice = source / 'COPYING'
    if digest(notice) != COPYING_SHA256:
        raise ValueError('libXt COPYING SHA-256 mismatch')
    build = output / 'build'
    build.mkdir()
    stage = output / 'stage'
    stage.mkdir()
    env = os.environ.copy()
    if not args.host_probe:
        env.pop('XT_CFLAGS', None)
        env.pop('XT_LIBS', None)
    run(str(source / 'configure'), '--prefix=/usr', '--sysconfdir=/etc',
        '--disable-static', '--without-xmlto', '--without-fop', '--without-xsltproc',
        cwd=build, env=env)
    run('make', '-j2', cwd=build, env=env)
    run('make', f'DESTDIR={stage}', 'install', cwd=build, env=env)
    library = stage / 'usr/lib/libXt.so.6.0.0'
    dynamic = subprocess.check_output(['readelf', '-d', str(library)], text=True)
    sonames = re.findall(r'\(SONAME\).*?\[(.*?)\]', dynamic)
    needed = set(re.findall(r'\(NEEDED\).*?\[(.*?)\]', dynamic))
    if sonames != ['libXt.so.6'] or needed != NEEDED:
        raise ValueError(f'unexpected libXt ELF linkage: SONAME={sonames}, NEEDED={sorted(needed)}')
    if (stage / 'usr/lib/libXt.so.6').resolve() != library or not library.is_file():
        raise ValueError('libXt SONAME link missing')
    result = {
        'source_sha256': SOURCE_SHA256,
        'copying_sha256': COPYING_SHA256,
        'candidate_sha256': digest(library),
        'soname': sonames[0],
        'needed': sorted(needed),
        'build_environment': 'host_probe' if args.host_probe else 'pkg_config',
        'status': 'source_candidate_only',
    }
    (output / 'result.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f'libXt source candidate: {error}', file=sys.stderr)
        sys.exit(1)
