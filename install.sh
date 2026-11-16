#!/bin/sh
# Installs the nametrail binary. Usage:
#   curl -fsSL https://raw.githubusercontent.com/LIVELUCKY/nametrail/main/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --scope project     (connect agents for this repository only)
#   sh install.sh [--from-source] [--scope global|project] [--no-setup]
# Run from a clone of the repository, it builds that checkout and needs no network. Piped from the
# web, it downloads the release binary and builds from the repository only when there is none.
# After installing, `nametrail setup` connects the agents found on this machine (default: globally).
# Environment: NAMETRAIL_BIN_DIR (default ~/.local/bin), NAMETRAIL_VERSION (default: latest release),
# NAMETRAIL_OWNER (the GitHub account that hosts the repository).
set -eu

OWNER="${NAMETRAIL_OWNER:-LIVELUCKY}"
REPO="$OWNER/nametrail"
BIN_DIR="${NAMETRAIL_BIN_DIR:-$HOME/.local/bin}"
VERSION="${NAMETRAIL_VERSION:-latest}"
from_source=0
scope=global
setup=1

while [ $# -gt 0 ]; do
  arg=$1
  shift
  case "$arg" in
    --from-source) from_source=1 ;;
    --no-setup) setup=0 ;;
    --scope)
      scope=${1:-}
      shift || true
      case "$scope" in global | project) ;; *)
        echo "nametrail: --scope takes global or project" >&2
        exit 2
        ;;
      esac
      ;;
    -h | --help)
      sed -n '2,10p' "$0"
      exit 0
      ;;
    *)
      echo "nametrail: unknown option $arg" >&2
      exit 2
      ;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  *) target=""; from_source=1 ;;
esac

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

install_release() {
  if [ "$VERSION" = latest ]; then
    base="https://github.com/$REPO/releases/latest/download"
  else
    base="https://github.com/$REPO/releases/download/v$VERSION"
  fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  archive="nametrail-$target.tar.gz"
  curl -fsSL "$base/$archive" -o "$tmp/$archive" || return 1
  curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" || return 1
  expected="$(grep " $archive\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)"
  actual="$(sha256 "$tmp/$archive")"
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    echo "nametrail: checksum mismatch for $archive, not installing" >&2
    return 1
  fi
  tar -xzf "$tmp/$archive" -C "$tmp"
  mkdir -p "$BIN_DIR"
  install -m 0755 "$tmp/nametrail" "$BIN_DIR/nametrail"
}

# The folder of this script when it is a nametrail checkout; empty when the script was piped in.
checkout=""
case "$0" in
  */install.sh | install.sh)
    dir="$(cd "$(dirname "$0")" && pwd)"
    grep -q '^name = "nametrail"' "$dir/Cargo.toml" 2>/dev/null && checkout="$dir"
    ;;
esac

install_source() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "nametrail: building needs a Rust toolchain; install it from https://rustup.rs and rerun" >&2
    exit 1
  fi
  if [ -n "$checkout" ]; then
    cargo install --locked --path "$checkout" --root "${BIN_DIR%/bin}" --force
  else
    cargo install --locked --git "https://github.com/$REPO" --root "${BIN_DIR%/bin}" nametrail || {
      echo "nametrail: could not fetch https://github.com/$REPO; clone it and run its install.sh" >&2
      exit 1
    }
  fi
}

if [ -n "$checkout" ]; then
  install_source
elif [ "$from_source" = 0 ]; then
  install_release || {
    echo "nametrail: release download failed, building from source" >&2
    install_source
  }
else
  install_source
fi

echo "nametrail installed: $BIN_DIR/nametrail"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "note: add $BIN_DIR to your PATH" ;;
esac
if [ "$setup" = 1 ]; then
  echo
  "$BIN_DIR/nametrail" setup --scope "$scope" || echo "nametrail setup did not finish; rerun it with --help" >&2
fi
cat <<EOF

Other agents or scopes: nametrail setup --agent <name,...> --scope global|project
EOF
