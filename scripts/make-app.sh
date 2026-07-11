#!/bin/sh
# Assemble dist/EWM.app -- a self-contained, double-clickable macOS bundle.
#
#   scripts/make-app.sh
#
# Phase 1 of notes/MAC_APP.md: static SDL3 (no Homebrew dependency at run
# time; CMake required at build time), the icon rendered by the emulator's
# own character generator, ad-hoc code signing, and Finder document types
# for the disk-image extensions. Real signing/notarization is Phase 2.
set -e
cd "$(dirname "$0")/.."

VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d '"' -f 2)
DIST=dist
APP="$DIST/EWM.app"

echo "==> generating the icon (examples/icon.rs)"
cargo run -q -p ewm --example icon -- "$DIST/EWM.iconset"

echo "==> building the release binary (static SDL3; first build compiles SDL from source)"
cargo build --release -p ewm --no-default-features --features sdl-static

echo "==> assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/ewm "$APP/Contents/MacOS/ewm"
iconutil -c icns -o "$APP/Contents/Resources/EWM.icns" "$DIST/EWM.iconset"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleIdentifier</key>
	<string>ca.arentz.ewm</string>
	<key>CFBundleName</key>
	<string>EWM</string>
	<key>CFBundleExecutable</key>
	<string>ewm</string>
	<key>CFBundleIconFile</key>
	<string>EWM</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>LSMinimumSystemVersion</key>
	<string>12.0</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.games</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>CFBundleDocumentTypes</key>
	<array>
		<dict>
			<key>CFBundleTypeName</key>
			<string>Apple II Floppy Disk Image</string>
			<key>CFBundleTypeExtensions</key>
			<array>
				<string>dsk</string>
				<string>do</string>
				<string>po</string>
				<string>nib</string>
				<string>woz</string>
			</array>
			<key>CFBundleTypeRole</key>
			<string>Viewer</string>
			<key>LSHandlerRank</key>
			<string>Alternate</string>
		</dict>
		<dict>
			<key>CFBundleTypeName</key>
			<string>ProDOS Hard Drive Image</string>
			<key>CFBundleTypeExtensions</key>
			<array>
				<string>hdv</string>
			</array>
			<key>CFBundleTypeRole</key>
			<string>Viewer</string>
			<key>LSHandlerRank</key>
			<string>Alternate</string>
		</dict>
	</array>
</dict>
</plist>
PLIST
plutil -lint "$APP/Contents/Info.plist"

echo "==> code signing (ad hoc)"
codesign --force -s - "$APP"

echo "==> verifying the binary is self-contained"
if otool -L "$APP/Contents/MacOS/ewm" | grep -q '/opt/homebrew\|/usr/local'; then
	echo "ERROR: the binary links non-system libraries:"
	otool -L "$APP/Contents/MacOS/ewm"
	exit 1
fi

echo "==> done: $APP"
