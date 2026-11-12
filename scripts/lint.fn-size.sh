#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$(realpath "$0")")/.."

print_help() {
  cat <<-EOF

	Keeps the functions pinned with #[expect(clippy::<size lint>)] from growing.

	Clippy only checks that a pinned function is still over the limit in
	clippy.toml, not by how much. This script measures each pinned function and
	fails if it differs from its value in BASELINE: above it, split the
	function; below it, lower the value (or, once clippy no longer fires,
	delete the entry together with the #[expect]).

	EOF
}

[[ "${1:-}" != "--help" ]] || {
  print_help
  exit 0
}

# Baseline: "path function lint value", one per line. Only lower these values.
BASELINE="
src/minter/src/api/admin/api.rs update_reserve_impl too_many_lines 66
src/minter/src/api/admin/api.rs update_reserve_impl cognitive_complexity 14
src/minter/src/api/admin/api.rs validate_manual_topup_inputs too_many_arguments 6
src/minter/src/api/admin/api.rs manual_topup_reserve_impl too_many_lines 118
src/minter/src/services/rebalance.rs rebalance_one too_many_lines 103
src/minter/src/services/reserve.rs validate_balance_invariants too_many_arguments 6
src/minter/src/services/reserve.rs compute_rebalance too_many_lines 87
src/minter/src/services/reserve.rs compute_rebalance cognitive_complexity 13
"

# Measure with every size threshold at 1 so clippy reports each function's value.
conf_dir="$(mktemp -d)"
trap 'rm -rf "$conf_dir"' EXIT
sed -E 's/^(too-many-lines|cognitive-complexity|too-many-arguments)-threshold = .*/\1-threshold = 1/' \
  clippy.toml >"$conf_dir/clippy.toml"

report="$(
  CLIPPY_CONF_DIR="$conf_dir" cargo clippy --locked -p minter \
    --target wasm32-unknown-unknown --message-format short -- \
    --force-warn clippy::too_many_lines \
    --force-warn clippy::cognitive_complexity \
    --force-warn clippy::too_many_arguments 2>&1 || true
)"

# One line per measurement: "path line lint value".
measurements="$(
  sed -nE \
    -e 's/^([^:]+):([0-9]+):[0-9]+: (warning|error): this function has too many lines \(([0-9]+)\/1\).*/\1 \2 too_many_lines \4/p' \
    -e 's/^([^:]+):([0-9]+):[0-9]+: (warning|error): the function has a cognitive complexity of \(([0-9]+)\/1\).*/\1 \2 cognitive_complexity \4/p' \
    -e 's/^([^:]+):([0-9]+):[0-9]+: (warning|error): this function has too many arguments \(([0-9]+)\/1\).*/\1 \2 too_many_arguments \4/p' \
    <<<"$report"
)"

[[ -n "$measurements" ]] || {
  echo "ERROR: clippy reported no function sizes; output was:"
  echo "$report"
  exit 1
}

# The name of the function whose span holds the reported line: the nearest `fn` at or above it.
function_at() {
  local file=$1 line=$2
  head -n "$line" "$file" | sed -nE 's/.*fn ([a-z_][a-z0-9_]*)[<(].*/\1/p' | tail -n 1
}

errors=0

while read -r file fn lint value; do
  [[ -n "$file" ]] || continue
  measured=""
  while read -r m_file m_line m_lint m_value; do
    [[ "$m_file" == "$file" && "$m_lint" == "$lint" ]] || continue
    [[ "$(function_at "$m_file" "$m_line")" == "$fn" ]] || continue
    measured="$m_value"
  done <<<"$measurements"

  if [[ -z "$measured" ]]; then
    echo "ERROR: $file $fn: no $lint value measured. Was it renamed or removed? Update $0."
    errors=$((errors + 1))
  elif ((measured > value)); then
    echo "ERROR: $file $fn: $lint is $measured, over its pinned $value. Split the function."
    errors=$((errors + 1))
  elif ((measured < value)); then
    echo "ERROR: $file $fn: $lint is down to $measured. Lower its pin from $value in $0."
    errors=$((errors + 1))
  fi
done <<<"$BASELINE"

((errors == 0))
