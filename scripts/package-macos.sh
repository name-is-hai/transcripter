#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 TRANSCRIPTER_BINARY ARCH OUTPUT.tar.gz" >&2
  exit 2
fi

binary=$1
architecture=$2
output=$3
version=${RELEASE_VERSION:-0.1.0}
version=${version#v}
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
app="$work/Transcripter.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/transcripter"
chmod 755 "$app/Contents/MacOS/transcripter"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Transcripter</string>
  <key>CFBundleExecutable</key><string>transcripter</string>
  <key>CFBundleIdentifier</key><string>dev.transcripter.app</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Transcripter</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

codesign --force --deep --sign - "$app"
mkdir -p "$(dirname "$output")"
tar -C "$work" -czf "$output" Transcripter.app
printf 'Created %s for %s\n' "$output" "$architecture"
