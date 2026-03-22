use candid::Nat;

use crate::model::{
    AddReserveArg, ComputedRebalance, GlobalPolicy, MintEvent, MinterError, RateLimits,
    RebalanceAction, ReserveConfig,
};

// ---------------------------------------------------------------------------
// Time constants (nanoseconds) – used at runtime for window checks
// ---------------------------------------------------------------------------

/// Nanoseconds in one hour (3 600 s).
const NANOS_PER_HOUR: u64 = 3_600_000_000_000;
/// Nanoseconds in one day (86 400 s).
const NANOS_PER_DAY: u64 = 86_400_000_000_000;
/// Nanoseconds in one week (604 800 s).
const NANOS_PER_WEEK: u64 = 604_800_000_000_000;
/// Nanoseconds in 30 days (≈ one month).
const NANOS_PER_MONTH: u64 = 2_592_000_000_000_000;
/// Nanoseconds in 365 days (≈ one year).
const NANOS_PER_YEAR: u64 = 31_536_000_000_000_000;

// ---------------------------------------------------------------------------
// Time constants (hours) – used for rate-limit validation math
// ---------------------------------------------------------------------------

/// Hours in one hour (identity, used for uniform iteration).
const HOURS_PER_HOUR: u64 = 1;
/// Hours in one day.
const HOURS_PER_DAY: u64 = 24;
/// Hours in one week.
const HOURS_PER_WEEK: u64 = 168;
/// Hours in 30 days (≈ one month).
const HOURS_PER_MONTH: u64 = 720;
/// Hours in 365 days (≈ one year).
const HOURS_PER_YEAR: u64 = 8760;

// ---------------------------------------------------------------------------
// Rate-limit validation
// ---------------------------------------------------------------------------

/// Verifies that larger time windows are strictly more restrictive (lower
/// effective rate) than smaller ones.
///
/// For any two adjacent configured windows A (smaller) and B (larger):
///
/// ```text
/// limit_B / duration_B  <  limit_A / duration_A
/// ```
///
/// Rearranged to avoid `Nat` division:
///
/// ```text
/// limit_B * duration_A  <  limit_A * duration_B
/// ```
///
/// Returns `Err(MinterError::InvalidConfig)` if the constraint is violated.
pub(crate) fn validate_rate_limits(limits: &RateLimits) -> Result<(), MinterError> {
    let windows: Vec<(u64, Nat)> = [
        (HOURS_PER_HOUR, limits.max_amount_per_hour.clone()),
        (HOURS_PER_DAY, limits.max_amount_per_day.clone()),
        (HOURS_PER_WEEK, limits.max_amount_per_week.clone()),
        (HOURS_PER_MONTH, limits.max_amount_per_month.clone()),
        (HOURS_PER_YEAR, limits.max_amount_per_year.clone()),
    ]
    .into_iter()
    .filter_map(|(d, l)| l.map(|l| (d, l)))
    .collect();

    for pair in windows.windows(2) {
        let (d_small, l_small) = &pair[0];
        let (d_large, l_large) = &pair[1];

        let lhs = l_large.clone() * Nat::from(*d_small);
        let rhs = l_small.clone() * Nat::from(*d_large);

        if lhs >= rhs {
            return Err(MinterError::InvalidConfig {
                reason: String::from(
                    "Rate limits must be progressively more restrictive \
                     for larger time windows",
                ),
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Balance / config validation
// ---------------------------------------------------------------------------

/// Checks the fundamental ordering invariants for a reserve's balance
/// thresholds and lifetime bounds.
///
/// * `min_balance` must be ≤ `target_balance`.
/// * `target_balance` must be ≤ `max_balance` (when set).
/// * `lifetime_received_minimum` must be ≤ `lifetime_received_maximum` (when both are set).
/// * `label` must not be empty.
fn validate_balance_invariants(
    min_balance: &Nat,
    target_balance: &Nat,
    max_balance: Option<&Nat>,
    lifetime_min: Option<&Nat>,
    lifetime_max: Option<&Nat>,
    label: &str,
) -> Result<(), MinterError> {
    if label.is_empty() {
        return Err(MinterError::InvalidConfig {
            reason: String::from("Label cannot be empty"),
        });
    }
    if min_balance > target_balance {
        return Err(MinterError::InvalidConfig {
            reason: String::from("min_balance must be <= target_balance"),
        });
    }
    if let Some(max) = max_balance {
        if target_balance > max {
            return Err(MinterError::InvalidConfig {
                reason: String::from("target_balance must be <= max_balance"),
            });
        }
    }
    if let (Some(lo), Some(hi)) = (lifetime_min, lifetime_max) {
        if lo > hi {
            return Err(MinterError::InvalidConfig {
                reason: String::from(
                    "lifetime_received_maximum must be >= lifetime_received_minimum",
                ),
            });
        }
    }
    Ok(())
}

/// Validates the fields of an [`AddReserveArg`] before inserting a new
/// reserve.  Checks balance invariants *and* rate-limit progressivity.
pub(crate) fn validate_add_reserve_arg(arg: &AddReserveArg) -> Result<(), MinterError> {
    validate_balance_invariants(
        &arg.min_balance,
        &arg.target_balance,
        arg.max_balance.as_ref(),
        arg.lifetime_received_minimum.as_ref(),
        arg.lifetime_received_maximum.as_ref(),
        &arg.label,
    )?;
    if let Some(limits) = &arg.rate_limits {
        validate_rate_limits(limits)?;
    }
    Ok(())
}

/// Validates a complete [`ReserveConfig`] (used after applying partial
/// updates).  Checks balance invariants *and* rate-limit progressivity.
pub(crate) fn validate_reserve_config(config: &ReserveConfig) -> Result<(), MinterError> {
    validate_balance_invariants(
        &config.min_balance,
        &config.target_balance,
        config.max_balance.as_ref(),
        config.lifetime_received_minimum.as_ref(),
        config.lifetime_received_maximum.as_ref(),
        &config.label,
    )?;
    if let Some(limits) = &config.rate_limits {
        validate_rate_limits(limits)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rate-limit runtime helpers
// ---------------------------------------------------------------------------

/// Sums the token amounts minted within the last `window_ns` nanoseconds.
fn usage_in_window(events: &[MintEvent], now_ns: u64, window_ns: u64) -> Nat {
    let cutoff = now_ns.saturating_sub(window_ns);
    events
        .iter()
        .filter(|e| e.timestamp_ns >= cutoff)
        .fold(Nat::from(0_u64), |acc, e| acc + e.amount.clone())
}

/// Returns the maximum amount mintable right now without exceeding any
/// configured rate-limit window.
///
/// Returns `None` when no rate limits are configured on the reserve.
/// Returns `Some(0)` when the budget is fully exhausted.
///
/// The result is the *minimum remaining budget* across all active windows.
pub(crate) fn available_mint_budget(
    rate_limits: Option<&RateLimits>,
    events: &[MintEvent],
    now_ns: u64,
) -> Option<Nat> {
    let limits = rate_limits?;
    let zero = Nat::from(0_u64);

    let windows: [(u64, &Option<Nat>); 5] = [
        (NANOS_PER_HOUR, &limits.max_amount_per_hour),
        (NANOS_PER_DAY, &limits.max_amount_per_day),
        (NANOS_PER_WEEK, &limits.max_amount_per_week),
        (NANOS_PER_MONTH, &limits.max_amount_per_month),
        (NANOS_PER_YEAR, &limits.max_amount_per_year),
    ];

    let mut budget: Option<Nat> = None;

    for (window_ns, limit_opt) in &windows {
        if let Some(limit) = limit_opt {
            let used = usage_in_window(events, now_ns, *window_ns);
            let remaining = if limit > &used {
                limit.clone() - used
            } else {
                zero.clone()
            };
            budget = Some(match budget {
                Some(b) if remaining < b => remaining,
                Some(b) => b,
                None => remaining,
            });
        }
    }

    budget
}

/// Hard check used by manual top-ups: returns an error if minting `amount`
/// would exceed any configured rate-limit window.
///
/// Unlike [`available_mint_budget`] (which returns a budget for soft
/// capping), this function fails fast with a descriptive error.
pub(crate) fn check_rate_limits(
    limits: &RateLimits,
    events: &[MintEvent],
    amount: &Nat,
    now_ns: u64,
) -> Result<(), MinterError> {
    let window_checks: [(&Option<Nat>, u64, &str); 5] = [
        (&limits.max_amount_per_hour, NANOS_PER_HOUR, "hour"),
        (&limits.max_amount_per_day, NANOS_PER_DAY, "day"),
        (&limits.max_amount_per_week, NANOS_PER_WEEK, "week"),
        (&limits.max_amount_per_month, NANOS_PER_MONTH, "month"),
        (&limits.max_amount_per_year, NANOS_PER_YEAR, "year"),
    ];

    for (limit_opt, window_ns, label) in &window_checks {
        if let Some(limit) = limit_opt {
            let used = usage_in_window(events, now_ns, *window_ns);
            let projected = used.clone() + amount.clone();
            if &projected > limit {
                return Err(MinterError::RateLimitExceeded {
                    window: String::from(*label),
                    limit: limit.clone(),
                    current_usage: used,
                    requested: amount.clone(),
                });
            }
        }
    }

    Ok(())
}

/// Removes mint events older than the largest rate-limit window (1 year).
///
/// Should be called after every successful mint to keep the event list
/// bounded.
pub(crate) fn prune_old_events(events: &mut Vec<MintEvent>, now_ns: u64) {
    let cutoff = now_ns.saturating_sub(NANOS_PER_YEAR);
    events.retain(|e| e.timestamp_ns >= cutoff);
}

// ---------------------------------------------------------------------------
// Rebalance computation (pure – no IO)
// ---------------------------------------------------------------------------

/// Computes how many tokens should be minted to a reserve.
///
/// The mint amount is the **larger** of two independent deficits:
///
/// * **cyclical** = `max(0, target_balance − current_balance)`
/// * **lifetime** = `max(0, lifetime_received_minimum − lifetime_minted)`
///
/// The raw amount is then successively capped by:
///
/// 1. `max_topup_per_rebalance` (per-reserve cap per operation)
/// 2. Remaining `max_balance` room
/// 3. Remaining `lifetime_received_maximum` budget
/// 4. `max_mint_per_operation` (global policy cap)
///
/// Rate-limit capping is applied *outside* this function by the caller.
pub(crate) fn compute_rebalance(
    config: &ReserveConfig,
    current_balance: &Nat,
    lifetime_minted: &Nat,
    policy: &GlobalPolicy,
) -> Result<ComputedRebalance, MinterError> {
    let zero = Nat::from(0_u64);

    if !policy.minting_enabled {
        return Err(MinterError::MintingDisabled);
    }

    if !config.enabled {
        return Ok(ComputedRebalance {
            action: RebalanceAction::Skipped {
                reason: String::from("Reserve is disabled"),
            },
            mint_amount: zero.clone(),
            cyclical_deficit: zero.clone(),
            lifetime_deficit: zero,
        });
    }

    if !config.allow_auto_rebalance {
        return Ok(ComputedRebalance {
            action: RebalanceAction::Skipped {
                reason: String::from("Auto-rebalance not allowed for this reserve"),
            },
            mint_amount: zero.clone(),
            cyclical_deficit: zero.clone(),
            lifetime_deficit: zero,
        });
    }

    let cyclical_deficit = if current_balance >= &config.target_balance {
        zero.clone()
    } else {
        config.target_balance.clone() - current_balance.clone()
    };

    let lifetime_deficit = match &config.lifetime_received_minimum {
        Some(min) if min > lifetime_minted => min.clone() - lifetime_minted.clone(),
        _ => zero.clone(),
    };

    let mut amount = if lifetime_deficit > cyclical_deficit {
        lifetime_deficit.clone()
    } else {
        cyclical_deficit.clone()
    };

    if amount == 0_u64 {
        return Ok(ComputedRebalance {
            action: RebalanceAction::AlreadyFunded,
            mint_amount: zero,
            cyclical_deficit,
            lifetime_deficit,
        });
    }

    if let Some(max_topup) = &config.max_topup_per_rebalance {
        if &amount > max_topup {
            amount = max_topup.clone();
        }
    }

    if let Some(max_balance) = &config.max_balance {
        if max_balance > current_balance {
            let room = max_balance.clone() - current_balance.clone();
            if amount > room {
                amount = room;
            }
        } else {
            return Ok(ComputedRebalance {
                action: RebalanceAction::Skipped {
                    reason: String::from("Balance already at or above max_balance"),
                },
                mint_amount: zero,
                cyclical_deficit,
                lifetime_deficit,
            });
        }
    }

    if let Some(max_lifetime) = &config.lifetime_received_maximum {
        if max_lifetime > lifetime_minted {
            let remaining = max_lifetime.clone() - lifetime_minted.clone();
            if amount > remaining {
                amount = remaining;
            }
        } else {
            return Ok(ComputedRebalance {
                action: RebalanceAction::Skipped {
                    reason: String::from("Lifetime maximum already reached"),
                },
                mint_amount: zero,
                cyclical_deficit,
                lifetime_deficit,
            });
        }
    }

    if let Some(max_per_op) = &policy.max_mint_per_operation {
        if &amount > max_per_op {
            amount = max_per_op.clone();
        }
    }

    if amount == 0_u64 {
        return Ok(ComputedRebalance {
            action: RebalanceAction::Skipped {
                reason: String::from("Amount capped to zero by policy or config limits"),
            },
            mint_amount: zero,
            cyclical_deficit,
            lifetime_deficit,
        });
    }

    Ok(ComputedRebalance {
        action: RebalanceAction::Minted,
        mint_amount: amount,
        cyclical_deficit,
        lifetime_deficit,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use candid::{Nat, Principal};
    use icrc_ledger_types::icrc1::account::Account;

    use super::{
        available_mint_budget, check_rate_limits, compute_rebalance, prune_old_events,
        validate_balance_invariants, validate_rate_limits, NANOS_PER_DAY, NANOS_PER_HOUR,
        NANOS_PER_WEEK,
    };
    use crate::model::{
        GlobalPolicy, MintEvent, MinterError, RateLimits, RebalanceAction, ReserveConfig,
    };

    fn sample_config() -> ReserveConfig {
        ReserveConfig {
            account: Account {
                owner: Principal::anonymous(),
                subaccount: None,
            },
            min_balance: Nat::from(100_u64),
            target_balance: Nat::from(1000_u64),
            max_balance: None,
            max_topup_per_rebalance: None,
            lifetime_received_minimum: None,
            lifetime_received_maximum: None,
            rate_limits: None,
            enabled: true,
            allow_manual_topup: true,
            allow_auto_rebalance: true,
            purpose: String::from("testing"),
            label: String::from("Test Reserve"),
        }
    }

    fn sample_policy() -> GlobalPolicy {
        GlobalPolicy {
            minting_enabled: true,
            max_mint_per_operation: None,
        }
    }

    // -- balance validation --------------------------------------------------

    #[test]
    fn valid_invariants_accepted() {
        let r = validate_balance_invariants(
            &Nat::from(100_u64),
            &Nat::from(1000_u64),
            Some(&Nat::from(5000_u64)),
            None,
            None,
            "ok",
        );
        assert!(r.is_ok());
    }

    #[test]
    fn rejects_min_greater_than_target() {
        let r = validate_balance_invariants(
            &Nat::from(2000_u64),
            &Nat::from(1000_u64),
            None,
            None,
            None,
            "ok",
        );
        assert!(r.is_err());
    }

    #[test]
    fn rejects_target_greater_than_max() {
        let r = validate_balance_invariants(
            &Nat::from(100_u64),
            &Nat::from(1000_u64),
            Some(&Nat::from(500_u64)),
            None,
            None,
            "ok",
        );
        assert!(r.is_err());
    }

    #[test]
    fn rejects_empty_label() {
        let r = validate_balance_invariants(
            &Nat::from(100_u64),
            &Nat::from(1000_u64),
            None,
            None,
            None,
            "",
        );
        assert!(r.is_err());
    }

    #[test]
    fn valid_lifetime_min_max_pair() {
        let r = validate_balance_invariants(
            &Nat::from(100_u64),
            &Nat::from(1000_u64),
            None,
            Some(&Nat::from(500_u64)),
            Some(&Nat::from(5000_u64)),
            "ok",
        );
        assert!(r.is_ok());
    }

    #[test]
    fn rejects_lifetime_max_below_min() {
        let r = validate_balance_invariants(
            &Nat::from(100_u64),
            &Nat::from(1000_u64),
            None,
            Some(&Nat::from(5000_u64)),
            Some(&Nat::from(1000_u64)),
            "ok",
        );
        assert!(r.is_err());
    }

    // -- rate-limit validation -----------------------------------------------

    #[test]
    fn valid_decreasing_rate_limits() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(100_u64)),
            max_amount_per_day: Some(Nat::from(2000_u64)), // 2000/24 ≈ 83 < 100
            max_amount_per_week: Some(Nat::from(10000_u64)), // 10000/168 ≈ 60 < 83
            max_amount_per_month: None,
            max_amount_per_year: Some(Nat::from(100_000_u64)), // 100000/8760 ≈ 11 < 60
        };
        assert!(validate_rate_limits(&limits).is_ok());
    }

    #[test]
    fn rejects_non_decreasing_rate_limits() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(100_u64)),
            max_amount_per_day: Some(Nat::from(2400_u64)), // 2400/24 = 100, not strictly less
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        assert!(validate_rate_limits(&limits).is_err());
    }

    #[test]
    fn rejects_increasing_rate_limits() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(100_u64)),
            max_amount_per_day: Some(Nat::from(3000_u64)), // 3000/24 = 125 > 100
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        assert!(validate_rate_limits(&limits).is_err());
    }

    #[test]
    fn single_window_always_valid() {
        let limits = RateLimits {
            max_amount_per_hour: None,
            max_amount_per_day: Some(Nat::from(1000_u64)),
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        assert!(validate_rate_limits(&limits).is_ok());
    }

    #[test]
    fn non_adjacent_windows_validated() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(100_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: Some(Nat::from(800_u64)), // 800/8760 ≈ 0.09 < 100
        };
        assert!(validate_rate_limits(&limits).is_ok());
    }

    // -- available_mint_budget -----------------------------------------------

    #[test]
    fn budget_no_limits_returns_none() {
        assert!(available_mint_budget(None, &[], 1_000_000).is_none());
    }

    #[test]
    fn budget_no_events_returns_full_limit() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let budget = available_mint_budget(Some(&limits), &[], NANOS_PER_HOUR * 2);
        assert_eq!(budget, Some(Nat::from(500_u64)));
    }

    #[test]
    fn budget_partially_used() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let now = NANOS_PER_HOUR * 2;
        let events = vec![MintEvent {
            timestamp_ns: now - NANOS_PER_HOUR / 2,
            amount: Nat::from(200_u64),
        }];
        let budget = available_mint_budget(Some(&limits), &events, now);
        assert_eq!(budget, Some(Nat::from(300_u64)));
    }

    #[test]
    fn budget_exhausted() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let now = NANOS_PER_HOUR * 2;
        let events = vec![MintEvent {
            timestamp_ns: now - 100,
            amount: Nat::from(600_u64),
        }];
        let budget = available_mint_budget(Some(&limits), &events, now);
        assert_eq!(budget, Some(Nat::from(0_u64)));
    }

    #[test]
    fn budget_uses_tightest_window() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: Some(Nat::from(800_u64)),
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let now = NANOS_PER_DAY * 2;
        let events = vec![MintEvent {
            timestamp_ns: now - NANOS_PER_HOUR / 2,
            amount: Nat::from(100_u64),
        }];
        let budget = available_mint_budget(Some(&limits), &events, now);
        assert_eq!(budget, Some(Nat::from(400_u64)));
    }

    #[test]
    fn old_events_outside_window_ignored() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let now = NANOS_PER_DAY;
        let events = vec![MintEvent {
            timestamp_ns: now - NANOS_PER_HOUR * 2,
            amount: Nat::from(9999_u64),
        }];
        let budget = available_mint_budget(Some(&limits), &events, now);
        assert_eq!(budget, Some(Nat::from(500_u64)));
    }

    // -- check_rate_limits ---------------------------------------------------

    #[test]
    fn check_passes_when_within_budget() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let r = check_rate_limits(&limits, &[], &Nat::from(400_u64), NANOS_PER_HOUR * 2);
        assert!(r.is_ok());
    }

    #[test]
    fn check_fails_when_exceeds_budget() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let r = check_rate_limits(&limits, &[], &Nat::from(600_u64), NANOS_PER_HOUR * 2);
        assert!(matches!(r, Err(MinterError::RateLimitExceeded { .. })));
    }

    #[test]
    fn check_accounts_for_existing_usage() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(500_u64)),
            max_amount_per_day: None,
            max_amount_per_week: None,
            max_amount_per_month: None,
            max_amount_per_year: None,
        };
        let now = NANOS_PER_HOUR * 2;
        let events = vec![MintEvent {
            timestamp_ns: now - 100,
            amount: Nat::from(400_u64),
        }];
        let r = check_rate_limits(&limits, &events, &Nat::from(200_u64), now);
        assert!(matches!(r, Err(MinterError::RateLimitExceeded { .. })));
    }

    // -- prune ---------------------------------------------------------------

    #[test]
    fn prune_removes_old_events() {
        let now = NANOS_PER_WEEK * 100;
        let mut events = vec![
            MintEvent {
                timestamp_ns: 1,
                amount: Nat::from(10_u64),
            },
            MintEvent {
                timestamp_ns: now - NANOS_PER_DAY,
                amount: Nat::from(20_u64),
            },
        ];
        prune_old_events(&mut events, now);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].amount, Nat::from(20_u64));
    }

    // -- compute_rebalance (cyclical only) -----------------------------------

    #[test]
    fn already_funded_when_at_target() {
        let c = sample_config();
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(1000_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::AlreadyFunded);
        assert_eq!(r.mint_amount, Nat::from(0_u64));
    }

    #[test]
    fn already_funded_when_above_target() {
        let c = sample_config();
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(2000_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::AlreadyFunded);
    }

    #[test]
    fn mints_full_deficit() {
        let c = sample_config();
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(700_u64));
    }

    #[test]
    fn caps_at_max_topup_per_rebalance() {
        let mut c = sample_config();
        c.max_topup_per_rebalance = Some(Nat::from(200_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(200_u64));
    }

    #[test]
    fn caps_at_max_balance_room() {
        let mut c = sample_config();
        c.target_balance = Nat::from(500_u64);
        c.max_balance = Some(Nat::from(500_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(200_u64));
    }

    #[test]
    fn caps_at_policy_max_per_operation() {
        let c = sample_config();
        let mut p = sample_policy();
        p.max_mint_per_operation = Some(Nat::from(100_u64));
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(100_u64));
    }

    #[test]
    fn skips_disabled_reserve() {
        let mut c = sample_config();
        c.enabled = false;
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(0_u64), &Nat::from(0_u64), &p).unwrap();
        assert!(matches!(r.action, RebalanceAction::Skipped { .. }));
    }

    #[test]
    fn skips_when_auto_rebalance_off() {
        let mut c = sample_config();
        c.allow_auto_rebalance = false;
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(0_u64), &Nat::from(0_u64), &p).unwrap();
        assert!(matches!(r.action, RebalanceAction::Skipped { .. }));
    }

    #[test]
    fn errors_when_minting_disabled() {
        let c = sample_config();
        let mut p = sample_policy();
        p.minting_enabled = false;
        let r = compute_rebalance(&c, &Nat::from(0_u64), &Nat::from(0_u64), &p);
        assert!(r.is_err());
    }

    #[test]
    fn skips_when_max_topup_is_zero() {
        let mut c = sample_config();
        c.max_topup_per_rebalance = Some(Nat::from(0_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert!(matches!(r.action, RebalanceAction::Skipped { .. }));
    }

    #[test]
    fn mints_from_zero_to_target() {
        let c = sample_config();
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(0_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(1000_u64));
    }

    // -- compute_rebalance (lifetime guarantee) ------------------------------

    #[test]
    fn lifetime_deficit_drives_mint_when_balance_at_target() {
        let mut c = sample_config();
        c.lifetime_received_minimum = Some(Nat::from(5000_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(1000_u64), &Nat::from(500_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(4500_u64));
        assert_eq!(r.cyclical_deficit, Nat::from(0_u64));
        assert_eq!(r.lifetime_deficit, Nat::from(4500_u64));
    }

    #[test]
    fn cyclical_wins_when_larger_than_lifetime() {
        let mut c = sample_config();
        c.lifetime_received_minimum = Some(Nat::from(200_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(100_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(700_u64));
    }

    #[test]
    fn lifetime_wins_when_larger_than_cyclical() {
        let mut c = sample_config();
        c.lifetime_received_minimum = Some(Nat::from(5000_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(5000_u64));
    }

    #[test]
    fn lifetime_satisfied_no_extra_mint() {
        let mut c = sample_config();
        c.lifetime_received_minimum = Some(Nat::from(1000_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(1000_u64), &Nat::from(1000_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::AlreadyFunded);
    }

    #[test]
    fn lifetime_capped_by_max_balance() {
        let mut c = sample_config();
        c.lifetime_received_minimum = Some(Nat::from(5000_u64));
        c.max_balance = Some(Nat::from(2000_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(1000_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(1000_u64));
    }

    #[test]
    fn no_lifetime_set_behaves_as_before() {
        let c = sample_config();
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(9999_u64), &p).unwrap();
        assert_eq!(r.mint_amount, Nat::from(700_u64));
    }

    // -- compute_rebalance (lifetime maximum cap) ----------------------------

    #[test]
    fn lifetime_max_caps_mint_amount() {
        let mut c = sample_config();
        c.lifetime_received_maximum = Some(Nat::from(800_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(500_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(300_u64));
    }

    #[test]
    fn lifetime_max_reached_skips() {
        let mut c = sample_config();
        c.lifetime_received_maximum = Some(Nat::from(500_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(500_u64), &p).unwrap();
        assert!(matches!(r.action, RebalanceAction::Skipped { .. }));
        assert_eq!(r.mint_amount, Nat::from(0_u64));
    }

    #[test]
    fn lifetime_max_exceeded_skips() {
        let mut c = sample_config();
        c.lifetime_received_maximum = Some(Nat::from(500_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(600_u64), &p).unwrap();
        assert!(matches!(r.action, RebalanceAction::Skipped { .. }));
    }

    #[test]
    fn lifetime_max_no_impact_when_plenty_of_budget() {
        let mut c = sample_config();
        c.lifetime_received_maximum = Some(Nat::from(99999_u64));
        let p = sample_policy();
        let r = compute_rebalance(&c, &Nat::from(300_u64), &Nat::from(0_u64), &p).unwrap();
        assert_eq!(r.action, RebalanceAction::Minted);
        assert_eq!(r.mint_amount, Nat::from(700_u64));
    }
}
