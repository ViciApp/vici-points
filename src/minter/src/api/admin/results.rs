use candid::{CandidType, Nat};
use serde::{Deserialize, Serialize};

use crate::model::{MinterError, RebalanceAction, RebalanceResponse, ReserveConfig};

// ---------------------------------------------------------------------------
// Response structs (success payloads)
// ---------------------------------------------------------------------------

/// Successful response from a manual top-up operation.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ManualTopupResponse {
    /// The reserve that was topped up.
    pub reserve_id: u64,
    /// Amount of tokens actually minted.
    pub minted_amount: Nat,
    /// Block index on the ledger where the mint was recorded.
    pub ledger_block_index: Nat,
}

/// Preview of what a rebalance *would* do without actually minting.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct PreviewRebalanceResponse {
    /// The reserve being previewed.
    pub reserve_id: u64,
    /// The action that would be taken.
    pub action: RebalanceAction,
    /// Current balance of the reserve's account on the ledger.
    pub current_balance: Nat,
    /// The configured target balance.
    pub target_balance: Nat,
    /// Shortfall relative to `target_balance` (cyclical rebalancing need).
    pub cyclical_deficit: Nat,
    /// Shortfall relative to `lifetime_received_minimum`.
    pub lifetime_deficit: Nat,
    /// Amount that would actually be minted (after all caps and rate limits).
    pub would_mint: Nat,
    /// Remaining rate-limit budget; `None` if no rate limits are configured.
    pub rate_limit_budget: Option<Nat>,
}

// ---------------------------------------------------------------------------
// Result enums
// ---------------------------------------------------------------------------

/// Outcome of registering a new reserve account.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum AddReserveResult {
    /// Successfully registered; contains the newly assigned reserve id.
    Ok(u64),
    /// Failed to register the reserve.
    Err(MinterError),
}

impl From<Result<u64, MinterError>> for AddReserveResult {
    fn from(value: Result<u64, MinterError>) -> Self {
        match value {
            Ok(v) => AddReserveResult::Ok(v),
            Err(e) => AddReserveResult::Err(e),
        }
    }
}

/// Outcome of updating an existing reserve's configuration.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum UpdateReserveResult {
    /// Successfully updated the reserve.
    Ok,
    /// Failed to update the reserve.
    Err(MinterError),
}

impl From<Result<(), MinterError>> for UpdateReserveResult {
    fn from(value: Result<(), MinterError>) -> Self {
        match value {
            Ok(()) => UpdateReserveResult::Ok,
            Err(e) => UpdateReserveResult::Err(e),
        }
    }
}

/// Outcome of removing a reserve.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum RemoveReserveResult {
    /// Successfully removed; contains the removed reserve's configuration.
    Ok(Box<ReserveConfig>),
    /// Failed to remove the reserve.
    Err(MinterError),
}

impl From<Result<ReserveConfig, MinterError>> for RemoveReserveResult {
    fn from(value: Result<ReserveConfig, MinterError>) -> Self {
        match value {
            Ok(v) => RemoveReserveResult::Ok(Box::new(v)),
            Err(e) => RemoveReserveResult::Err(e),
        }
    }
}

/// Outcome of setting the global minting policy.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum SetGlobalPolicyResult {
    /// Successfully updated the global policy.
    Ok,
    /// Failed to update the global policy.
    Err(MinterError),
}

impl From<Result<(), MinterError>> for SetGlobalPolicyResult {
    fn from(value: Result<(), MinterError>) -> Self {
        match value {
            Ok(()) => SetGlobalPolicyResult::Ok,
            Err(e) => SetGlobalPolicyResult::Err(e),
        }
    }
}

/// Outcome of a rebalance operation on a single reserve.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum RebalanceReserveResult {
    /// Successfully executed the rebalance.
    Ok(RebalanceResponse),
    /// Failed to execute the rebalance.
    Err(MinterError),
}

impl From<Result<RebalanceResponse, MinterError>> for RebalanceReserveResult {
    fn from(value: Result<RebalanceResponse, MinterError>) -> Self {
        match value {
            Ok(v) => RebalanceReserveResult::Ok(v),
            Err(e) => RebalanceReserveResult::Err(e),
        }
    }
}

/// Outcome of a manual top-up operation.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum ManualTopupReserveResult {
    /// Successfully minted the top-up.
    Ok(ManualTopupResponse),
    /// Failed to mint the top-up.
    Err(MinterError),
}

impl From<Result<ManualTopupResponse, MinterError>> for ManualTopupReserveResult {
    fn from(value: Result<ManualTopupResponse, MinterError>) -> Self {
        match value {
            Ok(v) => ManualTopupReserveResult::Ok(v),
            Err(e) => ManualTopupReserveResult::Err(e),
        }
    }
}

/// Outcome of previewing a rebalance for a single reserve.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum PreviewRebalanceReserveResult {
    /// Successfully computed the preview.
    Ok(PreviewRebalanceResponse),
    /// Failed to compute the preview.
    Err(MinterError),
}

impl From<Result<PreviewRebalanceResponse, MinterError>> for PreviewRebalanceReserveResult {
    fn from(value: Result<PreviewRebalanceResponse, MinterError>) -> Self {
        match value {
            Ok(v) => PreviewRebalanceReserveResult::Ok(v),
            Err(e) => PreviewRebalanceReserveResult::Err(e),
        }
    }
}

/// Outcome of clearing stuck mint / idempotency state.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum ReleaseStuckMintResult {
    /// Locks and/or pending idempotency entries were cleared.
    Ok,
    /// Failed (e.g. idempotency key pending for another reserve).
    Err(MinterError),
}

impl From<Result<(), MinterError>> for ReleaseStuckMintResult {
    fn from(value: Result<(), MinterError>) -> Self {
        match value {
            Ok(()) => ReleaseStuckMintResult::Ok,
            Err(e) => ReleaseStuckMintResult::Err(e),
        }
    }
}
