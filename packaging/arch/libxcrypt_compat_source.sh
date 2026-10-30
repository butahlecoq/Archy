#!/usr/bin/env bash
# Build and inspect the pinned compatibility library without installing it.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 libxcrypt-4.5.2.tar.xz NEW_PRIVATE_OUTPUT_DIR" >&2
  exit 2
fi
archive=$(realpath "$1")
output=$2
if [[ -e $output ]]; then
  echo "output directory already exists: $output" >&2
  exit 2
fi
expected=71513a31c01a428bccd5367a32fd95f115d6dac50fb5b60c779d5c7942aec071
actual=$(sha256sum "$archive" | cut -d' ' -f1)
if [[ $actual != "$expected" ]]; then
  echo "libxcrypt source SHA-256 mismatch" >&2
  exit 1
fi

umask 077
mkdir -p "$output"
output=$(realpath "$output")
tar -xJf "$archive" -C "$output"
source_dir="$output/libxcrypt-4.5.2"
printf '%s  %s\n' \
  9144e66d8140feb84f0e4e344906ac425378896320032e0248567ef195cf0135 "$source_dir/LICENSING" \
  dc626520dcd53a22f727af3ee42c770e56c97a64fe3adb063799d8ab032fe551 "$source_dir/COPYING.LIB" | sha256sum -c
mkdir "$output/build"
cd "$output/build"

# Arch libxcrypt 4.5.2-1 compatibility options; GCC 15 needs this narrow
# warning override for two upstream const-qualifier warnings.
CFLAGS='-O2 -Wno-error=discarded-qualifiers' "$source_dir/configure" \
  --prefix=/usr --disable-static --enable-hashes=strong,glibc \
  --enable-obsolete-api=glibc --disable-failure-tokens > configure.log 2>&1
make -j2 > make.log 2>&1
make -j2 check > check.log 2>&1
make DESTDIR="$output/stage" install > install.log 2>&1

library="$output/stage/usr/lib/libcrypt.so.1.1.0"
readelf -d "$library" > dynamic.txt
readelf --dyn-syms --wide "$library" > symbols.txt
readelf --version-info "$library" > versions.txt
grep -Fq 'Library soname: [libcrypt.so.1]' dynamic.txt
grep -Fq 'Shared library: [libc.so.6]' dynamic.txt
if [[ $(grep -c 'Shared library:' dynamic.txt) -ne 1 ]]; then
  echo 'unexpected direct ELF dependency' >&2
  exit 1
fi
grep -Fq 'crypt_r@@XCRYPT_2.0' symbols.txt
printf 'source_sha256=%s\n' "$actual"
printf 'library_sha256=%s\n' "$(sha256sum "$library" | cut -d' ' -f1)"
grep '^# \(TOTAL\|PASS\|SKIP\|FAIL\|ERROR\):' check.log
grep 'Shared library:\|Library soname:' dynamic.txt
echo 'ABI-only source build; target compatibility and distribution remain review gates.'
