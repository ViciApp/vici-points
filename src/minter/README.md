# Minter Canister

The minter canister manages **reserve accounts** on the VICI ICRC-1 ledger. It is the ledger's designated minting account: only the minter can create new VICI tokens, and it does so exclusively for pre-approved system accounts called _reserves_.

No tokens are ever minted to arbitrary users. Every mint operation flows through the reserve system described below.

## Concepts

### Reserves

A reserve is a trusted ICRC-1 account (principal + optional subaccount) whose token balance the minter keeps topped up. Each reserve is identified by a numeric id assigned at creation and carries a configuration that controls how and when tokens are minted to it.

Key configuration fields:

| Field                       | Description                                                                                                     |
| --------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `account`                   | The ICRC-1 account that holds the reserve's tokens.                                                             |
| `min_balance`               | Balance threshold below which a rebalance is triggered.                                                         |
| `target_balance`            | Desired balance the minter aims to restore.                                                                     |
| `max_balance`               | Hard upper bound; the minter never pushes the balance above this.                                               |
| `max_topup_per_rebalance`   | Cap on how much can be minted in a single rebalance.                                                            |
| `lifetime_received_minimum` | Guaranteed minimum total amount this reserve must receive over its lifetime. Can only increase, never decrease. |
| `lifetime_received_maximum` | Hard cap on the total amount this reserve may ever receive.                                                     |
| `rate_limits`               | Optional sliding-window caps (per hour / day / week / month / year).                                            |
| `enabled`                   | Master enable flag for this reserve.                                                                            |
| `allow_manual_topup`        | Whether ad-hoc manual top-ups are permitted.                                                                    |
| `allow_auto_rebalance`      | Whether automatic rebalancing is permitted.                                                                     |
| `purpose`                   | Free-form description.                                                                                          |
| `label`                     | Short human-readable name (must not be empty).                                                                  |

### Rebalance logic

When a rebalance is triggered for a reserve, the minter:

1. Queries the ledger for the reserve's current balance.
2. Computes two independent deficits:
   - **Cyclical deficit** = `max(0, target_balance - current_balance)` -- the shortfall relative to the target.
   - **Lifetime deficit** = `max(0, lifetime_received_minimum - lifetime_minted)` -- the shortfall relative to the lifetime guarantee.
3. Takes the **larger** of the two deficits as the raw mint amount.
4. Successively caps the amount by:
   - `max_topup_per_rebalance`
   - Remaining `max_balance` room
   - Remaining `lifetime_received_maximum` budget
   - `max_mint_per_operation` (global policy)
   - Available rate-limit budget (tightest window)
5. If the final amount is greater than zero, mints tokens to the reserve's account on the ledger.
6. Updates the `lifetime_minted` counter and records a `MintEvent` for rate-limit tracking.

### Rate limits

Each reserve can optionally define caps on how many tokens may be minted within sliding time windows:

- Per hour
- Per day (24 h)
- Per week (7 d)
- Per month (30 d)
- Per year (365 d)

A validation rule enforces that **larger windows are strictly more restrictive** (lower effective rate) than smaller ones. For example, if the hourly limit is 100, the daily limit must be strictly less than 2 400 (100 \* 24).

For automatic rebalances, rate limits act as a soft cap: the mint amount is reduced to fit within the budget. For manual top-ups, rate limits are a hard check: the request is rejected outright if it would exceed any window.

Mint events older than 1 year are pruned automatically after every mint.

### Lifetime guarantees

- `lifetime_received_minimum`: a monotonically increasing guarantee. If the minter has minted less than this amount to the reserve over its entire history, additional tokens are minted to cover the gap. This value can only be increased via `update_reserve`, never decreased.
- `lifetime_received_maximum`: a hard ceiling. Once `lifetime_minted` reaches this value, no further minting is allowed for the reserve regardless of balance deficits.

### Auto-rebalance timer

The minter runs a recurring timer (1-hour interval) that automatically rebalances all reserves. On each tick, the timer calls `rebalance_all` internally — the same logic used by the `rebalance_all_reserves` admin endpoint.

Key properties:

- **Starts on init and post-upgrade.** IC clears all timers on canister upgrade, so `post_upgrade` re-arms the timer.
- **Serial execution.** Uses `set_timer_interval_serial` from `ic-cdk-timers`: if a rebalance cycle is still running when the next tick fires, the new invocation is skipped. No concurrent executions, no manual reentrancy guard.
- **Only affects reserves with `allow_auto_rebalance = true`.** Reserves configured with `allow_auto_rebalance = false` are skipped during the timer cycle.
- **Same safety constraints as manual calls.** All caps, rate limits, lifetime maximums, and global policy checks apply identically.

The timer is the steady-state heartbeat. Manual `rebalance_reserve` and `manual_topup_reserve` remain available for on-demand operations (emergency refill, initial funding, post-reconfiguration).

### Global policy

A canister-wide `GlobalPolicy` provides:

- `minting_enabled`: master switch; when `false`, all minting is rejected.
- `max_mint_per_operation`: optional cap applied to every single mint (rebalance or manual).

### Idempotency

Manual top-ups accept an optional `idempotency_key`. If a mint with the same key was already executed, the original result (block index, amount) is returned without re-minting.

## API reference

### Update methods (controller only)

| Method                                 | Description                                          |
| -------------------------------------- | ---------------------------------------------------- |
| `add_reserve(AddReserveArg)`           | Registers a new reserve. Returns the assigned id.    |
| `update_reserve(UpdateReserveArg)`     | Partially updates an existing reserve's config.      |
| `remove_reserve(id)`                   | Removes a reserve and returns its config.            |
| `set_global_policy(GlobalPolicy)`      | Replaces the canister-wide minting policy.           |
| `rebalance_reserve(id)`                | Triggers a rebalance for a single reserve.           |
| `rebalance_all_reserves()`             | Triggers a rebalance for every reserve sequentially. |
| `manual_topup_reserve(ManualTopupArg)` | Mints a specific amount to a reserve.                |

### Update methods (any authenticated caller)

| Method                             | Description                                               |
| ---------------------------------- | --------------------------------------------------------- |
| `preview_rebalance_reserve(id)`    | Dry-run: shows what a rebalance would do without minting. |
| `preview_rebalance_all_reserves()` | Dry-run for all reserves.                                 |

Preview methods are `update` (not `query`) because they make inter-canister calls to the ledger to fetch live balances.

### Query methods (any authenticated caller)

| Method                | Description                                                    |
| --------------------- | -------------------------------------------------------------- |
| `list_reserves()`     | Returns all reserves with their configs and lifetime counters. |
| `get_reserve(id)`     | Returns a single reserve's config and lifetime counters.       |
| `get_global_policy()` | Returns the current global minting policy.                     |
| `get_ledger_id()`     | Returns the principal of the connected ledger canister.        |

All query methods reject anonymous callers.

## Access control

- **Controller-only** methods use the `caller_is_controller` guard. The call is rejected before the function body runs if the caller is not a canister controller.
- **Authenticated** methods (queries and previews) use the `caller_is_not_anonymous` guard, blocking the anonymous principal.

## State persistence

The entire canister state (reserves, counters, policy, idempotency map) is Candid-encoded and written to IC stable memory during `pre_upgrade`. On `post_upgrade`, the state is restored. This ensures data survives canister code upgrades.

## Duplicate account guard

Each reserve must have a unique ICRC-1 account (principal + optional subaccount). The minter rejects `add_reserve` calls with an account that is already registered (`ReserveAccountAlreadyExists`). Accounts are immutable after creation — `update_reserve` does not expose an `account` field, so a reserve's account can never be changed to collide with another.

The `init.reserves.sh` script also validates uniqueness before making any `dfx` calls, catching duplicates early at the configuration level.

## Code structure

```
src/minter/src/
  lib.rs             Canister lifecycle (init, pre/post upgrade, timer start), Candid export
  model.rs           All data types (Candid API types + internal types)
  state.rs           State, thread-local storage, stable memory persistence
  guards.rs          Access control guards (caller_is_controller, caller_is_not_anonymous)
  api/
    admin/           Update methods (reserve CRUD, rebalance, manual topup, preview)
    query/           Query methods (list/get reserves, policy, ledger id)
  services/
    ledger.rs        ICRC-1 ledger client (balance queries, mint transfers)
    rebalance.rs     Core rebalance execution (single + all reserves)
    reserve.rs       Pure business logic (validation, rebalance computation, rate limits)
    timer.rs         Recurring auto-rebalance timer (1-hour interval)
```
