#!/bin/sh
# Renders og/og.html to public/og.png (1200×630) with headless Chrome. macOS paths; set CHROME
# to use another Chrome or Chromium binary.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
profile=$(mktemp -d)
trap 'rm -rf "$profile"' EXIT
"$chrome" --headless=new --disable-gpu --hide-scrollbars --no-first-run \
  --user-data-dir="$profile" --force-device-scale-factor=1 --window-size=1200,630 \
  --virtual-time-budget=3000 --screenshot="$here/../public/og.png" "file://$here/og.html" >/dev/null 2>&1
echo "$here/../public/og.png"
