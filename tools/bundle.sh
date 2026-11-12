#!/bin/sh
# Builds target/Pomodoro.app from the release binary and the lantern iconset, signs it ad hoc,
# checks the signature and the Info.plist, and packs it into target/Pomodoro.dmg beside a link
# to /Applications. The bundle is what gives the app its Dock icon and lets macOS deliver its
# notifications; a bare `cargo run` binary has neither.
set -eu
cd "$(dirname "$0")/.."

ID=$(sed -n 's/^const APP_ID: &str = "\(.*\)";$/\1/p' src/main.rs)
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$ID" ] && [ -n "$VERSION" ] || { echo "bundle.sh: no APP_ID in src/main.rs or version in Cargo.toml" >&2; exit 1; }

APP=target/Pomodoro.app
cargo build --locked --release
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/pomodoro "$APP/Contents/MacOS/pomodoro"
iconutil -c icns assets/icon/sunlantern.iconset -o "$APP/Contents/Resources/Pomodoro.icns"
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>Pomodoro</string>
	<key>CFBundleExecutable</key>
	<string>pomodoro</string>
	<key>CFBundleIconFile</key>
	<string>Pomodoro</string>
	<key>CFBundleIdentifier</key>
	<string>$ID</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Pomodoro</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.productivity</string>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF
plutil -lint "$APP/Contents/Info.plist"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"

DMG=target/dmg
rm -rf "$DMG" target/Pomodoro.dmg
mkdir -p "$DMG"
cp -R "$APP" "$DMG/"
ln -s /Applications "$DMG/Applications"
hdiutil create -quiet -volname Pomodoro -srcfolder "$DMG" -format UDZO target/Pomodoro.dmg
rm -rf "$DMG"
echo "built $APP ($ID $VERSION) and target/Pomodoro.dmg"
