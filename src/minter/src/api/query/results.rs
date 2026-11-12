use candid::{CandidType, Nat};
use serde::{Deserialize, Serialize};

use crate::model::{MinterError, ReserveConfig, ReserveRecord};

/// Read-only snapshot of a reserve returned by query methods.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ReserveInfo {
    /// Numeric identifier assigned by the minter.
    pub id: u64,
    /// Full configuration of the reserve (same Candid shape as [`crate::model::AddReserveArg`]).
    pub config: ReserveConfig,
    /// Cumulative total of tokens minted to this reserve since creation.
    pub lifetime_minted: Nat,
}

impl ReserveInfo {
    /// The snapshot of the stored reserve `record` registered under `id`.
    pub(crate) fn new(id: u64, record: &ReserveRecord) -> Self {
        Self {
            id,
            config: record.config.clone(),
            lifetime_minted: record.lifetime_minted.clone(),
        }
    }
}

/// Outcome of retrieving a single reserve's information.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum GetReserveResult {
    /// Successfully retrieved the reserve information.
    Ok(Box<ReserveInfo>),
    /// Failed to retrieve the reserve.
    Err(MinterError),
}

impl From<Result<ReserveInfo, MinterError>> for GetReserveResult {
    fn from(value: Result<ReserveInfo, MinterError>) -> Self {
        match value {
            Ok(v) => GetReserveResult::Ok(Box::new(v)),
            Err(e) => GetReserveResult::Err(e),
        }
    }
}
