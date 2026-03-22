use core::{cell::RefCell, clone::Clone};
use std::collections::{BTreeMap, BTreeSet};

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

use crate::model::{GlobalPolicy, IdempotencyRecord, ReserveRecord};

/// The complete runtime state of the minter canister.
///
/// Serialised to stable memory on `pre_upgrade` and restored on
/// `post_upgrade` so that data survives canister code upgrades.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct State {
    /// Principal of the ICRC-1 ledger canister this minter operates on.
    pub ledger_id: Principal,
    /// Monotonically increasing counter used to assign unique reserve ids.
    pub next_reserve_id: u64,
    /// All registered reserves, keyed by their numeric id.
    pub reserves: BTreeMap<u64, ReserveRecord>,
    /// Canister-wide minting policy (e.g. master enable flag, per-op cap).
    pub global_policy: GlobalPolicy,
    /// Idempotency map for manual top-ups, keyed by caller-provided
    /// idempotency key.
    pub idempotency: BTreeMap<String, IdempotencyRecord>,
    /// Reserve ids currently executing a ledger mint (rebalance or manual).
    ///
    /// Prevents interleaved updates from double-minting across `await` points.
    pub reserve_mint_inflight: BTreeSet<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ledger_id: Principal::anonymous(),
            next_reserve_id: 0,
            reserves: BTreeMap::new(),
            global_policy: GlobalPolicy::default(),
            idempotency: BTreeMap::new(),
            reserve_mint_inflight: BTreeSet::new(),
        }
    }
}

thread_local! {
    /// Global canister state, accessed via `STATE.with_borrow(|s| …)`.
    pub static STATE: RefCell<State> = RefCell::new(State::default());
}

// ---------------------------------------------------------------------------
// Stable-memory persistence (pre/post upgrade)
// ---------------------------------------------------------------------------

/// Serialises the current [`State`] into IC stable memory.
///
/// The layout is a 4-byte little-endian length prefix followed by the
/// Candid-encoded payload.  Called from the `pre_upgrade` hook.
pub fn save_state() {
    use candid::encode_one;
    use ic_cdk::stable::{stable_grow, stable_size, stable_write};

    let state = STATE.with_borrow(Clone::clone);
    let bytes = encode_one(&state).expect("failed to encode minter state");
    #[expect(clippy::cast_possible_truncation)]
    // IC stable layout uses u32 length; payload capped by memory
    let len = bytes.len() as u32;
    let total = u64::from(len) + 4;
    let needed_pages = total.div_ceil(65536);
    let current_pages = stable_size();
    if needed_pages > current_pages {
        stable_grow(needed_pages - current_pages).expect("failed to grow stable memory");
    }
    stable_write(0, &len.to_le_bytes());
    stable_write(4, &bytes);
}

/// Attempts to restore [`State`] from IC stable memory.
///
/// Returns `true` if state was successfully decoded and applied, `false`
/// if stable memory is empty or contains invalid data.  Called from the
/// `post_upgrade` hook.
pub fn try_restore_state() -> bool {
    use candid::decode_one;
    use ic_cdk::stable::{stable_read, stable_size};

    let size = stable_size();
    if size == 0 {
        return false;
    }
    let mut len_bytes = [0_u8; 4];
    stable_read(0, &mut len_bytes);
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len == 0 {
        return false;
    }
    #[expect(clippy::cast_possible_truncation)]
    // wasm32 page count × page size fits in usize on canister targets
    let max_readable = size as usize * 65536 - 4;
    if len > max_readable {
        return false;
    }
    let mut bytes = vec![0_u8; len];
    stable_read(4, &mut bytes);
    match decode_one::<State>(&bytes) {
        Ok(state) => {
            STATE.with_borrow_mut(|s| *s = state);
            true
        }
        Err(_) => false,
    }
}
