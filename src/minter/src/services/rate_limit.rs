use candid::Nat;

use crate::{
    model::{MintEvent, MinterError, RateLimits},
    services::rate_windows::WINDOWS,
};

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

    let mut budget: Option<Nat> = None;

    for window in &WINDOWS {
        if let Some(limit) = (window.limit)(limits) {
            let used = usage_in_window(events, now_ns, window.nanos());
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
    for window in &WINDOWS {
        if let Some(limit) = (window.limit)(limits) {
            let used = usage_in_window(events, now_ns, window.nanos());
            let projected = used.clone() + amount.clone();
            if &projected > limit {
                return Err(MinterError::RateLimitExceeded {
                    window: String::from(window.label),
                    limit: limit.clone(),
                    current_usage: used,
                    requested: amount.clone(),
                });
            }
        }
    }

    Ok(())
}

/// Removes mint events older than the largest rate-limit window.
///
/// Should be called after every successful mint to keep the event list
/// bounded.
pub(crate) fn prune_old_events(events: &mut Vec<MintEvent>, now_ns: u64) {
    let cutoff = now_ns.saturating_sub(WINDOWS[WINDOWS.len() - 1].nanos());
    events.retain(|e| e.timestamp_ns >= cutoff);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use candid::Nat;

    use super::{available_mint_budget, check_rate_limits, prune_old_events};
    use crate::{
        model::{MintEvent, MinterError, RateLimits},
        services::rate_windows::NANOS_PER_HOUR,
    };

    const NANOS_PER_DAY: u64 = 86_400_000_000_000;
    const NANOS_PER_WEEK: u64 = 604_800_000_000_000;

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
}
