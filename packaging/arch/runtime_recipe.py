#!/usr/bin/env python3
"""Generate an offline makepkg recipe from exact local runtime sources."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shlex
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from runtime_manifest import AnchoredRoot, validate_archive


def sha(path):
    h = hashlib.sha256()
    with AnchoredRoot(path.parent) as source:
        with source.open_read(path.name) as (fd, _):
            while True:
                chunk = os.read(fd, 1024 * 1024)
                if not chunk:
                    break
                h.update(chunk)
    return h.hexdigest()


def write_recipe(output, content):
    with AnchoredRoot(output.parent) as target:
        held_fd = None
        try:
            with target.create_file(output.name) as stream:
                held_fd = os.dup(stream.fileno())
                stream.write(content)
            target.verify_outputs(require_exact_tree=False)
            target.verified_output_digest(output.name, held_fd, len(content))
        finally:
            if held_fd is not None:
                os.close(held_fd)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--source', required=True, type=Path)
    p.add_argument('--python', required=True, type=Path)
    p.add_argument('--blender', required=True, type=Path)
    p.add_argument('--node', required=True, type=Path)
    p.add_argument('--web-ifc', required=True, type=Path)
    p.add_argument('--bwrap-package', required=True, type=Path)
    p.add_argument('--wheelhouse', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    a = p.parse_args()
    source = Path(os.path.abspath(a.source))
    entries, source_sha256 = validate_archive(source)
    with AnchoredRoot(Path(os.path.abspath(__file__)).parent) as tools:
        for name in ('runtime_recipe.py', 'runtime_manifest.py'):
            if entries['packaging/arch/' + name] != tools.read_bytes(name):
                raise ValueError(f'recipe tool differs from reviewed source: {name}')
    wheelhouse = Path(os.path.abspath(a.wheelhouse))
    with AnchoredRoot(wheelhouse) as wheel_root:
        wheels = sorted(wheelhouse / name for name in os.listdir(wheel_root.fd) if name.endswith('.whl'))
    if len(wheels) != 15:
        raise ValueError('expected 15 locked wheels')
    assets = [source, *(Path(os.path.abspath(x)) for x in (a.python, a.blender, a.node,
                                                            a.web_ifc, a.bwrap_package)), *wheels]
    if len({x.name for x in assets}) != len(assets):
        raise ValueError('invalid or colliding inputs')
    hashes = [source_sha256, *(sha(x) for x in assets[1:])]
    if hashes[2] != '0d2b8bb2efe97e6c8eb57de6b7322f8800e1984e3125b60decca5f6771e4773e' or hashes[1] != 'b5796915dc45ff745cfb4e990a7e7c93d6f546328a102ce31448d69ebc37575c':
        raise ValueError('upstream source version/hash mismatch')
    if hashes[3] != '88fd1ce767091fd8d4a99fdb2356e98c819f93f3b1f8663853a2dee9b438068a' or hashes[4] != '4c88d9a07ebf5e1e646255edbed02ae2d8cd3a64ff6c3ea52e383525f16d4a26':
        raise ValueError('verifier source version/hash mismatch')
    if hashes[5] != 'f0efc4e02138b1b1f0719f37008da58b03213122d4881cfe949681fcbd72b36e':
        raise ValueError('signed Arch bubblewrap package hash mismatch')
    paths = ' '.join(shlex.quote(f'{x.name}::file://{x}') for x in assets)
    sums = ' '.join(shlex.quote(x) for x in hashes)
    payload_inputs = ' '.join(shlex.quote(x.name) for x in [a.blender, a.node, a.web_ifc, *wheels])
    content = f'''# Generated from SHA-256 checked local sources. Build with --nodeps and network disabled.
pkgname=archy-runtime
pkgver=0.1.0
pkgrel=1
pkgdesc='Fixed Blender and IFC backend runtime for Archy'
arch=('x86_64')
url='https://github.com/butahlecoq/Archy'
license=('GPL-2.0-or-later' 'GPL-3.0-or-later' 'LGPL-3.0-or-later' 'LGPL-2.1-or-later'
         'Python-2.0' 'PSF-2.0' 'MPL-2.0' 'MIT' 'MIT-Khronos-old' 'BSD-2-Clause'
         'BSD-3-Clause' '0BSD' 'Zlib' 'CC0-1.0' 'Apache-2.0' 'Apache-2.0 WITH LLVM-exception'
         'Bitstream-Vera' 'BSL-1.0' 'bzip2-1.0.6' 'FTL' 'libpng-2.0' 'libtiff'
         'NCSA' 'OFL-1.1' 'SGI-B-2.0' 'LicenseRef-Arev-Fonts' 'LicenseRef-ICS'
         'LicenseRef-TOST-1.0' 'LicenseRef-Public-Domain')
options=('!debug' '!strip')
depends=('brotli' 'bzip2' 'e2fsprogs' 'gdbm' 'glibc' 'keyutils' 'krb5' 'libb2'
         'libcap' 'libffi' 'libgcc' 'libglvnd' 'libgomp' 'libice' 'libnsl'
         'libsm' 'libstdc++' 'libtirpc' 'libx11' 'libxau' 'libxcb' 'libxcrypt'
         'libxdmcp' 'libxext' 'libxfixes' 'libxi' 'libxkbcommon' 'libxrender'
         'ncurses' 'openssl' 'readline' 'sqlite' 'util-linux-libs' 'xz' 'zlib' 'zstd')
makedepends=('base-devel' 'python' 'sqlite')
source=({paths})
sha256sums=({sums})
noextract=({shlex.quote(a.python.name)} {shlex.quote(a.blender.name)} {shlex.quote(a.node.name)} {shlex.quote(a.web_ifc.name)} {shlex.quote(a.bwrap_package.name)})

build() {{
  cd "$srcdir"
  /usr/bin/python3 -I -S -B "$srcdir/archy-source/packaging/arch/runtime_manifest.py" --source "$srcdir/archy-source"
  mkdir -p cpython-build
  tar -xJf {shlex.quote(a.python.name)} -C cpython-build --strip-components=1
  cd cpython-build
  ./configure --prefix=/usr/lib/archy/runtime/python --with-ensurepip=no
  make -j2
  make DESTDIR="$srcdir/python-stage" altinstall
  local sqlite_modules=("$srcdir/python-stage/usr/lib/archy/runtime/python/lib/python3.12/lib-dynload"/_sqlite3*.so)
  [[ -f "${{sqlite_modules[0]}}" ]] || return 1
}}

package() {{
  cd "$srcdir"
  mkdir -p verified-inputs
  for asset in {payload_inputs}; do
    install -m 0644 "$srcdir/$asset" "$srcdir/verified-inputs/$asset"
  done
  mkdir -p bwrap-extract
  tar --zstd -xf {shlex.quote(a.bwrap_package.name)} -C bwrap-extract usr/bin/bwrap usr/share/licenses/bubblewrap/LICENSE 2>/dev/null || \\
    tar --zstd -xf {shlex.quote(a.bwrap_package.name)} -C bwrap-extract usr/bin/bwrap
  /usr/bin/python3 -I -S -B "$srcdir/archy-source/packaging/arch/runtime.py" \\
    --dest "$pkgdir" --python-prefix "$srcdir/python-stage/usr/lib/archy/runtime/python" \\
    --python-license "$srcdir/cpython-build/LICENSE" \\
    --blender-archive "$srcdir/verified-inputs/{a.blender.name}" --blender-sha256 {hashes[2]} \\
    --node-archive "$srcdir/verified-inputs/{a.node.name}" --node-sha256 {hashes[3]} \\
    --web-ifc-archive "$srcdir/verified-inputs/{a.web_ifc.name}" --web-ifc-sha256 {hashes[4]} \\
    --wheelhouse "$srcdir/verified-inputs" --bwrap "$srcdir/bwrap-extract/usr/bin/bwrap" \\
    --bwrap-sha256 33516b9cd1fa6432307ad71f23765fa30604b0a8d769dac0d2ba054e85aeec83 \\
    --source "$srcdir/archy-source"
}}
'''
    write_recipe(Path(os.path.abspath(a.output)), content.encode())
    print('source_sha256=' + hashes[0])

if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError) as e:
        print(f'runtime recipe: {e}', file=sys.stderr)
        sys.exit(1)
