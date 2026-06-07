#!/bin/bash

# Check if an argument was provided
if [ -z "$1" ]; then
  echo "Error: No package version provided."
  echo "Usage: $0 <pkgver>"
  echo "Example: $0 1.0.0.beta.16"
  exit 1
fi

PKGVER=$1
FILENAME="PKGBUILD"

# Generate the PKGBUILD file
cat << EOF > PKGBUILD
# Maintainer: Nick Dowsett <nickd42 AT gmail DOT com>

pkgname=cosmic-applet-arch
pkgver=$PKGVER
pkgrel=1
pkgdesc='COSMIC applet to display Arch Linux package status'
arch=(x86_64)
url=https://github.com/nick42d/cosmic-applet-arch
license=(GPL-3.0-only)
depends=(
  cosmic-icon-theme
  git
  pacman-contrib
  openssl
  libxkbcommon
)
makedepends=(
  pkgconf
  cargo
  git
  just
  lld
)
source=(git+https://github.com/nick42d/cosmic-applet-arch.git#tag=\${pkgname}/v\${pkgver})
b2sums=('tbc')

prepare() {
  cd cosmic-applet-arch
  cargo fetch --locked
  sed 's/lto = "fat"/lto = "thin"/' -i Cargo.toml
}

build() {
  cd cosmic-applet-arch
  RUSTFLAGS+=" -C link-arg=-fuse-ld=lld"
  just build-release --frozen
}

package() {
  cd cosmic-applet-arch
  just rootdir="\${pkgdir}" install
}
EOF

echo "PKGBUILD generated successfully for version $PKGVER."
