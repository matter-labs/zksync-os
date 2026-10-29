#!/bin/bash
# Builds the four variants of the ecrecover benchmark guest (the bigint and the
# keccak delegations on or off) and packages each as a dist directory the host
# loads (`airbender_host::Program::load`), the way `../build.sh` does:
# `dist/<variant>/{app.bin,app.elf,app.text,manifest.toml}`.
#
# usage: build_variants.sh [variant ...]   (default: all four)
#   variants: bigint_keccak (production), bigint, keccak, none
set -euo pipefail
cd "$(dirname "$0")"

TARGET=riscv32i-unknown-none-elf
VARIANTS=("$@")
if [ ${#VARIANTS[@]} -eq 0 ]; then
  VARIANTS=(bigint_keccak bigint keccak none)
fi

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

for variant in "${VARIANTS[@]}"; do
  case "$variant" in
    bigint_keccak) features="bigint,keccak" ;;
    bigint) features="bigint" ;;
    keccak) features="keccak" ;;
    none) features="" ;;
    *) echo "unknown variant $variant (bigint_keccak, bigint, keccak, none)" >&2; exit 1 ;;
  esac
  echo "== building ecrecover_bench [$variant] (features: ${features:-none})"
  cargo build --release --no-default-features --features "$features" --target-dir "target/$variant"
  elf="target/$variant/$TARGET/release/ecrecover_bench"
  dist="dist/$variant"
  rm -rf "$dist"
  mkdir -p "$dist"
  rust-objcopy -O binary "$elf" "$dist/app.bin"
  rust-objcopy -R .text "$elf" "$dist/app.elf"
  rust-objcopy -O binary --only-section=.text "$elf" "$dist/app.text"
  cat > "$dist/manifest.toml" <<MANIFEST
package = "ecrecover_bench"
manifest = "v1"
codec = "v0"
target = "$TARGET"

[bin]
path = "app.bin"
sha256 = "$(sha256 "$dist/app.bin")"

[elf]
path = "app.elf"
sha256 = "$(sha256 "$dist/app.elf")"

[text]
path = "app.text"
sha256 = "$(sha256 "$dist/app.text")"

[build]
profile = "release"
features = "$features"
git_branch = "$(git rev-parse --abbrev-ref HEAD 2> /dev/null || echo unknown)"
git_commit = "$(git rev-parse HEAD 2> /dev/null || echo unknown)"
is_dirty = $(if [ -n "$(git status --porcelain 2> /dev/null)" ]; then echo true; else echo false; fi)
MANIFEST
  echo "-> $dist/app.bin ($(wc -c < "$dist/app.bin") bytes)"
done
