use candid::Principal;
use ic_cdk::query;

use super::results::{GetReserveResult, ReserveInfo};
use crate::{
    guards::caller_is_not_anonymous,
    model::{GlobalPolicy, MinterError},
    state::STATE,
};

/// Returns a list of all registered reserves with their configurations and
/// lifetime minting counters.
#[query(guard = "caller_is_not_anonymous")]
fn list_reserves() -> Vec<ReserveInfo> {
    STATE.with_borrow(|s| {
        s.reserves
            .iter()
            .map(|(&id, record)| ReserveInfo {
                id,
                config: record.config.clone(),
                lifetime_minted: record.lifetime_minted.clone(),
            })
            .collect()
    })
}

fn get_reserve_impl(id: u64) -> Result<ReserveInfo, MinterError> {
    STATE.with_borrow(|s| {
        s.reserves
            .get(&id)
            .map(|record| ReserveInfo {
                id,
                config: record.config.clone(),
                lifetime_minted: record.lifetime_minted.clone(),
            })
            .ok_or(MinterError::ReserveNotFound { id })
    })
}

/// Returns the configuration and lifetime counters for a single reserve.
///
/// Returns `Err(MinterError::ReserveNotFound)` if no reserve with the
/// given id exists.
#[query(guard = "caller_is_not_anonymous")]
fn get_reserve(id: u64) -> GetReserveResult {
    get_reserve_impl(id).into()
}

/// Returns the current canister-wide global minting policy.
#[query(guard = "caller_is_not_anonymous")]
fn get_global_policy() -> GlobalPolicy {
    STATE.with_borrow(|s| s.global_policy.clone())
}

/// Returns the principal of the ICRC-1 ledger canister this minter is
/// connected to.
#[query(guard = "caller_is_not_anonymous")]
fn get_ledger_id() -> Principal {
    STATE.with_borrow(|s| s.ledger_id)
}
