use candid::{Nat, Principal};
use ic_cdk::{api::time, update};

use super::{
    params::{AddReserveArg, ManualTopupArg, ReleaseStuckMintArg, UpdateReserveArg},
    results::{
        AddReserveResult, ManualTopupReserveResult, ManualTopupResponse,
        PreviewRebalanceReserveResult, PreviewRebalanceResponse, RebalanceReserveResult,
        ReleaseStuckMintResult, RemoveReserveResult, SetGlobalPolicyResult, UpdateReserveResult,
    },
};
use crate::{
    guards::{caller_is_controller, caller_is_not_anonymous},
    model::{
        GlobalPolicy, IdempotencyEntry, IdempotencyRecord, MintEvent, MinterError, ReserveConfig,
        ReserveRecord,
    },
    services::{ledger, rebalance, reserve},
    state::STATE,
};

// ---------------------------------------------------------------------------
// Reserve CRUD
// ---------------------------------------------------------------------------

fn add_reserve_impl(arg: AddReserveArg) -> Result<u64, MinterError> {
    reserve::validate_add_reserve_arg(&arg)?;

    STATE.with_borrow_mut(|s| {
        for record in s.reserves.values() {
            if record.config.account == arg.account {
                return Err(MinterError::ReserveAccountAlreadyExists {
                    account: arg.account,
                });
            }
        }

        let id = s.next_reserve_id;
        s.next_reserve_id += 1;

        s.reserves.insert(
            id,
            ReserveRecord {
                config: ReserveConfig {
                    account: arg.account,
                    min_balance: arg.min_balance,
                    target_balance: arg.target_balance,
                    max_balance: arg.max_balance,
                    max_topup_per_rebalance: arg.max_topup_per_rebalance,
                    lifetime_received_minimum: arg.lifetime_received_minimum,
                    lifetime_received_maximum: arg.lifetime_received_maximum,
                    rate_limits: arg.rate_limits,
                    enabled: arg.enabled,
                    allow_manual_topup: arg.allow_manual_topup,
                    allow_auto_rebalance: arg.allow_auto_rebalance,
                    purpose: arg.purpose,
                    label: arg.label,
                },
                lifetime_minted: Nat::from(0_u64),
                mint_events: Vec::new(),
            },
        );

        Ok(id)
    })
}

/// Registers a new reserve account.
///
/// Validates the configuration, ensures no duplicate account exists,
/// assigns a unique numeric id, and stores the reserve with zero lifetime
/// counters and an empty mint-event history.
///
/// Returns the newly assigned reserve id.
#[update(guard = "caller_is_controller")]
fn add_reserve(arg: AddReserveArg) -> AddReserveResult {
    add_reserve_impl(arg).into()
}

fn update_reserve_impl(arg: UpdateReserveArg) -> Result<(), MinterError> {
    STATE.with_borrow_mut(|s| {
        let record = s
            .reserves
            .get(&arg.id)
            .ok_or(MinterError::ReserveNotFound { id: arg.id })?;

        let mut updated = record.config.clone();
        let lifetime_minted = record.lifetime_minted.clone();
        let mint_events = record.mint_events.clone();

        if let Some(v) = arg.min_balance {
            updated.min_balance = v;
        }
        if let Some(v) = arg.target_balance {
            updated.target_balance = v;
        }
        if let Some(v) = arg.max_balance {
            updated.max_balance = v;
        }
        if let Some(v) = arg.max_topup_per_rebalance {
            updated.max_topup_per_rebalance = v;
        }
        if let Some(v) = arg.enabled {
            updated.enabled = v;
        }
        if let Some(v) = arg.allow_manual_topup {
            updated.allow_manual_topup = v;
        }
        if let Some(v) = arg.allow_auto_rebalance {
            updated.allow_auto_rebalance = v;
        }
        if let Some(v) = arg.purpose {
            updated.purpose = v;
        }
        if let Some(v) = arg.label {
            updated.label = v;
        }

        if let Some(new_min) = arg.lifetime_received_minimum {
            match &record.config.lifetime_received_minimum {
                Some(current) if &new_min < current => {
                    return Err(MinterError::InvalidConfig {
                        reason: String::from(
                            "lifetime_received_minimum can only increase, not decrease",
                        ),
                    });
                }
                _ => {
                    updated.lifetime_received_minimum = Some(new_min);
                }
            }
        }

        if let Some(v) = arg.lifetime_received_maximum {
            updated.lifetime_received_maximum = v;
        }

        if let Some(v) = arg.rate_limits {
            updated.rate_limits = v;
        }

        reserve::validate_reserve_config(&updated)?;

        s.reserves.insert(
            arg.id,
            ReserveRecord {
                config: updated,
                lifetime_minted,
                mint_events,
            },
        );
        Ok(())
    })
}

/// Partially updates the configuration of an existing reserve.
///
/// Only fields present in the argument are changed; omitted fields keep
/// their current values.  Special rules:
///
/// * `lifetime_received_minimum` can only increase, never decrease.
/// * `lifetime_received_maximum` and other `Option<Option<T>>` fields use the triple state: omit =
///   keep, `null` = clear, value = set (see [`UpdateReserveArg`]).
/// * The resulting configuration is re-validated before being persisted.
#[update(guard = "caller_is_controller")]
fn update_reserve(arg: UpdateReserveArg) -> UpdateReserveResult {
    update_reserve_impl(arg).into()
}

fn remove_reserve_impl(id: u64) -> Result<ReserveConfig, MinterError> {
    STATE.with_borrow_mut(|s| {
        if s.reserve_mint_inflight.contains(&id) {
            return Err(MinterError::ReserveOperationInProgress { id });
        }
        let pending_for_reserve = s.idempotency.values().any(|rec| {
            matches!(
                rec,
                IdempotencyRecord::Pending {
                    reserve_id: rid,
                } if *rid == id
            )
        });
        if pending_for_reserve {
            return Err(MinterError::ReserveOperationInProgress { id });
        }
        s.reserves
            .remove(&id)
            .map(|record| record.config)
            .ok_or(MinterError::ReserveNotFound { id })
    })
}

/// Removes a reserve and returns its configuration.
///
/// The reserve's lifetime counters and mint-event history are discarded.
/// Fails with [`MinterError::ReserveOperationInProgress`] if a mint is in flight to this reserve
/// or a manual idempotency entry is still **pending** for it.
#[update(guard = "caller_is_controller")]
fn remove_reserve(id: u64) -> RemoveReserveResult {
    remove_reserve_impl(id).into()
}

fn set_global_policy_impl(policy: GlobalPolicy) -> Result<(), MinterError> {
    STATE.with_borrow_mut(|s| s.global_policy = policy);
    Ok(())
}

/// Replaces the canister-wide global minting policy.
#[update(guard = "caller_is_controller")]
fn set_global_policy(policy: GlobalPolicy) -> SetGlobalPolicyResult {
    set_global_policy_impl(policy).into()
}

fn release_stuck_mint_state_impl(
    reserve_id: u64,
    idempotency_key: Option<&str>,
) -> Result<(), MinterError> {
    STATE.with_borrow_mut(|s| {
        s.reserve_mint_inflight.remove(&reserve_id);
        if let Some(k) = idempotency_key {
            match s.idempotency.get(k) {
                Some(IdempotencyRecord::Pending {
                    reserve_id: pending_rid,
                }) if *pending_rid == reserve_id => {
                    s.idempotency.remove(k);
                }
                Some(IdempotencyRecord::Pending { .. }) => {
                    return Err(MinterError::InvalidConfig {
                        reason: String::from("idempotency key is pending for a different reserve"),
                    });
                }
                Some(IdempotencyRecord::Completed(_)) | None => {}
            }
        }
        Ok(())
    })
}

/// Clears a stuck per-reserve mint lock and optionally removes a matching **pending**
/// idempotency entry (e.g. after a trap between a successful ledger mint and state commit).
///
/// Always removes `reserve_id` from the in-flight set.  If `idempotency_key` is set, removes that
/// key only when it is still pending for the same `reserve_id`; otherwise returns
/// `MinterError::InvalidConfig` if the key is pending for another reserve.
#[update(guard = "caller_is_controller")]
fn release_stuck_mint_state(
    ReleaseStuckMintArg {
        reserve_id,
        idempotency_key,
    }: ReleaseStuckMintArg,
) -> ReleaseStuckMintResult {
    release_stuck_mint_state_impl(reserve_id, idempotency_key.as_deref()).into()
}

// ---------------------------------------------------------------------------
// Mint coordination (no `await` inside these closures)
// ---------------------------------------------------------------------------

fn rollback_manual_mint_begin(reserve_id: u64, idempotency_key: Option<&str>) {
    STATE.with_borrow_mut(|s| {
        s.reserve_mint_inflight.remove(&reserve_id);
        if let Some(k) = idempotency_key {
            let drop_pending = matches!(
                s.idempotency.get(k),
                Some(IdempotencyRecord::Pending {
                    reserve_id: rid,
                }) if *rid == reserve_id
            );
            if drop_pending {
                s.idempotency.remove(k);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Rebalance (delegates to services::rebalance)
// ---------------------------------------------------------------------------

/// Triggers a rebalance for a single reserve by its id.
///
/// Takes a per-reserve mint lock for the duration of the call.  If another update is already
/// minting to this reserve, returns [`MinterError::ReserveOperationInProgress`].
#[update(guard = "caller_is_controller")]
async fn rebalance_reserve(id: u64) -> RebalanceReserveResult {
    rebalance::rebalance_one(id).await.into()
}

/// Triggers a rebalance for every registered reserve, sequentially.
///
/// Returns one result per reserve.  Individual failures do not prevent
/// remaining reserves from being processed.
#[update(guard = "caller_is_controller")]
async fn rebalance_all_reserves() -> Vec<RebalanceReserveResult> {
    rebalance::rebalance_all()
        .await
        .into_iter()
        .map(Into::into)
        .collect()
}

// ---------------------------------------------------------------------------
// Manual top-up
// ---------------------------------------------------------------------------

#[expect(clippy::large_enum_variant)] // `Started` is the hot path; boxing would add noise for an internal enum
enum ManualTopupBegin {
    Cached(ManualTopupResponse),
    Started {
        ledger_id: Principal,
        config: ReserveConfig,
        lifetime_minted: Nat,
        mint_events: Vec<MintEvent>,
        policy: GlobalPolicy,
    },
}

fn validate_manual_topup_inputs(
    arg: &ManualTopupArg,
    policy: &GlobalPolicy,
    config: &ReserveConfig,
    lifetime_minted: &Nat,
    mint_events: &[MintEvent],
    now_ns: u64,
) -> Result<(), MinterError> {
    if !policy.minting_enabled {
        return Err(MinterError::MintingDisabled);
    }
    if !config.enabled {
        return Err(MinterError::ReserveDisabled { id: arg.reserve_id });
    }
    if !config.allow_manual_topup {
        return Err(MinterError::ManualTopupNotAllowed { id: arg.reserve_id });
    }

    if let Some(max) = &policy.max_mint_per_operation {
        if &arg.amount > max {
            return Err(MinterError::AmountExceedsLimit {
                requested: arg.amount.clone(),
                limit: max.clone(),
            });
        }
    }

    if let Some(max_lifetime) = &config.lifetime_received_maximum {
        let projected = lifetime_minted.clone() + arg.amount.clone();
        if &projected > max_lifetime {
            return Err(MinterError::AmountExceedsLimit {
                requested: arg.amount.clone(),
                limit: max_lifetime.clone(),
            });
        }
    }

    if let Some(limits) = &config.rate_limits {
        reserve::check_rate_limits(limits, mint_events, &arg.amount, now_ns)?;
    }

    Ok(())
}

/// Core manual top-up logic, returning a standard `Result` for ergonomic
/// use of `?` internally.
async fn manual_topup_reserve_impl(
    arg: ManualTopupArg,
) -> Result<ManualTopupResponse, MinterError> {
    let idempotency_key = arg.idempotency_key.as_deref();

    let begin = STATE.with_borrow_mut(|s| -> Result<ManualTopupBegin, MinterError> {
        if let Some(k) = idempotency_key {
            match s.idempotency.get(k) {
                Some(IdempotencyRecord::Completed(e)) => {
                    return Ok(ManualTopupBegin::Cached(ManualTopupResponse {
                        reserve_id: e.reserve_id,
                        minted_amount: e.minted_amount.clone(),
                        ledger_block_index: e.ledger_block_index.clone(),
                    }));
                }
                Some(IdempotencyRecord::Pending { .. }) => {
                    return Err(MinterError::IdempotencyOperationInProgress {
                        key: String::from(k),
                    });
                }
                None => {}
            }
        }

        let record = s
            .reserves
            .get(&arg.reserve_id)
            .ok_or(MinterError::ReserveNotFound { id: arg.reserve_id })?;

        if !s.reserve_mint_inflight.insert(arg.reserve_id) {
            return Err(MinterError::ReserveOperationInProgress { id: arg.reserve_id });
        }

        if let Some(k) = idempotency_key {
            s.idempotency.insert(
                String::from(k),
                IdempotencyRecord::Pending {
                    reserve_id: arg.reserve_id,
                },
            );
        }

        Ok(ManualTopupBegin::Started {
            ledger_id: s.ledger_id,
            config: record.config.clone(),
            lifetime_minted: record.lifetime_minted.clone(),
            mint_events: record.mint_events.clone(),
            policy: s.global_policy.clone(),
        })
    })?;

    match begin {
        ManualTopupBegin::Cached(r) => Ok(r),
        ManualTopupBegin::Started {
            ledger_id,
            config,
            lifetime_minted,
            mint_events,
            policy,
        } => {
            let now = time();
            if let Err(e) = validate_manual_topup_inputs(
                &arg,
                &policy,
                &config,
                &lifetime_minted,
                &mint_events,
                now,
            ) {
                rollback_manual_mint_begin(arg.reserve_id, idempotency_key);
                return Err(e);
            }

            let memo = match idempotency_key {
                Some(key) => format!("topup:{}:{key}", arg.reserve_id),
                None => format!("topup:{}", arg.reserve_id),
            };

            let block_index = match ledger::mint_to(
                ledger_id,
                &config.account,
                arg.amount.clone(),
                &memo,
            )
            .await
            {
                Ok(b) => b,
                Err(e) => {
                    rollback_manual_mint_begin(arg.reserve_id, idempotency_key);
                    return Err(e);
                }
            };

            STATE.with_borrow_mut(|s| -> Result<(), MinterError> {
                s.reserve_mint_inflight.remove(&arg.reserve_id);
                let record = s.reserves.get_mut(&arg.reserve_id).ok_or_else(|| {
                    MinterError::InternalInvariantViolated {
                        reason: String::from(
                            "reserve missing after successful ledger mint; call release_stuck_mint_state if needed",
                        ),
                    }
                })?;
                record.lifetime_minted += arg.amount.clone();
                record.mint_events.push(MintEvent {
                    timestamp_ns: now,
                    amount: arg.amount.clone(),
                });
                reserve::prune_old_events(&mut record.mint_events, now);

                if let Some(k) = idempotency_key {
                    s.idempotency.insert(
                        String::from(k),
                        IdempotencyRecord::Completed(IdempotencyEntry {
                            ledger_block_index: block_index.clone(),
                            minted_amount: arg.amount.clone(),
                            reserve_id: arg.reserve_id,
                            executed_at_ns: now,
                        }),
                    );
                }
                Ok(())
            })?;

            Ok(ManualTopupResponse {
                reserve_id: arg.reserve_id,
                minted_amount: arg.amount,
                ledger_block_index: block_index,
            })
        }
    }
}

/// Mints a specific amount to a reserve as an ad-hoc (manual) top-up.
///
/// Unlike rebalancing, the caller specifies the exact amount.  The
/// operation enforces:
///
/// * Global policy (master enable, per-operation cap).
/// * Reserve flags (`enabled`, `allow_manual_topup`).
/// * Lifetime maximum cap.
/// * Rate limits (hard rejection if exceeded).
/// * Per-reserve mint lock: concurrent manual top-ups or rebalances to the same reserve return
///   [`MinterError::ReserveOperationInProgress`].
/// * Idempotency key: completed replays return the stored result; an in-flight key returns
///   [`MinterError::IdempotencyOperationInProgress`].
#[update(guard = "caller_is_controller")]
async fn manual_topup_reserve(arg: ManualTopupArg) -> ManualTopupReserveResult {
    manual_topup_reserve_impl(arg).await.into()
}

// ---------------------------------------------------------------------------
// Preview (update because they query the ledger)
// ---------------------------------------------------------------------------

/// Computes what a rebalance would do for a single reserve **without**
/// actually minting.
async fn preview_inner(id: u64) -> Result<PreviewRebalanceResponse, MinterError> {
    let (ledger_id, config, lifetime_minted, mint_events, policy) = STATE.with_borrow(|s| {
        let record = s
            .reserves
            .get(&id)
            .ok_or(MinterError::ReserveNotFound { id })?;
        Ok::<_, MinterError>((
            s.ledger_id,
            record.config.clone(),
            record.lifetime_minted.clone(),
            record.mint_events.clone(),
            s.global_policy.clone(),
        ))
    })?;

    let balance = ledger::get_balance(ledger_id, &config.account).await?;
    let computed = reserve::compute_rebalance(&config, &balance, &lifetime_minted, &policy)?;

    let now = time();
    let budget = reserve::available_mint_budget(config.rate_limits.as_ref(), &mint_events, now);

    let mut would_mint = computed.mint_amount;
    if let Some(b) = &budget {
        if &would_mint > b {
            would_mint = b.clone();
        }
    }

    Ok(PreviewRebalanceResponse {
        reserve_id: id,
        action: computed.action,
        current_balance: balance,
        target_balance: config.target_balance,
        cyclical_deficit: computed.cyclical_deficit,
        lifetime_deficit: computed.lifetime_deficit,
        would_mint,
        rate_limit_budget: budget,
    })
}

/// Previews the rebalance outcome for a single reserve.
#[update(guard = "caller_is_not_anonymous")]
async fn preview_rebalance_reserve(id: u64) -> PreviewRebalanceReserveResult {
    preview_inner(id).await.into()
}

/// Previews the rebalance outcome for every registered reserve.
#[update(guard = "caller_is_not_anonymous")]
async fn preview_rebalance_all_reserves() -> Vec<PreviewRebalanceReserveResult> {
    let ids: Vec<u64> = STATE.with_borrow(|s| s.reserves.keys().copied().collect());
    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        results.push(preview_inner(id).await.into());
    }
    results
}
