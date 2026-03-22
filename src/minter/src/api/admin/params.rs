use candid::{CandidType, Nat};
use serde::{Deserialize, Serialize};

pub use crate::model::AddReserveArg;
use crate::model::RateLimits;

/// Arguments for partially updating an existing reserve's configuration.
///
/// Every field except `id` is optional.  Omitted fields (`None`) are left
/// unchanged.  Fields wrapped in `Option<Option<T>>` support three states:
/// `None` = keep current, `Some(None)` = clear, `Some(Some(v))` = set to v.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
#[expect(clippy::option_option)] // triple-state fields per Candid / admin API contract
pub struct UpdateReserveArg {
    /// Numeric identifier of the reserve to update.
    pub id: u64,
    /// New balance threshold, if changing.
    pub min_balance: Option<Nat>,
    /// New target balance, if changing.
    pub target_balance: Option<Nat>,
    /// New max balance cap (triple-option: keep / clear / set).
    pub max_balance: Option<Option<Nat>>,
    /// New max topup per rebalance (triple-option: keep / clear / set).
    pub max_topup_per_rebalance: Option<Option<Nat>>,
    /// New lifetime minimum guarantee.  Can only increase, never decrease.
    pub lifetime_received_minimum: Option<Nat>,
    /// New lifetime maximum cap (triple-option: keep / clear / set).
    pub lifetime_received_maximum: Option<Option<Nat>>,
    /// New rate limits (triple-option: keep / clear / set).
    pub rate_limits: Option<Option<RateLimits>>,
    /// Enable or disable the reserve.
    pub enabled: Option<bool>,
    /// Allow or disallow manual top-ups.
    pub allow_manual_topup: Option<bool>,
    /// Allow or disallow auto-rebalancing.
    pub allow_auto_rebalance: Option<bool>,
    /// New purpose description.
    pub purpose: Option<String>,
    /// New label.
    pub label: Option<String>,
}

/// Clears a stuck per-reserve mint lock and optionally drops a pending
/// idempotency entry (controller-only recovery).
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ReleaseStuckMintArg {
    /// Reserve id to remove from the in-flight mint lock set.
    pub reserve_id: u64,
    /// When set, removes this key only if it is still pending for `reserve_id`.
    pub idempotency_key: Option<String>,
}

/// Arguments for issuing an ad-hoc manual top-up to a reserve.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ManualTopupArg {
    /// Which reserve to top up.
    pub reserve_id: u64,
    /// Exact amount to mint to the reserve's account.
    pub amount: Nat,
    /// Optional caller-provided key for idempotent delivery.
    ///
    /// If the key already has a **completed** mint, the original response is returned.  If a
    /// mint with this key is **in progress**, the call fails with
    /// `MinterError::IdempotencyOperationInProgress`.  Concurrent mints to the same reserve
    /// fail with `MinterError::ReserveOperationInProgress` (whether or not a key is used).
    pub idempotency_key: Option<String>,
}
