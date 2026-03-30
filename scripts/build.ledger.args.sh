#!/usr/bin/env bash
set -euo pipefail

# This script builds the init arguments for the Ledger canister,
# taking into account the target DFX network (ic, staging, local).
# It considers whether the canister is already installed to determine
# whether to use the Init or Upgrade variant for the arguments.
# The interface of the Init variant can be found here:
# https://github.com/dfinity/ic/blob/d5b336cf169b3fec81385701a23e92388e8f77ae/rs/ledger_suite/icrc1/ledger/src/lib.rs#L270

ECHO "Building Ledger args..."

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
ECHO "Building Ledger args for network: ${DFX_NETWORK}"

case "${DFX_NETWORK}" in
ic | staging | local) ;;
*)
  ECHO "ERROR: Unsupported DFX_NETWORK '${DFX_NETWORK:-<unset>}'"
  ECHO "       Supported values: ic, staging, local"
  exit 1
  ;;
esac

if [[ "${DFX_NETWORK}" == "ic" ]]; then
  TOKEN_SYMBOL="VXP"
  TOKEN_NAME="Vici XP"
  # TODO: rename it with the ledger canister when it is released in production
  LOGO_FILE="assets/logo/prod_logo_1024.png"
elif [[ "${DFX_NETWORK}" == "staging" ]]; then
  TOKEN_SYMBOL="testVXP"
  TOKEN_NAME="Test Vici XP"
  LOGO_FILE="assets/logo/ap6gq-taaaa-aaaae-acsaq-cai.png"
else
  # For local env we use the same as ic, since we assume it is a local deployment
  TOKEN_SYMBOL="VXP"
  TOKEN_NAME="Vici XP"
  # TODO: rename it with the ledger canister when it is released in production
  LOGO_FILE="assets/logo/prod_logo_1024.png"
fi

# 0.0001 VXP per transfer (4 decimals → fee = 1 base unit)
TRANSFER_FEE=1
DECIMALS=4

# Portable base64 (works on Linux and macOS)
encode_b64() {
  if base64 --help 2>&1 | grep -q -- '-w '; then
    base64 -w 0 "$1"
  else
    base64 <"$1" | tr -d '\n'
  fi
}

MIME_TYPE="image/png"

if [[ -f "$LOGO_FILE" ]]; then
  B64_LOGO="$(encode_b64 "$LOGO_FILE")"
  DATA_URI="data:${MIME_TYPE};base64,${B64_LOGO}"
  HAS_LOGO=true
else
  ECHO "Warning: logo file '$LOGO_FILE' not found – skipping logo metadata"
  HAS_LOGO=false
fi

PRINCIPAL="$(dfx identity get-principal)"
MINTER_PRINCIPAL="${MINTER_PRINCIPAL:-$PRINCIPAL}"
ECHO "Using minter principal: $MINTER_PRINCIPAL"

if [[ "$MODE" == "upgrade" ]]; then
  VARIANT="Upgrade"
elif [[ "$MODE" == "init" ]]; then
  VARIANT="Init"
else
  if scripts/check.canister.installed.sh ledger "$DFX_NETWORK"; then
    VARIANT="Upgrade"
  else
    VARIANT="Init"
  fi
fi

ARG_FILE="$(jq -re .canisters.ledger.init_arg_file dfx.json)"

mkdir -p "$(dirname "$ARG_FILE")"

# Init requires `metadata` (vec, not opt). With no logo, use an empty vec — do not omit the field.
LOGO_METADATA_OPT=""
LOGO_METADATA="metadata = vec {};"
if [[ "$HAS_LOGO" == true ]]; then
  LOGO_METADATA_OPT="metadata = opt vec { record { \"icrc1:logo\"; variant { Text = \"$DATA_URI\" } } };"
  LOGO_METADATA="metadata = vec { record { \"icrc1:logo\"; variant { Text = \"$DATA_URI\" } } };"
fi

if [[ "$VARIANT" == "Upgrade" ]]; then

  # Use Upgrade variant: same values, but everything is opt
  cat <<-EOF >"$ARG_FILE"
  (
    variant {
      Upgrade = opt record {
        token_symbol = opt "$TOKEN_SYMBOL";
        token_name = opt "$TOKEN_NAME";
        transfer_fee = opt $TRANSFER_FEE;
        decimals = opt $DECIMALS;
        ${LOGO_METADATA_OPT}
        feature_flags = opt record {
          icrc2 = true;
          icrc3 = true
        }
      }
    }
  )
EOF

else

  # Original Init variant
  cat <<-EOF >"$ARG_FILE"
  (
    variant {
      Init = record {
        token_symbol = "$TOKEN_SYMBOL";
        token_name = "$TOKEN_NAME";
        transfer_fee = $TRANSFER_FEE;
        decimals = opt $DECIMALS;
        ${LOGO_METADATA}
        feature_flags = opt record {
          icrc2 = true;
          icrc3 = true
        };
        minting_account = record {
          owner = principal "$MINTER_PRINCIPAL"
        };
        initial_balances = vec {};
        archive_options = record {
          num_blocks_to_archive = 1_000;
          trigger_threshold = 2_000;
          controller_id = principal "$PRINCIPAL";
          cycles_for_archive_creation = opt 10_000_000_000_000
        }
      }
    }
  )
EOF

fi
