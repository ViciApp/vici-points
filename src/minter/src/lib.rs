mod api;
mod guards;
mod model;
mod services;
mod state;

use candid::Principal;
use ic_cdk::{export_candid, init, post_upgrade, pre_upgrade, trap};

use crate::{
    api::{
        admin::{
            params::{ManualTopupArg, ReleaseStuckMintArg, UpdateReserveArg},
            results::{
                AddReserveResult, ManualTopupReserveResult, PreviewRebalanceReserveResult,
                RebalanceReserveResult, ReleaseStuckMintResult, RemoveReserveResult,
                SetGlobalPolicyResult, UpdateReserveResult,
            },
        },
        query::results::{GetReserveResult, ReserveInfo},
    },
    model::{AddReserveArg, Arg, GlobalPolicy},
    services::timer::start_rebalance_timer,
    state::{save_state, try_restore_state, STATE},
};

/// Called when the canister is first installed.
///
/// Expects `Arg::Init` carrying the ledger principal.  Traps if
/// the `Upgrade` variant is provided during initial installation.
#[init]
fn init(arg: Arg) {
    match arg {
        Arg::Init(init_arg) => {
            STATE.with_borrow_mut(|s| s.ledger_id = init_arg.ledger_id);
        }
        Arg::Upgrade => trap("expected Init variant for canister init"),
    }

    start_rebalance_timer();
}

/// Serialises the entire canister state to stable memory before a code
/// upgrade.
#[pre_upgrade]
fn pre_upgrade() {
    save_state();
}

/// Restores canister state from stable memory after a code upgrade.
///
/// Accepts `Arg::Upgrade` (normal path) or `Arg::Init` (first
/// upgrade from a placeholder canister that had no prior state).
#[post_upgrade]
fn post_upgrade(arg: Arg) {
    match arg {
        Arg::Upgrade => {
            if !try_restore_state() {
                trap("failed to restore state from stable memory");
            }
        }
        Arg::Init(init_arg) => {
            STATE.with_borrow_mut(|s| s.ledger_id = init_arg.ledger_id);
        }
    }

    start_rebalance_timer();
}

export_candid!();
