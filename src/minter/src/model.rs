use candid::{CandidType, Nat, Principal};
use icrc_ledger_types::icrc1::account::Account;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Init / upgrade arg
// ---------------------------------------------------------------------------

/// Arguments supplied when the minter canister is first installed.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct InitArg {
    /// The principal of the ICRC-1 ledger canister this minter operates on.
    pub ledger_id: Principal,
}

/// Discriminated union passed to both `init` and `post_upgrade` hooks so the
/// canister knows whether it is being freshly installed or upgraded.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum Arg {
    /// First-time installation; carries the initial configuration.
    Init(InitArg),
    /// Code upgrade; the canister restores state from stable memory.
    Upgrade,
}

// ---------------------------------------------------------------------------
// Rate limits
// ---------------------------------------------------------------------------

/// Per-reserve rate-limit caps that restrict how many tokens can be minted
/// within sliding time windows.
///
/// When multiple windows are configured, larger windows must be **strictly
/// more restrictive** (lower effective rate) than smaller ones.  This is
/// validated at creation and update time.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
#[expect(clippy::struct_field_names)] // field names mirror policy / Candid interface
pub struct RateLimits {
    /// Maximum total amount mintable in any rolling 1-hour window.
    pub max_amount_per_hour: Option<Nat>,
    /// Maximum total amount mintable in any rolling 24-hour window.
    pub max_amount_per_day: Option<Nat>,
    /// Maximum total amount mintable in any rolling 7-day window.
    pub max_amount_per_week: Option<Nat>,
    /// Maximum total amount mintable in any rolling 30-day window.
    pub max_amount_per_month: Option<Nat>,
    /// Maximum total amount mintable in any rolling 365-day window.
    pub max_amount_per_year: Option<Nat>,
}

/// A single recorded mint event used for rate-limit accounting.
///
/// Stored in [`ReserveRecord::mint_events`] and automatically pruned
/// once older than the largest window (1 year).
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct MintEvent {
    /// IC timestamp (in nanoseconds) when the mint occurred.
    pub timestamp_ns: u64,
    /// Amount that was minted in this event.
    pub amount: Nat,
}

// ---------------------------------------------------------------------------
// Reserve configuration
// ---------------------------------------------------------------------------

/// Full configuration of a single reserve account.
///
/// A reserve represents a trusted system account whose token balance the
/// minter keeps topped up via periodic rebalancing or manual top-ups.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ReserveConfig {
    /// The ICRC-1 account that holds the reserve's tokens.
    pub account: Account,
    /// Balance threshold below which a rebalance is triggered.
    pub min_balance: Nat,
    /// Desired balance the minter aims to restore during rebalance.
    pub target_balance: Nat,
    /// Hard upper bound on the account's balance; the minter will never
    /// mint tokens that would push the balance above this value.
    pub max_balance: Option<Nat>,
    /// Maximum amount the minter may mint in a single rebalance operation.
    pub max_topup_per_rebalance: Option<Nat>,
    /// Guaranteed minimum total amount this reserve must have received from
    /// the minter over its entire lifetime.  Can only be increased, never
    /// decreased.  If the guarantee is not yet met, additional tokens are
    /// minted to cover the shortfall.
    pub lifetime_received_minimum: Option<Nat>,
    /// Hard cap on the total amount this reserve may ever receive from the
    /// minter.  Once reached, no further minting is allowed for this
    /// reserve.  Must be >= `lifetime_received_minimum` when both are set.
    pub lifetime_received_maximum: Option<Nat>,
    /// Optional per-reserve rate limits constraining how quickly tokens can
    /// be minted within sliding time windows.
    pub rate_limits: Option<RateLimits>,
    /// Whether this reserve is active.  Disabled reserves are skipped during
    /// rebalancing and reject manual top-ups.
    pub enabled: bool,
    /// Whether administrators may issue ad-hoc manual top-ups to this
    /// reserve.
    pub allow_manual_topup: bool,
    /// Whether the automatic rebalance logic is permitted to mint tokens
    /// for this reserve.
    pub allow_auto_rebalance: bool,
    /// Free-form description of what this reserve is used for.
    pub purpose: String,
    /// Short human-readable name for this reserve.
    pub label: String,
}

/// Persistent per-reserve record stored in canister state.
///
/// Wraps the admin-defined [`ReserveConfig`] together with runtime counters
/// that track cumulative minting activity and recent mint events for
/// rate-limit enforcement.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ReserveRecord {
    /// The admin-defined configuration for this reserve.
    pub config: ReserveConfig,
    /// Cumulative total of tokens minted to this reserve since it was
    /// created.  Used to enforce lifetime minimum / maximum guarantees.
    pub lifetime_minted: Nat,
    /// Recent mint events kept for rate-limit enforcement.  Events older
    /// than 1 year are pruned after every mint.
    pub mint_events: Vec<MintEvent>,
}

/// Arguments for registering a new reserve account.
///
/// All fields mirror [`ReserveConfig`]; the minter assigns the numeric id.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct AddReserveArg {
    /// The ICRC-1 account to register as a new reserve.
    pub account: Account,
    /// Balance threshold below which a rebalance is triggered.
    pub min_balance: Nat,
    /// Desired balance the minter aims to restore during rebalance.
    pub target_balance: Nat,
    /// Hard upper bound on the account's balance.
    pub max_balance: Option<Nat>,
    /// Maximum amount the minter may mint in a single rebalance.
    pub max_topup_per_rebalance: Option<Nat>,
    /// Guaranteed minimum total amount this reserve must receive from the
    /// minter over its lifetime.
    pub lifetime_received_minimum: Option<Nat>,
    /// Hard cap on the total amount this reserve may ever receive.
    pub lifetime_received_maximum: Option<Nat>,
    /// Optional rate limits for this reserve.
    pub rate_limits: Option<RateLimits>,
    /// Whether the reserve starts active.
    pub enabled: bool,
    /// Whether manual top-ups are permitted.
    pub allow_manual_topup: bool,
    /// Whether auto-rebalancing is permitted.
    pub allow_auto_rebalance: bool,
    /// Free-form description of the reserve's purpose.
    pub purpose: String,
    /// Short human-readable label.
    pub label: String,
}

// ---------------------------------------------------------------------------
// Global policy
// ---------------------------------------------------------------------------

/// Canister-wide policy flags that govern all minting operations.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct GlobalPolicy {
    /// Master switch: when `false`, all minting (rebalance and manual) is
    /// rejected.
    pub minting_enabled: bool,
    /// Optional cap on the amount that can be minted in any single
    /// operation (rebalance or manual top-up).
    pub max_mint_per_operation: Option<Nat>,
}

impl Default for GlobalPolicy {
    fn default() -> Self {
        Self {
            minting_enabled: true,
            max_mint_per_operation: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Idempotency tracking
// ---------------------------------------------------------------------------

/// Record of a previously executed manual top-up, keyed by the caller's
/// idempotency key.  Replaying a request with the same key returns this
/// entry without re-minting.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct IdempotencyEntry {
    /// Block index on the ledger where the original mint was recorded.
    pub ledger_block_index: Nat,
    /// Amount that was minted.
    pub minted_amount: Nat,
    /// Reserve that received the tokens.
    pub reserve_id: u64,
    /// IC timestamp (nanoseconds) when the mint was executed.
    pub executed_at_ns: u64,
}

/// Stored value for manual top-up idempotency: in-flight vs completed.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum IdempotencyRecord {
    /// A mint using this key is in progress (reserved before the ledger call).
    Pending {
        /// Reserve the pending mint targets (for safety checks on admin paths).
        reserve_id: u64,
    },
    /// The original mint finished; replays must return this payload.
    Completed(IdempotencyEntry),
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors surfaced as `Err` on methods whose return type includes `MinterError`.
///
/// Controller and anonymity checks are enforced by `ic_cdk` update/query guards and reject the
/// call with a textual message before the handler runs; they do **not** map to these variants.
/// [`MinterError::NotAuthorized`] is kept for the Candid interface.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum MinterError {
    /// Reserved for the Candid interface; authorization failures use guard rejections instead.
    NotAuthorized,
    /// No reserve exists with the given id.
    ReserveNotFound { id: u64 },
    /// A reserve with this ICRC-1 account is already registered.
    ReserveAccountAlreadyExists { account: Account },
    /// The global policy has minting disabled.
    MintingDisabled,
    /// The targeted reserve is disabled.
    ReserveDisabled { id: u64 },
    /// Manual top-ups are not allowed for this reserve.
    ManualTopupNotAllowed { id: u64 },
    /// Auto-rebalancing is not allowed for this reserve.
    AutoRebalanceNotAllowed { id: u64 },
    /// The requested mint amount exceeds a configured limit (per-operation
    /// policy cap or lifetime maximum).
    AmountExceedsLimit { requested: Nat, limit: Nat },
    /// The mint would exceed a sliding-window rate limit.
    RateLimitExceeded {
        /// Which time window was breached (e.g. "hour", "day").
        window: String,
        /// The configured cap for that window.
        limit: Nat,
        /// How much has already been minted within the window.
        current_usage: Nat,
        /// The amount that was requested.
        requested: Nat,
    },
    /// The reserve or rate-limit configuration is invalid.
    InvalidConfig { reason: String },
    /// An error occurred while communicating with the ICRC-1 ledger.
    LedgerError { message: String },
    /// Reserved for the Candid interface.  Manual top-ups replay completed keys via
    /// [`IdempotencyRecord::Completed`] without returning this error.
    IdempotencyKeyAlreadyUsed {
        /// The key that collided.
        key: String,
        /// Block index of the original mint.
        existing_block_index: Nat,
    },
    /// Another update is already minting to this reserve; wait and retry.
    ReserveOperationInProgress { id: u64 },
    /// This idempotency key is tied to a mint that has not finished yet.
    IdempotencyOperationInProgress { key: String },
    /// An internal consistency check failed (e.g. after a ledger mint).
    InternalInvariantViolated { reason: String },
}

// ---------------------------------------------------------------------------
// Rebalance
// ---------------------------------------------------------------------------

/// Outcome of a rebalance computation or execution.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RebalanceAction {
    /// Tokens were (or would be) minted to the reserve.
    Minted,
    /// The reserve already meets its target; no minting needed.
    AlreadyFunded,
    /// The rebalance was skipped for the stated reason.
    Skipped { reason: String },
}

// ---------------------------------------------------------------------------
// Rebalance response (shared between API and service layers)
// ---------------------------------------------------------------------------

/// Response from a rebalance operation on a single reserve.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct RebalanceResponse {
    /// The reserve that was rebalanced.
    pub reserve_id: u64,
    /// What happened during the rebalance.
    pub action: RebalanceAction,
    /// Balance of the reserve's account before the rebalance.
    pub balance_before: Nat,
    /// Amount of tokens that were actually minted (0 if nothing was minted).
    pub minted_amount: Nat,
    /// Ledger block index of the mint transaction, if any.
    pub ledger_block_index: Option<Nat>,
}

// ---------------------------------------------------------------------------
// Internal (not exposed over Candid)
// ---------------------------------------------------------------------------

/// Internal result of [`crate::services::reserve::compute_rebalance`].
///
/// Carries the computed mint amount and deficit breakdowns so callers can
/// decide how to proceed (mint, skip, or preview).
pub struct ComputedRebalance {
    /// The determined action (`Minted` / `AlreadyFunded` / `Skipped`).
    pub action: RebalanceAction,
    /// Token amount to mint (0 when no minting is needed).
    pub mint_amount: Nat,
    /// The cyclical (balance-target) deficit component.
    pub cyclical_deficit: Nat,
    /// The lifetime-guarantee deficit component.
    pub lifetime_deficit: Nat,
}
