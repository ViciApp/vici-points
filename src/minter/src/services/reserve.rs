use candid::Nat;

use crate::{
    model::{
        AddReserveArg, ComputedRebalance, GlobalPolicy, MinterError, RebalanceAction, ReserveConfig,
    },
    services::rate_windows,
};

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
#[expect(
    clippy::too_many_arguments,
    reason = "baseline, pinned in scripts/lint.fn-size.sh"
)]
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
        rate_windows::validate_rate_limits(limits)?;
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
        rate_windows::validate_rate_limits(limits)?;
    }
    Ok(())
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
#[expect(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "baseline, pinned in scripts/lint.fn-size.sh"
)]
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
        return Ok(skipped("Reserve is disabled", zero.clone(), zero));
    }

    if !config.allow_auto_rebalance {
        return Ok(skipped(
            "Auto-rebalance not allowed for this reserve",
            zero.clone(),
            zero,
        ));
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
            return Ok(skipped(
                "Balance already at or above max_balance",
                cyclical_deficit,
                lifetime_deficit,
            ));
        }
    }

    if let Some(max_lifetime) = &config.lifetime_received_maximum {
        if max_lifetime > lifetime_minted {
            let remaining = max_lifetime.clone() - lifetime_minted.clone();
            if amount > remaining {
                amount = remaining;
            }
        } else {
            return Ok(skipped(
                "Lifetime maximum already reached",
                cyclical_deficit,
                lifetime_deficit,
            ));
        }
    }

    if let Some(max_per_op) = &policy.max_mint_per_operation {
        if &amount > max_per_op {
            amount = max_per_op.clone();
        }
    }

    if amount == 0_u64 {
        return Ok(skipped(
            "Amount capped to zero by policy or config limits",
            cyclical_deficit,
            lifetime_deficit,
        ));
    }

    Ok(ComputedRebalance {
        action: RebalanceAction::Minted,
        mint_amount: amount,
        cyclical_deficit,
        lifetime_deficit,
    })
}

/// A [`ComputedRebalance`] that mints nothing, with the reason and the
/// deficits found before the check that stopped it.
fn skipped(reason: &str, cyclical_deficit: Nat, lifetime_deficit: Nat) -> ComputedRebalance {
    ComputedRebalance {
        action: RebalanceAction::Skipped {
            reason: String::from(reason),
        },
        mint_amount: Nat::from(0_u64),
        cyclical_deficit,
        lifetime_deficit,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use candid::{Nat, Principal};
    use icrc_ledger_types::icrc1::account::Account;

    use super::{compute_rebalance, validate_balance_invariants};
    use crate::model::{GlobalPolicy, RebalanceAction, ReserveConfig};

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
