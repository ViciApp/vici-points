use candid::Nat;
use ic_cdk::api::time;

use crate::{
    model::{MintEvent, MinterError, RebalanceAction, RebalanceResponse},
    services::{ledger, reserve},
    state::STATE,
};

fn release_mint_lock(reserve_id: u64) {
    STATE.with_borrow_mut(|s| {
        s.reserve_mint_inflight.remove(&reserve_id);
    });
}

/// Rebalances a single reserve: queries its ledger balance, computes the
/// deficit, applies all caps and rate limits, and mints if needed.
///
/// Takes a per-reserve mint lock for the duration of the call.  If another
/// update is already minting to this reserve, returns
/// [`MinterError::ReserveOperationInProgress`].
pub(crate) async fn rebalance_one(id: u64) -> Result<RebalanceResponse, MinterError> {
    let (ledger_id, config, lifetime_minted, mint_events, policy) =
        STATE.with_borrow_mut(|s| -> Result<_, MinterError> {
            let record = s
                .reserves
                .get(&id)
                .ok_or(MinterError::ReserveNotFound { id })?;
            if !s.reserve_mint_inflight.insert(id) {
                return Err(MinterError::ReserveOperationInProgress { id });
            }
            Ok((
                s.ledger_id,
                record.config.clone(),
                record.lifetime_minted.clone(),
                record.mint_events.clone(),
                s.global_policy.clone(),
            ))
        })?;

    let balance = match ledger::get_balance(ledger_id, &config.account).await {
        Ok(b) => b,
        Err(e) => {
            release_mint_lock(id);
            return Err(e);
        }
    };

    let computed = match reserve::compute_rebalance(&config, &balance, &lifetime_minted, &policy) {
        Ok(c) => c,
        Err(e) => {
            release_mint_lock(id);
            return Err(e);
        }
    };

    match computed.action {
        RebalanceAction::Minted => {
            let mut mint_amount = computed.mint_amount;

            let now = time();
            if let Some(budget) =
                reserve::available_mint_budget(config.rate_limits.as_ref(), &mint_events, now)
            {
                if budget == 0_u64 {
                    release_mint_lock(id);
                    return Ok(RebalanceResponse {
                        reserve_id: id,
                        action: RebalanceAction::Skipped {
                            reason: String::from("Rate limit budget exhausted"),
                        },
                        balance_before: balance,
                        minted_amount: Nat::from(0_u64),
                        ledger_block_index: None,
                    });
                }
                if mint_amount > budget {
                    mint_amount = budget;
                }
            }

            let memo = format!("rebalance:{id}");
            let block_index =
                match ledger::mint_to(ledger_id, &config.account, mint_amount.clone(), &memo).await
                {
                    Ok(b) => b,
                    Err(e) => {
                        release_mint_lock(id);
                        return Err(e);
                    }
                };

            STATE.with_borrow_mut(|s| -> Result<(), MinterError> {
                s.reserve_mint_inflight.remove(&id);
                let record = s.reserves.get_mut(&id).ok_or_else(|| {
                    MinterError::InternalInvariantViolated {
                        reason: String::from(
                            "reserve missing after successful ledger mint; \
                             call release_stuck_mint_state if needed",
                        ),
                    }
                })?;
                record.lifetime_minted += mint_amount.clone();
                record.mint_events.push(MintEvent {
                    timestamp_ns: now,
                    amount: mint_amount.clone(),
                });
                reserve::prune_old_events(&mut record.mint_events, now);
                Ok(())
            })?;

            Ok(RebalanceResponse {
                reserve_id: id,
                action: RebalanceAction::Minted,
                balance_before: balance,
                minted_amount: mint_amount,
                ledger_block_index: Some(block_index),
            })
        }
        action => {
            release_mint_lock(id);
            Ok(RebalanceResponse {
                reserve_id: id,
                action,
                balance_before: balance,
                minted_amount: Nat::from(0_u64),
                ledger_block_index: None,
            })
        }
    }
}

/// Rebalances every registered reserve sequentially.
///
/// Individual failures do not prevent remaining reserves from being
/// processed.
pub(crate) async fn rebalance_all() -> Vec<Result<RebalanceResponse, MinterError>> {
    let ids: Vec<u64> = STATE.with_borrow(|s| s.reserves.keys().copied().collect());
    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        results.push(rebalance_one(id).await);
    }
    results
}
