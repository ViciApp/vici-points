#!/usr/bin/env bash
#
# Initialize minter reserves for Vici XP (gameplay token).
#
# XP uses 6 gameplay-only reserves (no corporate allocations):
#
#   Bucket        Cap (XP)   %      Auto-rebalance
#   ─────────────────────────────────────────────────
#   forecast      400M       40%    yes
#   onboarding    200M       20%    yes
#   streaks       200M       20%    yes
#   leaderboard   100M       10%    yes
#   campaign       50M        5%    yes
#   buffer         50M        5%    no (manual only)
#
# Auto-rebalancing reserves are configured with:
#   - target_balance          = 7 days of daily emission budget
#   - min_balance             = 2 days of daily emission budget
#   - max_topup_per_rebalance = target_balance
#   - rate_limits             = 2x daily budget per day,
#                               1x yearly budget per year
#
# The minter timer (1-hour interval) checks all reserves and refills
# any that have dropped below target_balance — subject to all caps
# and rate limits.
#
# Prerequisites:
#   - bash, dfx
#   - Minter deployed; caller must be minter controller
#
# Supply six unique ICRC-1 owner principals via environment variables.
# See scripts/init.reserves.config.example.sh for the full list.
#
# Optional:
#   DFX_NETWORK (default: local)
#   DFX_IDENTITY — passed to dfx as --identity if set
#   DECIMALS — must match ledger token decimals (default: 8)
#   DRY_RUN=1 — print would-be dfx calls, do not execute
#   MINTER_CANISTER — canister name for dfx (default: minter)
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

DFX_NETWORK="${DFX_NETWORK:-local}"
DECIMALS="${DECIMALS:-8}"
MINTER_CANISTER="${MINTER_CANISTER:-minter}"
DRY_RUN="${DRY_RUN:-0}"

case "${DECIMALS}" in
'' | *[!0-9]*)
  echo "ERROR: DECIMALS must be a non-negative integer (got '${DECIMALS}')" >&2
  exit 1
  ;;
esac

pow10() {
  local d="$1"
  local r=1
  local i
  for ((i = 0; i < d; i++)); do
    r=$((r * 10))
  done
  echo "${r}"
}

MULT="$(pow10 "${DECIMALS}")"

# ---------------------------------------------------------------------------
# Caps — lifetime_received_maximum per reserve (base units)
# ---------------------------------------------------------------------------

CAP_FORECAST=$((400000000 * MULT))    # 40%
CAP_ONBOARDING=$((200000000 * MULT))  # 20%
CAP_STREAKS=$((200000000 * MULT))     # 20%
CAP_LEADERBOARD=$((100000000 * MULT)) # 10%
CAP_CAMPAIGN=$((50000000 * MULT))     #  5%
CAP_BUFFER=$((50000000 * MULT))       #  5%

TOTAL_CAP=$((CAP_FORECAST + CAP_ONBOARDING + CAP_STREAKS + CAP_LEADERBOARD + CAP_CAMPAIGN + CAP_BUFFER))
EXPECTED_TOTAL=$((1000000000 * MULT)) # 100%
if [[ "${TOTAL_CAP}" -ne "${EXPECTED_TOTAL}" ]]; then
  echo "ERROR: total cap sum mismatch (${TOTAL_CAP} vs ${EXPECTED_TOTAL})." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Daily emission budgets (whole tokens, for auto-rebalance config)
#
# Year 1-3 target: ~200M/year = ~548k XP/day total across all buckets.
# ---------------------------------------------------------------------------

DAILY_FORECAST=220000
DAILY_ONBOARDING=130000
DAILY_STREAKS=110000
DAILY_LEADERBOARD=55000
DAILY_CAMPAIGN=33000

# ---------------------------------------------------------------------------
# Required environment variables (6 unique principals)
# ---------------------------------------------------------------------------

ALL_VARS=(
  VXP_RESERVE_PRINCIPAL_FORECAST
  VXP_RESERVE_PRINCIPAL_ONBOARDING
  VXP_RESERVE_PRINCIPAL_STREAKS
  VXP_RESERVE_PRINCIPAL_LEADERBOARD
  VXP_RESERVE_PRINCIPAL_CAMPAIGN
  VXP_RESERVE_PRINCIPAL_BUFFER
)

missing=()
for var in "${ALL_VARS[@]}"; do
  if [[ -z "${!var:-}" ]]; then
    missing+=("${var}")
  fi
done
if [[ "${#missing[@]}" -gt 0 ]]; then
  echo "ERROR: unset environment variables: ${missing[*]}" >&2
  echo "" >&2
  echo "Export all six principals, or use scripts/init.reserves.config.sh." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Duplicate principal check (belt-and-suspenders; minter also rejects dupes)
# ---------------------------------------------------------------------------

all_principals=()
for var in "${ALL_VARS[@]}"; do
  all_principals+=("${!var}")
done

for ((i = 0; i < ${#all_principals[@]}; i++)); do
  for ((j = i + 1; j < ${#all_principals[@]}; j++)); do
    if [[ "${all_principals[i]}" == "${all_principals[j]}" ]]; then
      echo "ERROR: ${ALL_VARS[i]} and ${ALL_VARS[j]} share the same principal '${all_principals[i]}'." >&2
      echo "Each reserve must have a unique principal." >&2
      exit 1
    fi
  done
done

# ---------------------------------------------------------------------------
# dfx base command
# ---------------------------------------------------------------------------

dfx_base=(dfx canister --network "${DFX_NETWORK}")
if [[ -n "${DFX_IDENTITY:-}" ]]; then
  dfx_base+=(--identity "${DFX_IDENTITY}")
fi

# ---------------------------------------------------------------------------
# Reserve registration helpers
# ---------------------------------------------------------------------------

call_add_manual_reserve() {
  local label="$1"
  local purpose="$2"
  local principal="$3"
  local cap_nat="$4"

  local candid
  candid=$(
    cat <<EOF
(record {
  account = record { owner = principal "${principal}"; subaccount = null };
  min_balance = 0 : nat;
  target_balance = 0 : nat;
  max_balance = opt (${cap_nat} : nat);
  max_topup_per_rebalance = null;
  lifetime_received_minimum = null;
  lifetime_received_maximum = opt (${cap_nat} : nat);
  rate_limits = null;
  enabled = true;
  allow_manual_topup = true;
  allow_auto_rebalance = false;
  purpose = "${purpose}";
  label = "${label}";
})
EOF
  )

  echo "--- add_reserve ${label} [manual] (cap=${cap_nat}) ---"
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "${dfx_base[*]} call ${MINTER_CANISTER} add_reserve '${candid}'"
    return 0
  fi
  "${dfx_base[@]}" call "${MINTER_CANISTER}" add_reserve "${candid}"
}

call_add_auto_reserve() {
  local label="$1"
  local purpose="$2"
  local principal="$3"
  local cap_nat="$4"
  local daily_budget="$5"

  local target=$((daily_budget * 7 * MULT))
  local min=$((daily_budget * 2 * MULT))
  local max_topup="${target}"
  local rate_day=$((daily_budget * 2 * MULT))
  local rate_year=$((daily_budget * 365 * MULT))

  local candid
  candid=$(
    cat <<EOF
(record {
  account = record { owner = principal "${principal}"; subaccount = null };
  min_balance = ${min} : nat;
  target_balance = ${target} : nat;
  max_balance = opt (${cap_nat} : nat);
  max_topup_per_rebalance = opt (${max_topup} : nat);
  lifetime_received_minimum = null;
  lifetime_received_maximum = opt (${cap_nat} : nat);
  rate_limits = opt record {
    max_amount_per_hour = null;
    max_amount_per_day = opt (${rate_day} : nat);
    max_amount_per_week = null;
    max_amount_per_month = null;
    max_amount_per_year = opt (${rate_year} : nat);
  };
  enabled = true;
  allow_manual_topup = true;
  allow_auto_rebalance = true;
  purpose = "${purpose}";
  label = "${label}";
})
EOF
  )

  echo "--- add_reserve ${label} [auto] (cap=${cap_nat} target=${target} min=${min} rate_day=${rate_day} rate_year=${rate_year}) ---"
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "${dfx_base[*]} call ${MINTER_CANISTER} add_reserve '${candid}'"
    return 0
  fi
  "${dfx_base[@]}" call "${MINTER_CANISTER}" add_reserve "${candid}"
}

# ---------------------------------------------------------------------------
# Register reserves
# ---------------------------------------------------------------------------

echo "Network: ${DFX_NETWORK}, minter: ${MINTER_CANISTER}, decimals: ${DECIMALS}"
echo ""

echo "=== Gameplay reserves (auto-rebalance) ==="
call_add_auto_reserve "forecast" \
  "Gameplay: prediction participation rewards (40%)" \
  "${VXP_RESERVE_PRINCIPAL_FORECAST}" "${CAP_FORECAST}" "${DAILY_FORECAST}"

call_add_auto_reserve "onboarding" \
  "Gameplay: signup bonuses, activation, tutorials (20%)" \
  "${VXP_RESERVE_PRINCIPAL_ONBOARDING}" "${CAP_ONBOARDING}" "${DAILY_ONBOARDING}"

call_add_auto_reserve "streaks" \
  "Gameplay: daily engagement, login streaks (20%)" \
  "${VXP_RESERVE_PRINCIPAL_STREAKS}" "${CAP_STREAKS}" "${DAILY_STREAKS}"

call_add_auto_reserve "leaderboard" \
  "Gameplay: leaderboard prizes, competitions (10%)" \
  "${VXP_RESERVE_PRINCIPAL_LEADERBOARD}" "${CAP_LEADERBOARD}" "${DAILY_LEADERBOARD}"

call_add_auto_reserve "campaign" \
  "Gameplay: promotions, referrals, events (5%)" \
  "${VXP_RESERVE_PRINCIPAL_CAMPAIGN}" "${CAP_CAMPAIGN}" "${DAILY_CAMPAIGN}"

echo ""
echo "=== Buffer (manual only) ==="
call_add_manual_reserve "buffer" \
  "Gameplay: strategic buffer for future features (5%, manual only)" \
  "${VXP_RESERVE_PRINCIPAL_BUFFER}" "${CAP_BUFFER}"

echo ""
echo "Done. Verify with: ${dfx_base[*]} call ${MINTER_CANISTER} list_reserves"
