#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$(realpath "$0")")/.."

print_help() {
  cat <<-EOF

	Checks that no Rust source file grows past its line cap.

	New files get MAX_LINES. Files already over it are pinned at their size when
	the gate was added (rounded up to the next 50) and may only go down: lower the
	cap when a file shrinks, and remove the entry once it is under MAX_LINES.

	EOF
}

[[ "${1:-}" != "--help" ]] || {
  print_help
  exit 0
}

MAX_LINES=400

# Baseline: "path cap", one per line. Only lower these numbers.
BASELINE="
src/minter/src/services/reserve.rs 1000
src/minter/src/api/admin/api.rs 600
"

cap_for() {
  local file=$1
  local cap
  cap="$(awk -v f="$file" '$1 == f { print $2 }' <<<"$BASELINE")"
  echo "${cap:-$MAX_LINES}"
}

errors=0

while IFS= read -r file; do
  lines="$(wc -l <"$file" | tr -d ' ')"
  cap="$(cap_for "$file")"
  if ((lines > cap)); then
    echo "ERROR: $file has $lines lines, over its cap of $cap. Split it."
    errors=$((errors + 1))
  fi
done < <(git ls-files 'src/*.rs')

while read -r file cap; do
  [[ -n "$file" ]] || continue
  if [[ ! -f "$file" ]]; then
    echo "ERROR: baseline entry $file no longer exists. Remove it from $0."
    errors=$((errors + 1))
    continue
  fi
  lines="$(wc -l <"$file" | tr -d ' ')"
  if ((lines <= MAX_LINES)); then
    echo "ERROR: $file is down to $lines lines, under $MAX_LINES. Remove its baseline entry from $0."
    errors=$((errors + 1))
  elif ((cap - lines >= 50)); then
    echo "ERROR: $file is down to $lines lines; lower its baseline cap from $cap in $0."
    errors=$((errors + 1))
  fi
done <<<"$BASELINE"

((errors == 0))
