use candid::{Nat, Principal};
use ic_cdk::{api::time, call::Call};
use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::{Memo, TransferArg, TransferError},
};

use crate::model::MinterError;

/// Queries the ledger for the current balance of `account`.
///
/// Maps any inter-canister call failure into [`MinterError::LedgerError`].
pub(crate) async fn get_balance(
    ledger_id: Principal,
    account: &Account,
) -> Result<Nat, MinterError> {
    let response = Call::bounded_wait(ledger_id, "icrc1_balance_of")
        .with_args(&(account,))
        .await
        .map_err(|e| MinterError::LedgerError {
            message: format!("Balance query failed: {e:?}"),
        })?;
    let (balance,) = response
        .candid_tuple()
        .map_err(|e| MinterError::LedgerError {
            message: format!("Balance query response decode failed: {e:?}"),
        })?;
    Ok(balance)
}

/// Mints `amount` tokens to `to` by calling `icrc1_transfer` on the ledger.
///
/// Because the minter canister *is* the ledger's minting account, this
/// transfer creates new tokens.  A `TransferError::Duplicate` is treated
/// as a success (idempotent replay).
///
/// `memo_text` is truncated to 32 bytes and attached to the transaction.
pub(crate) async fn mint_to(
    ledger_id: Principal,
    to: &Account,
    amount: Nat,
    memo_text: &str,
) -> Result<Nat, MinterError> {
    let memo_bytes: Vec<u8> = memo_text.as_bytes().iter().copied().take(32).collect();
    let arg = TransferArg {
        from_subaccount: None,
        to: *to,
        amount,
        fee: None,
        memo: Some(Memo(memo_bytes.into())),
        created_at_time: Some(time()),
    };

    let response = Call::bounded_wait(ledger_id, "icrc1_transfer")
        .with_arg(arg)
        .await
        .map_err(|e| MinterError::LedgerError {
            message: format!("Transfer call failed: {e:?}"),
        })?;
    let (result,) = response
        .candid_tuple()
        .map_err(|e| MinterError::LedgerError {
            message: format!("Transfer response decode failed: {e:?}"),
        })?;

    match result {
        Ok(block_index) => Ok(block_index),
        Err(TransferError::Duplicate { duplicate_of }) => Ok(duplicate_of),
        Err(e) => Err(MinterError::LedgerError {
            message: format!("Transfer error: {e:?}"),
        }),
    }
}
