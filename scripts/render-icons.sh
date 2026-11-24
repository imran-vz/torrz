#!/bin/sh
# Rasterises assets/icon.svg and assets/icon-dark.svg to the PNGs the app embeds for its Dock
# icon (assets/icon-light.png, assets/icon-dark.png). Needs Google Chrome; the PNGs are checked
# in, so only rerun this after scripts/make-icon.py changes the artwork.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -x "$chrome" ] || { echo "render-icons: Google Chrome is needed to rasterise the SVGs" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
render() {
  printf '<!doctype html><body style="margin:0;background:transparent"><img src="file://%s" width="1024" height="1024" style="display:block">' "$1" > "$work/icon.html"
  "$chrome" --headless --disable-gpu --hide-scrollbars --default-background-color=00000000 \
    --window-size=1024,1024 --screenshot="$2" "file://$work/icon.html" >/dev/null 2>&1
  echo "$2"
}
render "$root/assets/icon.svg" "$root/assets/icon-light.png"
render "$root/assets/icon-dark.svg" "$root/assets/icon-dark.png"
