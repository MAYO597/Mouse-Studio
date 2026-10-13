#!/usr/bin/env bash
# Build an Arch package (pkg.tar.zst) into ./dist from the current checkout.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/src" "$root/dist"
cp packaging/PKGBUILD "$work/PKGBUILD" 2>/dev/null || cp "$root/packaging/PKGBUILD" "$work/PKGBUILD"
# Copy sources without build output so the package build is clean.
mkdir -p "$work/src/project"
tar -C "$root" --exclude=./target --exclude=./dist --exclude=./output -cf - . | tar -C "$work/src/project" -xf -
cd "$work"
PKGDEST="$root/dist" makepkg --force --noextract --skipinteg
ls -1 "$root/dist"
