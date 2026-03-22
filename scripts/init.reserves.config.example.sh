#!/usr/bin/env bash
#
# Local reserve principals — copy to init.reserves.config.sh and edit.
#
#   cp scripts/init.reserves.config.example.sh scripts/init.reserves.config.sh
#
# scripts/init.reserves.config.sh is gitignored so real principals stay off git.
# Fill all ten principals before running — each must be unique.
#
# Why a wrapper instead of editing init.reserves.sh?
#   - init.reserves.sh stays generic (reviewable, safe to commit).
#   - Your principals live in one obvious place (this file).
#   - Same pattern as .env + tooling: separate config from logic.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Community sub-reserves (each needs a unique principal)
export VICI_RESERVE_PRINCIPAL_FORECAST=""
export VICI_RESERVE_PRINCIPAL_LIQUIDITY=""
export VICI_RESERVE_PRINCIPAL_ONBOARDING=""
export VICI_RESERVE_PRINCIPAL_ORACLE=""
export VICI_RESERVE_PRINCIPAL_CAMPAIGN=""
export VICI_RESERVE_PRINCIPAL_BUFFER=""

# Non-community reserves
export VICI_RESERVE_PRINCIPAL_TREASURY=""
export VICI_RESERVE_PRINCIPAL_TEAM=""
export VICI_RESERVE_PRINCIPAL_INVESTORS=""
export VICI_RESERVE_PRINCIPAL_ADVISORS=""

exec "${SCRIPT_DIR}/init.reserves.sh" "$@"
