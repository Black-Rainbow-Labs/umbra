#!/bin/sh
# Installs Umbra on macOS or Linux from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.sh | sh
#
# It downloads the archive for this machine and SHA256SUMS, checks the archive against
# SHA256SUMS, and puts `umbra` in ~/.local/bin. It needs no administrator rights.
#
#   UMBRA_VERSION=0.4.0    install that version instead of the latest
#   UMBRA_INSTALL_DIR=dir  install into dir instead of ~/.local/bin

set -eu

repo="Black-Rainbow-Labs/umbra"
install_dir="${UMBRA_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  echo "umbra install: $*" >&2
  exit 1
}

# The release target for this operating system and processor.
target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "no Umbra build for the processor $arch" ;;
  esac
  case "$os" in
    Darwin) echo "$arch-apple-darwin" ;;
    Linux) echo "$arch-unknown-linux-musl" ;;
    *) fail "no Umbra build for $os. On Windows, use install.ps1" ;;
  esac
}

# Downloads a URL to a file with curl or wget.
fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$1" -O "$2"
  else
    fail "needs curl or wget"
  fi
}

# Prints the SHA-256 of a file.
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

name="umbra-$(target).tar.gz"
if [ -n "${UMBRA_VERSION:-}" ]; then
  base="https://github.com/$repo/releases/download/v${UMBRA_VERSION#v}"
else
  base="https://github.com/$repo/releases/latest/download"
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading $base/$name"
fetch "$base/$name" "$work/$name" || fail "could not download $base/$name"
fetch "$base/SHA256SUMS" "$work/SHA256SUMS" || fail "could not download $base/SHA256SUMS"

expected=$(grep " $name\$" "$work/SHA256SUMS" | cut -d ' ' -f 1)
[ -n "$expected" ] || fail "SHA256SUMS has no line for $name"
actual=$(sha256 "$work/$name")
[ "$expected" = "$actual" ] || fail "$name does not match SHA256SUMS (expected $expected, got $actual)"

tar -xzf "$work/$name" -C "$work"
mkdir -p "$install_dir"
mv -f "$work/umbra" "$install_dir/umbra"
chmod +x "$install_dir/umbra"

echo "Installed $("$install_dir/umbra" --version) to $install_dir/umbra"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "Add $install_dir to your PATH, for example: echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.profile" ;;
esac
echo "Next: run \`umbra setup\` to choose a model."
