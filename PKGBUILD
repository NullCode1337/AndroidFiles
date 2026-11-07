# Maintainer: NullCode1337 <contact.nullcode@proton.me>
pkgname=androidfiles-git
pkgver=0.1.0
pkgrel=1
pkgdesc="Desktop file manager for Android devices over adb"
arch=('x86_64' 'aarch64')
url="https://github.com/NullCode1337/AndroidFiles"
license=('GPL-3.0-only')
depends=(
  'android-tools'
  'fontconfig'
  'freetype2'
  'gcc-libs'
  'glibc'
  'libglvnd'
  'libxcb'
  'libxkbcommon'
  'libxkbcommon-x11'
  'vulkan-icd-loader'
  'wayland'
)
makedepends=('cargo' 'cmake' 'git' 'perl' 'rust')
provides=('androidfiles')
conflicts=('androidfiles')
source=("$pkgname::git+$url.git")
sha256sums=('SKIP')
# Distro LTO breaks linking the wayland-backend during compile
options=('!lto')

pkgver() {
  cd "$pkgname"
  printf 'r%s.%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
  cd "$pkgname"
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd "$pkgname"
  cargo build --release --locked --frozen
}

check() {
  cd "$pkgname"
  cargo test --release --locked --frozen
}

package() {
  cd "$pkgname"
  install -Dm755 target/release/AndroidFiles "$pkgdir/usr/bin/androidfiles"
  install -Dm644 src/assets/androidfiles.desktop "$pkgdir/usr/share/applications/androidfiles.desktop"
  install -Dm644 src/assets/logo.png \
    "$pkgdir/usr/share/icons/hicolor/1024x1024/apps/androidfiles.png"
}
