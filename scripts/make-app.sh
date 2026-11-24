#!/bin/sh
# Builds dist/Torrz.app: the release binaries, the light/dark icon compiled from
# assets/Torrz.icon, and an Info.plist that registers magnet: links. macOS only; needs Xcode's
# actool. Sign with your own identity by setting TORRZ_SIGN (default: ad hoc).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
app="$root/dist/Torrz.app"

cargo build --release -p torrz-app -p torrz-cli

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/torrz-app target/release/torrz "$app/Contents/MacOS/"

# Assets.car holds the icon's light, dark and tinted looks, which macOS 26 and later switch
# between themselves; Torrz.icns is what older systems show.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
xcrun actool "$root/assets/Torrz.icon" --compile "$app/Contents/Resources" --app-icon Torrz \
  --platform macosx --minimum-deployment-target 12.0 --include-all-app-icons \
  --output-partial-info-plist "$work/icon.plist" >/dev/null

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Torrz</string>
  <key>CFBundleDisplayName</key><string>Torrz</string>
  <key>CFBundleIdentifier</key><string>app.torrz.Torrz</string>
  <key>CFBundleExecutable</key><string>torrz-app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleIconFile</key><string>Torrz</string>
  <key>CFBundleIconName</key><string>Torrz</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>BitTorrent magnet link</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleURLSchemes</key><array><string>magnet</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist" >/dev/null

codesign --force --sign "${TORRZ_SIGN:--}" "$app/Contents/MacOS/torrz" >/dev/null 2>&1
codesign --force --sign "${TORRZ_SIGN:--}" "$app" >/dev/null 2>&1
codesign --verify --strict "$app"
echo "$app"
