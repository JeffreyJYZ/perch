#!/bin/sh
# Builds the release binary and stages a signed `dist/Perch.app`.
#
# The bundle is assembled by hand rather than by a bundler: perch is one executable, one plist and
# one icon, and `cargo` already knows how to build the executable. Signing is ad-hoc (`-`), which
# is enough for the app to launch on this machine; a distributed build needs a Developer ID
# identity and notarization, which this script does not do.
#
#   scripts/bundle.sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

# Sourcing the Zig environment is what makes perch-term build at all: its vendored Ghostty is
# pinned to Zig 0.15.2, and a clean shell has 0.17 on PATH.
if [ -f "$root/scripts/zig-env.sh" ]; then
	. "$root/scripts/zig-env.sh"
fi

# Its own target dir, so a bundle build never invalidates the dev tree's artefacts (or the other
# way round) and the release profile does not sit in the default `target/`.
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/target-package}
export CARGO_TARGET_DIR

app=$root/dist/Perch.app
contents=$app/Contents
resources=$contents/Resources
binary=$CARGO_TARGET_DIR/release/perch

# The crate's own version, so the plist cannot drift from Cargo.toml.
version=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
	tr ',' '\n' |
	awk '/"name":"perch-app"/ { app = 1 } app && /"version"/ { gsub(/[^0-9.]+/, ""); print; exit }')
if [ -z "$version" ]; then
	printf 'bundle: cannot read perch-app version from Cargo.toml\n' >&2
	exit 1
fi

# The bundle id is the binary's own application id, used for its data directory and keychain
# service, so the plist takes it from there instead of carrying a second copy. Read it from
# wherever the const lives — it moved once already, from the binary's root to its boot module.
bundle_id=$(rg --no-filename -o 'const APP_ID: &str = "([^"]+)"' -r '$1' "$root/crates/perch-app/src" | head -1)
if [ -z "$bundle_id" ]; then
	printf 'bundle: cannot find `const APP_ID` under crates/perch-app/src\n' >&2
	exit 1
fi

printf 'bundle: building perch-app %s (release)\n' "$version"
cargo build --release -p perch-app

if [ ! -x "$binary" ]; then
	printf 'bundle: %s was not produced\n' "$binary" >&2
	exit 1
fi

# The deployment target is the binary's own: taking it from the Mach-O load command means the
# plist cannot promise an older macOS than the code was built for.
minimum=$(vtool -show-build "$binary" | awk '/^ *minos/ { print $2; exit }')
if [ -z "$minimum" ]; then
	minimum=11.0
fi

rm -rf "$app"
mkdir -p "$contents/MacOS" "$resources"
install -m 755 "$binary" "$contents/MacOS/perch"

# The icon is drawn, never downloaded: `make-icon.swift` renders the 1024 plate and `sips` fills
# the sizes `iconutil` expects. The compiled helper lives with the other build artefacts, so no
# new directory appears next to the source tree.
helper=$CARGO_TARGET_DIR/icon-tools/make-icon
if [ ! -x "$helper" ] || [ "$root/scripts/make-icon.swift" -nt "$helper" ]; then
	mkdir -p "$CARGO_TARGET_DIR/icon-tools"
	xcrun swiftc -O -o "$helper" "$root/scripts/make-icon.swift"
fi
"$helper" "$root/assets/icon-1024.png"

iconset=$root/dist/Perch.iconset
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
	sips -z "$size" "$size" "$root/assets/icon-1024.png" \
		--out "$iconset/icon_${size}x${size}.png" >/dev/null
	double=$((size * 2))
	sips -z "$double" "$double" "$root/assets/icon-1024.png" \
		--out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$resources/Perch.icns"
rm -rf "$iconset"

cat >"$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>perch</string>
	<key>CFBundleIconFile</key>
	<string>Perch</string>
	<key>CFBundleIdentifier</key>
	<string>$bundle_id</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Perch</string>
	<key>CFBundleDisplayName</key>
	<string>Perch</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>$minimum</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
PLIST
plutil -lint "$contents/Info.plist" >/dev/null

# Ad-hoc: no identity, no timestamp, no notarization — a local build, not a shipped one.
codesign --force --sign - --identifier "$bundle_id" "$app"
codesign --verify --deep --strict "$app"

printf 'bundle: %s\n' "$app"
printf 'bundle: id %s, version %s, macOS %s, ad-hoc signed\n' "$bundle_id" "$version" "$minimum"
