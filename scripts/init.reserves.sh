#!/usr/bin/env bash
#
# Initialize minter reserves with per-bucket community sub-reserves.
#
# Community 450M (45%) is split into six sub-reserves:
#
#   Bucket        Cap (VICI)  % of 45%  Auto-rebalance
#   ─────────────────────────────────────────────────────
#   forecast      135M        30%       yes
#   liquidity     112.5M      25%       yes
#   onboarding     67.5M      15%       yes
#   oracle         67.5M      15%       yes
#   campaign       45M        10%       yes
#   buffer         22.5M       5%       no (manual only)
#
# Plus four non-community reserves (all manual only):
#
#   treasury      200M (20%)
#   team          150M (15%)
#   investors     150M (15%)
#   advisors       50M  (5%)
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
# Supply ten unique ICRC-1 owner principals via environment variables.
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

# Community sub-reserves (must sum to 450M)
CAP_FORECAST=$((135000000 * MULT))  # 30% of community
CAP_LIQUIDITY=$((112500000 * MULT)) # 25% of community
CAP_ONBOARDING=$((67500000 * MULT)) # 15% of community
CAP_ORACLE=$((67500000 * MULT))     # 15% of community
CAP_CAMPAIGN=$((45000000 * MULT))   # 10% of community
CAP_BUFFER=$((22500000 * MULT))     #  5% of community

COMMUNITY_SUM=$((CAP_FORECAST + CAP_LIQUIDITY + CAP_ONBOARDING + CAP_ORACLE + CAP_CAMPAIGN + CAP_BUFFER))
EXPECTED_COMMUNITY=$((450000000 * MULT)) # 45% of total
if [[ "${COMMUNITY_SUM}" -ne "${EXPECTED_COMMUNITY}" ]]; then
  echo "ERROR: community sub-cap sum mismatch (${COMMUNITY_SUM} vs ${EXPECTED_COMMUNITY})." >&2
  exit 1
fi

# Non-community reserves
CAP_TREASURY=$((200000000 * MULT))  # 20% of total
CAP_TEAM=$((150000000 * MULT))      # 15% of total
CAP_INVESTORS=$((150000000 * MULT)) # 15% of total
CAP_ADVISORS=$((50000000 * MULT))   #  5% of total

TOTAL_CAP=$((COMMUNITY_SUM + CAP_TREASURY + CAP_TEAM + CAP_INVESTORS + CAP_ADVISORS))
EXPECTED_TOTAL=$((1000000000 * MULT)) # 100%
if [[ "${TOTAL_CAP}" -ne "${EXPECTED_TOTAL}" ]]; then
  echo "ERROR: total cap sum mismatch (${TOTAL_CAP} vs ${EXPECTED_TOTAL})." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Daily emission budgets (whole tokens, for auto-rebalance config)
#
# Year 1-3 target: ~75M/year = ~205k/day total across all buckets.
# ---------------------------------------------------------------------------

DAILY_FORECAST=80000
DAILY_LIQUIDITY=60000
DAILY_ONBOARDING=30000
DAILY_ORACLE=20000
DAILY_CAMPAIGN=10000

# ---------------------------------------------------------------------------
# Required environment variables (10 unique principals)
# ---------------------------------------------------------------------------

ALL_VARS=(
  VICI_RESERVE_PRINCIPAL_FORECAST
  VICI_RESERVE_PRINCIPAL_LIQUIDITY
  VICI_RESERVE_PRINCIPAL_ONBOARDING
  VICI_RESERVE_PRINCIPAL_ORACLE
  VICI_RESERVE_PRINCIPAL_CAMPAIGN
  VICI_RESERVE_PRINCIPAL_BUFFER
  VICI_RESERVE_PRINCIPAL_TREASURY
  VICI_RESERVE_PRINCIPAL_TEAM
  VICI_RESERVE_PRINCIPAL_INVESTORS
  VICI_RESERVE_PRINCIPAL_ADVISORS
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
  echo "Export all ten principals, or use scripts/init.reserves.config.sh." >&2
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

# Manual-only reserve (no auto-rebalance, no rate limits).
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

# Auto-rebalancing community sub-reserve.
#
# Derived parameters from daily_budget:
#   target_balance          = daily_budget * 7 days  (1-week runway)
#   min_balance             = daily_budget * 2 days  (refill trigger)
#   max_topup_per_rebalance = target_balance         (full refill in one go)
#   max_amount_per_day      = daily_budget * 2       (catch-up headroom)
#   max_amount_per_year     = daily_budget * 365     (annual emission cap)
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

echo "=== Community sub-reserves (auto-rebalance) ==="
call_add_auto_reserve "forecast" \
  "Community: forecast rewards (30% of 45%)" \
  "${VICI_RESERVE_PRINCIPAL_FORECAST}" "${CAP_FORECAST}" "${DAILY_FORECAST}"

call_add_auto_reserve "liquidity" \
  "Community: liquidity incentives (25% of 45%)" \
  "${VICI_RESERVE_PRINCIPAL_LIQUIDITY}" "${CAP_LIQUIDITY}" "${DAILY_LIQUIDITY}"

call_add_auto_reserve "onboarding" \
  "Community: new user onboarding (15% of 45%)" \
  "${VICI_RESERVE_PRINCIPAL_ONBOARDING}" "${CAP_ONBOARDING}" "${DAILY_ONBOARDING}"

call_add_auto_reserve "oracle" \
  "Community: market/oracle rewards (15% of 45%)" \
  "${VICI_RESERVE_PRINCIPAL_ORACLE}" "${CAP_ORACLE}" "${DAILY_ORACLE}"

call_add_auto_reserve "campaign" \
  "Community: ecosystem campaigns (10% of 45%)" \
  "${VICI_RESERVE_PRINCIPAL_CAMPAIGN}" "${CAP_CAMPAIGN}" "${DAILY_CAMPAIGN}"

echo ""
echo "=== Community buffer (manual only) ==="
call_add_manual_reserve "buffer" \
  "Community: strategic buffer (5% of 45%, manual only)" \
  "${VICI_RESERVE_PRINCIPAL_BUFFER}" "${CAP_BUFFER}"

echo ""
echo "=== Non-community reserves (manual only) ==="
call_add_manual_reserve "treasury" \
  "Tokenomics: treasury (20% lifetime cap)" \
  "${VICI_RESERVE_PRINCIPAL_TREASURY}" "${CAP_TREASURY}"

call_add_manual_reserve "team" \
  "Tokenomics: team allocation (15% lifetime cap)" \
  "${VICI_RESERVE_PRINCIPAL_TEAM}" "${CAP_TEAM}"

call_add_manual_reserve "investors" \
  "Tokenomics: investors (15% lifetime cap)" \
  "${VICI_RESERVE_PRINCIPAL_INVESTORS}" "${CAP_INVESTORS}"

call_add_manual_reserve "advisors" \
  "Tokenomics: advisors (5% lifetime cap)" \
  "${VICI_RESERVE_PRINCIPAL_ADVISORS}" "${CAP_ADVISORS}"

echo ""
echo "Done. Verify with: ${dfx_base[*]} call ${MINTER_CANISTER} list_reserves"
