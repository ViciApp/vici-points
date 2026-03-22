#!/usr/bin/env bash
set -euo pipefail

# Builds install/upgrade arguments for the minter canister (Candid type `Arg`).
# Init carries the ledger principal; Upgrade is used when WASM is replaced
# but stable state is preserved. See src/minter/src/model.rs.

ECHO() { echo "$@"; }

ECHO "Building Minter args..."

MODE="${1:-auto}"
case "$MODE" in
auto | init | upgrade) ;;
*)
  ECHO "Usage: $0 [auto|init|upgrade]"
  ECHO "       mode: auto (default), init, upgrade"
  exit 1
  ;;
esac

DFX_NETWORK="${DFX_NETWORK:-local}"
ECHO "Building Minter args for network: ${DFX_NETWORK}"

case "${DFX_NETWORK}" in
ic | staging | local) ;;
*)
  ECHO "ERROR: Unsupported DFX_NETWORK '${DFX_NETWORK:-<unset>}'"
  ECHO "       Supported values: ic, staging, local"
  exit 1
  ;;
esac

CANISTER_ID_LEDGER="$(jq -re ".ledger.\"$DFX_NETWORK\"" canister_ids.json)"
ECHO "Using Ledger canister ID: $CANISTER_ID_LEDGER"

if [[ "$MODE" == "upgrade" ]]; then
  VARIANT="Upgrade"
elif [[ "$MODE" == "init" ]]; then
  VARIANT="Init"
else
  if scripts/check.canister.installed.sh minter "$DFX_NETWORK"; then
    VARIANT="Upgrade"
  else
    VARIANT="Init"
  fi
fi

ARG_FILE="$(jq -re .canisters.minter.init_arg_file dfx.json)"
mkdir -p "$(dirname "$ARG_FILE")"

if [[ "$VARIANT" == "Upgrade" ]]; then
  cat <<-EOF >"$ARG_FILE"
	(
		variant {
			Upgrade
		}
	)
EOF
else
  cat <<-EOF >"$ARG_FILE"
	(
		variant {
			Init = record {
				ledger_id = principal "$CANISTER_ID_LEDGER"
			}
		}
	)
EOF
fi
