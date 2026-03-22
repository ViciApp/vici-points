#!/usr/bin/env bash
#
# Local reserve principals — copy to init.reserves.config.sh and edit.
#
#   cp scripts/init.reserves.config.example.sh scripts/init.reserves.config.sh
#
# scripts/init.reserves.config.sh is gitignored so real principals stay off git.
# Fill all six principals before running — each must be unique.
#
# Why a wrapper instead of editing init.reserves.sh?
#   - init.reserves.sh stays generic (reviewable, safe to commit).
#   - Your principals live in one obvious place (this file).
#   - Same pattern as .env + tooling: separate config from logic.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Gameplay reserves (each needs a unique principal)
export VXP_RESERVE_PRINCIPAL_FORECAST=""
export VXP_RESERVE_PRINCIPAL_ONBOARDING=""
export VXP_RESERVE_PRINCIPAL_STREAKS=""
export VXP_RESERVE_PRINCIPAL_LEADERBOARD=""
export VXP_RESERVE_PRINCIPAL_CAMPAIGN=""
export VXP_RESERVE_PRINCIPAL_BUFFER=""

exec "${SCRIPT_DIR}/init.reserves.sh" "$@"
