# Maintainer: scubba
#
# Flock — the ewe account app (RFC-006). Tauri v2 has no pacman bundler, so
# this PKGBUILD is how it is packaged for Arch, exactly like Komble and
# ewe-settings. No privileged helper: everything Flock touches is the user's.

pkgname=flock
pkgver=0.1.0
pkgrel=1
pkgdesc="Flock — your ewe account: the one file, your machines, your folders"
arch=('x86_64' 'aarch64')
url="https://github.com/prj786/flock"
license=('MIT')

depends=(
  'webkit2gtk-4.1'
  'gtk3'
  'libayatana-appindicator'   # the tray icon (StatusNotifierItem)
  'python'                    # the ewe tools it drives are Python
  'libsecret'                 # secret-tool — the app password lives in the keyring
)
optdepends=(
  'ewe: the desktop this is the account app of (ewe-cloud, ewe-conf)'
  'nextcloud-client: nextcloudcmd, the folder-sync engine (Flock 0.2)'
)
makedepends=('rust' 'cargo' 'nodejs' 'npm')

# Arch's makepkg LTO breaks linking the `ring` crate's assembly (see Komble's
# PKGBUILD for the full story); Rust-level LTO stays on in Cargo.toml.
options=(!lto !debug)

source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
  cd "$srcdir/$pkgname-$pkgver"
  npm ci
  # Through the Tauri CLI, never bare `cargo build`: tauri-build decides
  # dev-vs-production at compile time and a plain cargo build bakes the vite
  # devUrl into the binary. --no-bundle: this PKGBUILD is the packaging.
  npm run tauri build -- --no-bundle
}

package() {
  cd "$srcdir/$pkgname-$pkgver"

  install -Dm755 src-tauri/target/release/ewe-flock "$pkgdir/usr/bin/ewe-flock"

  install -Dm644 packaging/io.github.prj786.flock.desktop \
    "$pkgdir/usr/share/applications/io.github.prj786.flock.desktop"
  for s in 32 64 128 256; do
    install -Dm644 "src-tauri/icons/${s}x${s}.png" \
      "$pkgdir/usr/share/icons/hicolor/${s}x${s}/apps/io.github.prj786.flock.png"
  done
  install -Dm644 packaging/flock.svg \
    "$pkgdir/usr/share/icons/hicolor/scalable/apps/io.github.prj786.flock.svg"

  # Autostart: ewe's session activates graphical-session.target, so a user
  # unit wanted by it starts Flock hidden (tray only) at every login. The unit
  # is installed but not enabled — ewe's `ewe-setup` enables it for the user
  # (RFC-006 F3); until then `systemctl --user enable --now ewe-flock.service`.
  install -Dm644 packaging/ewe-flock.service \
    "$pkgdir/usr/lib/systemd/user/ewe-flock.service"

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
