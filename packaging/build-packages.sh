#!/bin/sh
# Builds the .deb and .rpm of a compiled urlmenu binary with nfpm.
#
#   packaging/build-packages.sh <binary> <version> <deb-arch> <out-dir>
#   packaging/build-packages.sh target/release/urlmenu 0.1.1 amd64 dist
set -eu

[ $# -eq 4 ] || { echo "usage: $0 <binary> <version> <deb-arch> <out-dir>" >&2; exit 2; }
root=$(cd "$(dirname "$0")/.." && pwd)
pkgroot=$(mktemp -d)
trap 'rm -rf "$pkgroot"' EXIT

PREFIX=/usr DESTDIR="$pkgroot" BIN=$1 "$root/packaging/install.sh"
mkdir -p "$4"
for format in deb rpm; do
    VERSION=$2 ARCH=$3 PKGROOT=$pkgroot nfpm package -f "$root/packaging/nfpm.yaml" -p "$format" -t "$4"
done
