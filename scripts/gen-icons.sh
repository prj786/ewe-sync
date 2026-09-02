#!/usr/bin/env bash
# Rasterise the SVG art into the PNGs Tauri and the package need.
#   packaging/flock.svg        → src-tauri/icons/{32,64,128,256}x*.png, icon.png,
#                                128x128@2x.png (Tauri's expected names)
#   packaging/tray/<state>.svg → src-tauri/icons/tray-<state>.png (22 px, the
#                                StatusNotifierItem size the ewe bar shows)
# Needs rsvg-convert (librsvg). Run after editing the SVGs; the PNGs are
# committed so a checkout builds without librsvg.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v rsvg-convert >/dev/null || { echo "rsvg-convert (librsvg) is required" >&2; exit 1; }
out=src-tauri/icons
mkdir -p "$out"
for s in 32 64 128 256; do
    rsvg-convert -w "$s" -h "$s" packaging/flock.svg -o "$out/${s}x${s}.png"
done
cp "$out/256x256.png" "$out/128x128@2x.png"
rsvg-convert -w 512 -h 512 packaging/flock.svg -o "$out/icon.png"
for f in packaging/tray/*.svg; do
    n=$(basename "$f" .svg)
    rsvg-convert -w 22 -h 22 "$f" -o "$out/tray-$n.png"
    rsvg-convert -w 44 -h 44 "$f" -o "$out/tray-$n@2x.png"
done
ls -1 "$out"
