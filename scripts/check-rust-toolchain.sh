#!/bin/sh
set -eu
export RUSTUP_AUTO_INSTALL=0
rustc_version=$(rustc --version)
cargo_version=$(cargo --version)
case "$rustc_version" in
  "rustc 1.99.1 "*) ;;
  *) printf 'expected rustc 1.99.1, got %s\n' "$rustc_version" >&2; exit 1 ;;
esac
case "$cargo_version" in
  "cargo 1.99.1 "*) ;;
  *) printf 'expected cargo 1.99.1, got %s\n' "$cargo_version" >&2; exit 1 ;;
esac
