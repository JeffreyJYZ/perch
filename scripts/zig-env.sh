#!/bin/sh
# Source this, do not execute it:  . scripts/zig-env.sh
#
# `libghostty-vt-sys` 0.2.2 vendors Ghostty at a887df42, whose build.zig.zon
# sets `minimum_zig_version = "0.15.2"`. The linked `zig` here is 0.17.1 and is
# refused at configure time, so the terminal crate builds only with the
# keg-only 0.15.2, and only inside a shell that put that keg on PATH.
#
# Sourcing does three things, all idempotent:
#   1. puts Zig 0.15.2 first on PATH and exports ZIG (never `brew link`);
#   2. probes whether that Zig can link against the installed macOS SDK;
#   3. when it cannot, writes an xcrun/SDK shim into the gitignored .zig-env/.
#
# The probe answer is cached in .zig-env/state, keyed to the Zig binary and the
# SDK, so a second `source` exports the same environment without rebuilding.

if [ -n "${ZSH_VERSION-}" ]; then
	# zsh points $0 at the sourced file too, so the eval context is what tells
	# a `source` from an executed script.
	case ${ZSH_EVAL_CONTEXT-} in
	*:file*) eval '_zig_self=${(%):-%x}' ;;
	*)
		printf 'zig-env: source this file, do not execute it\n' >&2
		return 1 2>/dev/null || exit 1
		;;
	esac
elif [ -n "${BASH_SOURCE-}" ]; then
	if [ "${BASH_SOURCE[0]}" = "$0" ]; then
		printf 'zig-env: source this file, do not execute it\n' >&2
		return 1 2>/dev/null || exit 1
	fi
	_zig_self=${BASH_SOURCE[0]}
else
	_zig_self=$0
fi
_zig_root=$(cd "$(dirname "$_zig_self")/.." && pwd)
_zig_env=$_zig_root/.zig-env

# Resolve Zig: an explicit PERCH_ZIG wins, then the keg `brew install zig@0.15`
# creates, then any 0.15.2 already on PATH. The version is checked either way —
# a different Zig cannot build the pinned Ghostty, so it is not a fallback.
_zig_bin=${PERCH_ZIG-}
if [ -z "$_zig_bin" ]; then
	_zig_keg=
	if command -v brew >/dev/null 2>&1; then
		_zig_keg=$(brew --prefix zig@0.15 2>/dev/null || true)
	fi
	for _zig_candidate in "$_zig_keg" /opt/homebrew/opt/zig@0.15 /usr/local/opt/zig@0.15; do
		[ -n "$_zig_candidate" ] || continue
		if [ -x "$_zig_candidate/bin/zig" ]; then
			_zig_bin=$_zig_candidate/bin/zig
			break
		fi
	done
fi
if [ -z "$_zig_bin" ]; then
	_zig_bin=$(command -v zig 2>/dev/null || true)
fi
case $_zig_bin in
*/*) ;;
*) _zig_bin=$(command -v "$_zig_bin" 2>/dev/null || true) ;;
esac
if [ -z "$_zig_bin" ] || [ "$("$_zig_bin" version 2>/dev/null)" != 0.15.2 ]; then
	printf 'zig-env: Zig 0.15.2 not found; run `brew install zig@0.15`\n' >&2
	return 1
fi
_zig_dir=$(cd "$(dirname "$_zig_bin")" && pwd)
_zig_bin=$_zig_dir/$(basename "$_zig_bin")
# Zig's own macOS headers: its bundled libcxx expects these, and the SDK's
# current headers are newer than 0.15.2's libcxx understands.
_zig_lib=$("$_zig_bin" env 2>/dev/null | awk -F'"' '/\.lib_dir/ { print $2 }')
_zig_include=$_zig_lib/libc/include/any-macos-any
if [ ! -d "$_zig_include" ]; then
	printf 'zig-env: Zig macOS headers not found under %s\n' "$_zig_lib" >&2
	return 1
fi

# The SDK is the one `xcode-select` points at, so the probe and the shim read
# the same files. `xcrun --show-sdk-path` is the fallback: it can answer with
# the Command Line Tools path, which is not always a location xcodebuild
# accepts.
_zig_sdk=
_zig_dev=$(xcode-select -p 2>/dev/null || true)
if [ -n "$_zig_dev" ] && [ -f "$_zig_dev/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/usr/lib/libSystem.B.tbd" ]; then
	_zig_sdk=$_zig_dev/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
else
	_zig_sdk=$(xcrun --show-sdk-path 2>/dev/null || true)
fi
if [ -z "$_zig_sdk" ] || [ ! -f "$_zig_sdk/usr/lib/libSystem.B.tbd" ]; then
	printf 'zig-env: no macOS SDK with libSystem stubs found\n' >&2
	return 1
fi

_zig_key=zig-env-1:0.15.2:${_zig_bin}:${_zig_sdk}:$(/usr/bin/stat -f %m "$_zig_sdk/usr/lib/libSystem.B.tbd" 2>/dev/null || printf 0)

# A link is the operation that breaks: Zig 0.15.2 asks xcrun for an SDK and
# takes its libSystem stubs, and macOS 27 lists only arm64e in them. Compiling
# alone would pass and prove nothing.
_zig_probe() {
	_zig_probe_bin=$_zig_env/probe.$$
	if ! "$_zig_bin" cc "$_zig_env/probe.c" -o "$_zig_probe_bin" >"$_zig_probe_bin.log" 2>&1; then
		rm -f "$_zig_probe_bin"
		return 1
	fi
	"$_zig_probe_bin" >/dev/null 2>&1
	_zig_probe_ok=$?
	rm -f "$_zig_probe_bin" "$_zig_probe_bin.log"
	return $_zig_probe_ok
}

# The shim is the real SDK with an arm64 slice added to its libSystem stubs,
# plus an xcrun that answers the SDK probe with it. Adding the slice keeps
# every arm64e entry, so the same SDK still serves the system linker.
_zig_build_shim() {
	_zip_shim=$_zig_env/sdk
	mkdir -p "$_zip_shim/usr/lib" "$_zig_env/bin"
	ln -sfn "$_zig_sdk/System" "$_zip_shim/System"
	ln -sfn "$_zig_include" "$_zip_shim/usr/include"
	cp "$_zig_sdk/SDKSettings.json" "$_zip_shim/SDKSettings.json" 2>/dev/null || true
	for _zig_entry in "$_zig_sdk/usr/lib"/*; do
		ln -sfn "$_zig_entry" "$_zip_shim/usr/lib/$(basename "$_zig_entry")"
	done
	rm -f "$_zip_shim/usr/lib/libSystem.tbd" "$_zip_shim/usr/lib/libSystem.B.tbd"
	awk '{ gsub(/arm64e-macos/, "arm64-macos, arm64e-macos"); print }' \
		"$_zig_sdk/usr/lib/libSystem.B.tbd" >"$_zip_shim/usr/lib/libSystem.tbd"
	cp "$_zip_shim/usr/lib/libSystem.tbd" "$_zip_shim/usr/lib/libSystem.B.tbd"
	cat >"$_zig_env/bin/xcrun" <<EOF
#!/bin/sh
# Answers Zig's SDK probe with the shim SDK, whose libSystem stubs carry an
# arm64 slice; every other query is the real xcrun.
for _zig_arg in "\$@"; do
	case \$_zig_arg in
	--show-sdk-path) printf '%s\n' "$_zip_shim"; exit 0 ;;
	esac
done
exec /usr/bin/xcrun "\$@"
EOF
	chmod +x "$_zig_env/bin/xcrun"
}

mkdir -p "$_zig_env"
[ -f "$_zig_env/probe.c" ] || printf '#include <stdio.h>\nint main(void) { puts("zig-env"); return 0; }\n' >"$_zig_env/probe.c"

_zig_mode=
if [ -f "$_zig_env/state" ]; then
	case $(cat "$_zig_env/state") in
	"${_zig_key}:direct") _zig_mode=direct ;;
	"${_zig_key}:shim") _zig_mode=shim ;;
	esac
fi

if [ -z "$_zig_mode" ]; then
	_zig_ambient=$PATH
	PATH=${_zig_dir}:${PATH}
	_zig_mode=direct
	if ! _zig_probe; then
		_zig_mode=shim
		_zig_build_shim
		PATH=${_zig_env}/bin:${_zig_dir}:${PATH}
		if ! _zig_probe; then
			PATH=${_zig_ambient}
			printf 'zig-env: Zig 0.15.2 cannot link against %s, and the SDK shim did not help\n' \
				"$_zig_sdk" >&2
			return 1
		fi
	fi
	PATH=${_zig_ambient}
	printf '%s\n' "${_zig_key}:${_zig_mode}" >"$_zig_env/state.$$"
	mv "$_zig_env/state.$$" "$_zig_env/state"
fi

# Rebuild PATH from scratch each time: sourcing twice must not stack entries.
_zig_rest=$(printf '%s' "$PATH" | tr ':' '\n' | awk -v zig="$_zig_dir" -v shim="$_zig_env/bin" '
	$0 == "" || $0 == zig || $0 == shim { next }
	{ rest = rest == "" ? $0 : rest ":" $0 }
	END { printf "%s", rest }')
if [ "$_zig_mode" = shim ]; then
	PATH=${_zig_env}/bin:${_zig_dir}${_zig_rest:+:${_zig_rest}}
	SDKROOT=$_zig_env/sdk
else
	PATH=${_zig_dir}${_zig_rest:+:${_zig_rest}}
	SDKROOT=$_zig_sdk
fi
export PATH SDKROOT ZIG=$_zig_bin

printf 'zig-env: Zig 0.15.2 from %s (%s SDK)\n' "$_zig_bin" "$_zig_mode"

unset -f _zig_probe _zig_build_shim
unset _zig_self _zig_root _zig_env _zig_keg _zig_candidate _zig_dev _zig_sdk \
	_zig_lib _zig_include _zig_key _zig_mode _zig_ambient _zig_rest _zig_dir _zig_bin \
	_zig_entry _zig_probe_bin _zig_probe_ok
