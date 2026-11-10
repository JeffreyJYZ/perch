#!/bin/sh
# Checks a staged `dist/Perch.app` and prints one verdict per check.
#
# Every expected value is read from its source of truth rather than repeated here: the version
# from the crate, the bundle id from the binary's own `APP_ID`, the architecture from the Mach-O
# itself — so a plist that drifted from the code fails rather than agreeing with a copy.
#
#   scripts/verify-bundle.sh [path/to/Perch.app]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
app=${1:-$root/dist/Perch.app}
plist=$app/Contents/Info.plist
binary=$app/Contents/MacOS/perch
icon=$app/Contents/Resources/Perch.icns

status=0
pass() { printf 'ok    %s\n' "$1"; }
fail() {
	printf 'FAIL  %s\n' "$1"
	status=1
}

# read <key>: one raw plist value, or nothing when the key or the plist is absent.
read_key() {
	plutil -extract "$1" raw -o - "$plist" 2>/dev/null || true
}

want_id=$(awk -F'"' '/^const APP_ID/ { print $2; exit }' "$root/crates/perch-app/src/main.rs")
want_version=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
	tr ',' '\n' |
	awk '/"name":"perch-app"/ { app = 1 } app && /"version"/ { gsub(/[^0-9.]+/, ""); print; exit }')

if [ -d "$app" ]; then
	pass "bundle exists: $app"
else
	fail "bundle missing: $app"
fi

if [ -f "$plist" ] && plutil -lint "$plist" >/dev/null 2>&1; then
	pass "Info.plist is a valid property list"
else
	fail "Info.plist missing or malformed: $plist"
fi

got_id=$(read_key CFBundleIdentifier)
if [ "$got_id" = "$want_id" ]; then
	pass "CFBundleIdentifier is $got_id"
else
	fail "CFBundleIdentifier is '${got_id:-missing}', want '$want_id'"
fi

got_version=$(read_key CFBundleShortVersionString)
if [ "$got_version" = "$want_version" ]; then
	pass "CFBundleShortVersionString is $got_version"
else
	fail "CFBundleShortVersionString is '${got_version:-missing}', want '$want_version'"
fi

got_name=$(read_key CFBundleName)
if [ "$got_name" = "Perch" ]; then
	pass "CFBundleName is Perch"
else
	fail "CFBundleName is '${got_name:-missing}', want 'Perch'"
fi

got_exec=$(read_key CFBundleExecutable)
if [ -n "$got_exec" ] && [ -x "$app/Contents/MacOS/$got_exec" ]; then
	pass "CFBundleExecutable is $got_exec and it is executable"
else
	fail "CFBundleExecutable is '${got_exec:-missing}' and no executable file matches it"
fi

if codesign --verify --deep --strict "$app" >/dev/null 2>&1; then
	pass "codesign --verify --deep --strict passes"
else
	fail "codesign --verify --deep --strict fails: $(codesign --verify --deep --strict "$app" 2>&1 | head -1)"
fi

kind=$(file -b "$binary" 2>/dev/null || true)
case $kind in
*Mach-O*executable*) pass "binary is a Mach-O executable: $kind" ;;
*) fail "binary is not a Mach-O executable: ${kind:-missing}" ;;
esac

arches=$(lipo -archs "$binary" 2>/dev/null || true)
if [ "$arches" = "arm64" ]; then
	pass "binary architectures: $arches"
else
	fail "binary architectures: '${arches:-unknown}', want arm64"
fi

if [ -s "$icon" ] && sips -g pixelWidth "$icon" 2>/dev/null | grep -q 'pixelWidth: 1024'; then
	pass "icon is non-empty and decodes at 1024 px: $(wc -c <"$icon" | tr -d ' ') bytes"
else
	fail "icon missing, empty or undecodable: $icon"
fi

if [ "$status" -eq 0 ]; then
	printf 'verify-bundle: all checks passed\n'
else
	printf 'verify-bundle: failures above\n'
fi
exit "$status"
