use candid::Nat;

use crate::model::{MinterError, RateLimits};

// ---------------------------------------------------------------------------
// Rate-limit windows
// ---------------------------------------------------------------------------

/// Nanoseconds in one hour (3 600 s), the unit every window is counted in.
pub(crate) const NANOS_PER_HOUR: u64 = 3_600_000_000_000;

/// One rolling rate-limit window.
pub(crate) struct Window {
    /// The window's name in `MinterError::RateLimitExceeded`.
    pub(crate) label: &'static str,
    /// Length of the window in hours; months are 30 days, years 365.
    hours: u64,
    /// The cap configured for this window, if any.
    pub(crate) limit: fn(&RateLimits) -> &Option<Nat>,
}

impl Window {
    const fn new(label: &'static str, hours: u64, limit: fn(&RateLimits) -> &Option<Nat>) -> Self {
        Self {
            label,
            hours,
            limit,
        }
    }

    /// Length of the window in nanoseconds.
    pub(crate) const fn nanos(&self) -> u64 {
        self.hours * NANOS_PER_HOUR
    }
}

/// Every rate-limit window, from the smallest to the largest.
pub(crate) const WINDOWS: [Window; 5] = [
    Window::new("hour", 1, |l| &l.max_amount_per_hour),
    Window::new("day", 24, |l| &l.max_amount_per_day),
    Window::new("week", 168, |l| &l.max_amount_per_week),
    Window::new("month", 720, |l| &l.max_amount_per_month),
    Window::new("year", 8760, |l| &l.max_amount_per_year),
];

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
    let windows: Vec<(u64, Nat)> = WINDOWS
        .iter()
        .filter_map(|w| (w.limit)(limits).clone().map(|l| (w.hours, l)))
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use candid::Nat;

    use super::{validate_rate_limits, WINDOWS};
    use crate::model::RateLimits;

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

    #[test]
    fn windows_are_an_hour_a_day_a_week_30_and_365_days_each_with_its_cap() {
        let limits = RateLimits {
            max_amount_per_hour: Some(Nat::from(1_u64)),
            max_amount_per_day: Some(Nat::from(2_u64)),
            max_amount_per_week: Some(Nat::from(3_u64)),
            max_amount_per_month: Some(Nat::from(4_u64)),
            max_amount_per_year: Some(Nat::from(5_u64)),
        };
        let windows: Vec<(&str, u64, Option<Nat>)> = WINDOWS
            .iter()
            .map(|w| (w.label, w.nanos(), (w.limit)(&limits).clone()))
            .collect();
        let cap = |n: u64| Some(Nat::from(n));
        assert_eq!(
            windows,
            vec![
                ("hour", 3_600_000_000_000, cap(1)),
                ("day", 86_400_000_000_000, cap(2)),
                ("week", 604_800_000_000_000, cap(3)),
                ("month", 2_592_000_000_000_000, cap(4)),
                ("year", 31_536_000_000_000_000, cap(5)),
            ]
        );
    }
}
