#!/bin/sh
# A source file has a budget, not a limit: 200 lines, and a file already over it may only shrink.
#
# docs/refactor/oversized.txt records every offender and its size today. Adding a file to that
# list is not allowed, so a new file over 200 fails, and a listed file that grows by even one line
# fails. A listed file that drops under the cap should leave the list — the script says so without
# failing, because deleting the entry is a person's decision made once.
set -eu

cap=200
baseline=docs/refactor/oversized.txt

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

status=0

size_of() {
	wc -l <"$1" | tr -d ' '
}

baselined() {
	[ -f "$baseline" ] || return 0
	awk -v path="$1" '$1 == path { print $2; found = 1 } END { exit !found }' "$baseline"
}

found=$(mktemp)
trap 'rm -f "$found"' EXIT

for file in $(find crates -path '*/src/*' -name '*.rs' -type f | sort); do
	lines=$(size_of "$file")
	[ "$lines" -le "$cap" ] && continue

	if allowed=$(baselined "$file"); then
		if [ "$lines" -gt "$allowed" ]; then
			printf 'grew: %s is %s lines, was %s\n' "$file" "$lines" "$allowed"
			status=1
		fi
		printf '%s\n' "$file" >>"$found"
	else
		printf 'new offender: %s is %s lines, cap is %s\n' "$file" "$lines" "$cap"
		status=1
	fi
done

if [ -f "$baseline" ]; then
	while read -r path allowed; do
		[ -n "${path:-}" ] || continue
		case "$path" in \#*) continue ;; esac
		if [ ! -f "$path" ]; then
			printf 'stale: %s no longer exists, drop it from %s\n' "$path" "$baseline"
		elif [ "$(size_of "$path")" -le "$cap" ]; then
			printf 'shrunk: %s is under the cap now, drop it from %s\n' "$path" "$baseline"
		fi
	done <"$baseline"
fi

if [ "$status" -eq 0 ]; then
	count=0
	[ -f "$baseline" ] && count=$(grep -vc '^#' "$baseline" || true)
	printf 'file size: ok (%s file(s) over %s lines, none grew)\n' "$count" "$cap"
fi

exit "$status"
