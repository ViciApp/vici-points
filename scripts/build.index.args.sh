#!/usr/bin/env bash
set -euo pipefail

# This script builds the init arguments for the Index canister,
# taking into account the target DFX network (ic, staging, local).
# It considers whether the canister is already installed to determine
# whether to use the Init or Upgrade variant for the arguments.
# The interface of the Init variant can be found here:
# https://github.com/dfinity/ic/blob/d5b336cf169b3fec81385701a23e92388e8f77ae/rs/ledger_suite/icrc1/index-ng/src/lib.rs#L17

ECHO "Building Index args..."

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
ECHO "Building Index args for network: ${DFX_NETWORK}"

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
  if scripts/check.canister.installed.sh index "$DFX_NETWORK"; then
    VARIANT="Upgrade"
  else
    VARIANT="Init"
  fi
fi

ARG_FILE="$(jq -re .canisters.index.init_arg_file dfx.json)"

mkdir -p "$(dirname "$ARG_FILE")"

if [[ "$VARIANT" == "Upgrade" ]]; then

  # Use Upgrade variant: same values, but everything is opt
  cat <<-EOF >"$ARG_FILE"
  (
  	opt variant {
  		Upgrade = record {
  			ledger_id = opt principal "$CANISTER_ID_LEDGER";
  			retrieve_blocks_from_ledger_interval_seconds = opt 10
  		}
  	}
  )
EOF

else

  # Original Init variant
  cat <<-EOF >"$ARG_FILE"
  (
  	opt variant {
  		Init = record {
  			ledger_id = principal "$CANISTER_ID_LEDGER";
  			retrieve_blocks_from_ledger_interval_seconds = opt 10
  		}
  	}
  )
EOF

fi
