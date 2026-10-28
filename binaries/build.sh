#!/bin/bash
# Builds the Glamsterdam guest programs and packages what the host loads
# (`airbender_host::Program::load`): `dist/{app.bin,app.elf,app.text,manifest.toml}`
# next to each program, produced from the ELF with `rust-objcopy` the way the
# Airbender SDK build does, without `cargo-airbender`.
#
# usage: build.sh [program ...]   (default: every program in this directory)
#
# Needs `cargo-binutils` (`rust-objcopy`) and the `llvm-tools-preview` component.
set -euo pipefail
cd "$(dirname "$0")"

TARGET=riscv32i-unknown-none-elf
PROGRAMS=("$@")
if [ ${#PROGRAMS[@]} -eq 0 ]; then
  PROGRAMS=(glamsterdam_pre_execution glamsterdam_tx_range glamsterdam_aggregation)
fi

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

for app in "${PROGRAMS[@]}"; do
  if [ ! -f "$app/Cargo.toml" ]; then
    echo "no program named $app here" >&2
    exit 1
  fi
  echo "== building $app"
  (cd "$app" && cargo build --release)
  elf="$app/target/$TARGET/release/$app"
  dist="$app/dist"
  rm -rf "$dist"
  mkdir -p "$dist"
  rust-objcopy -O binary "$elf" "$dist/app.bin"
  rust-objcopy -R .text "$elf" "$dist/app.elf"
  rust-objcopy -O binary --only-section=.text "$elf" "$dist/app.text"
  cat > "$dist/manifest.toml" <<MANIFEST
package = "$app"
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
git_branch = "$(git rev-parse --abbrev-ref HEAD 2> /dev/null || echo unknown)"
git_commit = "$(git rev-parse HEAD 2> /dev/null || echo unknown)"
is_dirty = $(if [ -n "$(git status --porcelain 2> /dev/null)" ]; then echo true; else echo false; fi)
MANIFEST
  echo "-> $dist/app.bin ($(wc -c < "$dist/app.bin") bytes), app.elf, app.text, manifest.toml"
done
