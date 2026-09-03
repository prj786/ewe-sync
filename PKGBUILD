# Maintainer: scubba
#
# ewe-sync — the ewe account app (RFC-006). Tauri v2 has no pacman bundler, so
# this PKGBUILD is how it is packaged for Arch, exactly like Komble and
# ewe-settings. No privileged helper: everything ewe-sync touches is the user's.

pkgname=ewe-sync
pkgver=0.3.0
pkgrel=1
pkgdesc="ewe-sync — your ewe account: the one file, your machines, your folders"
arch=('x86_64' 'aarch64')
url="https://github.com/prj786/ewe-sync"
license=('MIT')

depends=(
  'webkit2gtk-4.1'
  'gtk3'
  'libayatana-appindicator'   # the tray icon (StatusNotifierItem)
  'python'                    # the ewe tools it drives are Python
  'libsecret'                 # secret-tool — the app password lives in the keyring
  # nextcloudcmd — the two-way folder-sync engine (RFC-006 F2). The package's
  # GUI (/usr/bin/nextcloud) is deliberately NOT part of the desktop: ewe ships
  # a desktop-entry override that hides it, so ewe-sync is the only sync app a
  # user ever sees. The dependency stays because nextcloudcmd ships nowhere
  # else, and writing a two-way sync engine is the Nextcloud client's whole
  # multi-year job (RFC-006, "Folder sync engine").
  'nextcloud-client'
)
optdepends=(
  'ewe: the desktop this is the account app of (ewe-cloud, ewe-conf)'
  'rclone: one-way (upload / download only) folders'
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

  install -Dm755 src-tauri/target/release/ewe-sync "$pkgdir/usr/bin/ewe-sync"

  install -Dm644 packaging/io.github.prj786.ewe-sync.desktop \
    "$pkgdir/usr/share/applications/io.github.prj786.ewe-sync.desktop"
  for s in 32 64 128 256; do
    install -Dm644 "src-tauri/icons/${s}x${s}.png" \
      "$pkgdir/usr/share/icons/hicolor/${s}x${s}/apps/io.github.prj786.ewe-sync.png"
  done
  install -Dm644 packaging/ewe-sync.svg \
    "$pkgdir/usr/share/icons/hicolor/scalable/apps/io.github.prj786.ewe-sync.svg"

  # Autostart: ewe's session activates graphical-session.target, so a user
  # unit wanted by it starts ewe-sync hidden (tray only) at every login. The unit
  # is installed but not enabled — ewe's `ewe-setup` enables it for the user
  # (RFC-006 F3); until then `systemctl --user enable --now ewe-sync.service`.
  install -Dm644 packaging/ewe-sync.service \
    "$pkgdir/usr/lib/systemd/user/ewe-sync.service"

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
