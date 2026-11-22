#!/bin/sh
# Installs urlmenu and its desktop entry, man pages, shell completions and
# sample config, from the source tree or from a release archive.
#
#   packaging/install.sh                       # into /usr/local
#   PREFIX=/usr DESTDIR="$pkgdir" packaging/install.sh
#   BIN=path/to/urlmenu packaging/install.sh   # binary built elsewhere
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
prefix=${PREFIX:-/usr/local}
dest=${DESTDIR:-}

if [ -n "${BIN:-}" ]; then
    bin=$BIN
elif [ -f "$root/urlmenu" ]; then
    bin=$root/urlmenu
else
    bin=$root/target/release/urlmenu
fi
[ -f "$bin" ] || { echo "install.sh: $bin not found; run cargo build --release --locked or set BIN" >&2; exit 1; }

share=$dest$prefix/share
install -Dm755 "$bin" "$dest$prefix/bin/urlmenu"
install -Dm644 "$root/packaging/urlmenu.desktop" "$share/applications/urlmenu.desktop"
install -Dm644 "$root/packaging/urlmenu.svg" "$share/icons/hicolor/scalable/apps/urlmenu.svg"
for page in "$root"/packaging/man/*.1; do
    install -Dm644 "$page" "$share/man/man1/$(basename "$page")"
done
install -Dm644 "$root/packaging/completions/urlmenu.bash" "$share/bash-completion/completions/urlmenu"
install -Dm644 "$root/packaging/completions/urlmenu.fish" "$share/fish/vendor_completions.d/urlmenu.fish"
install -Dm644 "$root/packaging/completions/_urlmenu" "$share/zsh/site-functions/_urlmenu"
install -Dm644 "$root/config.sample.toml" "$share/doc/urlmenu/config.sample.toml"
install -Dm644 "$root/UNLICENSE" "$share/licenses/urlmenu/UNLICENSE"
