# Patterns

The patterns the minter really uses, each with an example to copy, and the
anti-patterns already in the code, named so nobody copies them. Paths are
relative to `src/minter/src/` unless they start with `scripts/` or a file at
the root.

## Endpoints

- **`*_impl` plus a thin endpoint.** The logic is a private function returning
  `Result<T, MinterError>`, so it can use `?`; the `#[update]` / `#[query]`
  function only converts it with `.into()`: `api/admin/api.rs:26` and
  `api/admin/api.rs:75-78`.
- **One named result enum per fallible endpoint**, `Ok(T)` /
  `Err(MinterError)`, with a `From<Result<T, MinterError>>` impl next to it:
  `api/admin/results.rs:47-63`. Endpoints that cannot fail return the payload
  directly (`list_reserves`, `get_global_policy`, `get_ledger_id`,
  `api/query/api.rs:14`, `:51`, `:58`).
  Large payloads go in a `Box` (`api/query/results.rs:21`).
  `scripts/lint.did.sh` fails the build if `Result` or `Result_N` shows up in
  the `.did`.
- **A guard on every endpoint**, in the attribute:
  `#[update(guard = "caller_is_controller")]` for anything that writes or
  mints, `caller_is_not_anonymous` for reads and previews (`guards.rs:5`,
  `guards.rs:16`). Authorization failures are guard rejections, not
  `MinterError` values (`model.rs:221-225`).
- **Previews are `#[update]`**, not queries, because they call the ledger:
  `api/admin/api.rs:527-577`.
- **Partial updates use `Option<Option<T>>`**: omit keeps, `null` clears, a
  value sets. The struct carries `#[expect(clippy::option_option)]` with the
  reason: `api/admin/params.rs:13`.

## Errors

- Every failure a caller can act on is a `MinterError` variant with the data
  needed to act (`model.rs:226`). Add a variant rather than a free-text
  `InvalidConfig` when callers need to branch on it.
- Ledger call failures are mapped once, in the client, to
  `MinterError::LedgerError` with the context in the message:
  `services/ledger.rs:17-27`.
- `TransferError::Duplicate` is a success that returns the original block:
  `services/ledger.rs:66-72`.
- After a successful mint, a missing reserve is
  `InternalInvariantViolated`, never a panic: `services/rebalance.rs:99-106`.

## State

- One `State`, in one `thread_local!` `RefCell`, reached only with
  `STATE.with_borrow` / `with_borrow_mut`: `state.rs:45-48`.
- **No `await` inside a borrow.** Read a snapshot (clone what the flow needs),
  drop the borrow, await, then commit in a new borrow. The section header
  `api/admin/api.rs:261` says it; `services/rebalance.rs:27-43` and `:97-114`
  show it.

### Stable state

- `pre_upgrade` writes the Candid-encoded `State` with a 4-byte length prefix;
  `post_upgrade` restores it and traps if it cannot: `state.rs:58`,
  `state.rs:90`, `lib.rs:55`.
- So every type reachable from `State` (`ReserveRecord`, `ReserveConfig`,
  `RateLimits`, `MintEvent`, `GlobalPolicy`, `IdempotencyRecord`) is a stored
  format. A new field is `Option<T>` (Candid decodes a missing `opt` field as
  `null`); a removed or renamed field or variant needs a migration. Recipe:
  [change-persisted-type](./workflows/change-persisted-type.md).
- Timers do not survive an upgrade: `init` and `post_upgrade` both call
  `start_rebalance_timer` (`services/timer.rs:18`).

## Mint

Every mint, automatic or manual, follows the same five steps:

1. **Lock and snapshot** in one `with_borrow_mut`: insert the id into
   `reserve_mint_inflight`, fail with `ReserveOperationInProgress` if it was
   there, clone config, counters, events and policy
   (`services/rebalance.rs:27-43`).
2. **Check** with the pure functions of `services/reserve.rs`
   (`compute_rebalance`, `available_mint_budget`, `check_rate_limits`).
3. **Mint** through `ledger::mint_to` with a memo naming the operation
   (`rebalance:{id}`, `topup:{id}:{key}`).
4. **Commit** in a new borrow: release the lock, add to `lifetime_minted`,
   push a `MintEvent`, `prune_old_events` (`services/rebalance.rs:97-114`).
5. **Release the lock on every early exit**: `release_mint_lock`
   (`services/rebalance.rs:10`) or `rollback_manual_mint_begin`
   (`api/admin/api.rs:264`). A lock left behind blocks the reserve until a
   controller calls `release_stuck_mint_state`.

Manual top-ups add **idempotency**: a key is reserved as `Pending` in the same
borrow as the lock, replayed from `Completed`, and rejected while pending
(`api/admin/api.rs:382-426`).

## Pure rules

- Validation and amount computation take plain values and return
  `Result<_, MinterError>` or a value: `services/reserve.rs:58`
  (`validate_rate_limits`), `:196` (`available_mint_budget`), `:303`
  (`compute_rebalance`). `now_ns` is a parameter, never `time()` inside.
- Amounts are `candid::Nat` end to end, compared and subtracted only after a
  `>` check, so they never underflow (`services/reserve.rs:217-221`).
- Larger rate-limit windows must be strictly more restrictive; compared by
  cross-multiplying, never dividing `Nat` (`services/reserve.rs:42-88`).

## Lints

- A lint that must stay off at one site gets `#[expect(clippy::lint)]` plus a
  comment saying why (`model.rs:37`, `api/admin/api.rs:311`, `state.rs:68`).
  `#[allow]` is denied by `allow_attributes`.
- Imports go through `use`, never absolute paths (`absolute_paths`), and from
  `core` before `std` (`std_instead_of_core`).
- `ic_cdk::println!`, never `std::println!` (`clippy.toml` `disallowed-macros`).
- A function over a size limit today carries
  `#[expect(clippy::too_many_lines, reason = "baseline, ...")]` and the like,
  with its size pinned in `scripts/lint.fn-size.sh`
  (`services/reserve.rs:298-302`). Those pins only shrink; never add one to
  new code. See [the quality gate](./pr-and-ci.md#quality-gate).

## Anti-patterns in the code today

Do not copy these. Each is a follow-up of its own; fix one only in a PR whose
purpose it is (rule 2, scoped).

- **A flow in the API layer.** The manual top-up flow, with its own
  validation, lock, ledger call and commit, lives in `api/admin/api.rs:327-507`
  while the rebalance flow lives in `services/rebalance.rs`. New flows go in
  `services/`.
- **Copy-paste around the mint.** The lock-and-snapshot tuple
  (`services/rebalance.rs:27-43`, `api/admin/api.rs:382-426`; the preview at
  `api/admin/api.rs:534-546` takes the same snapshot without the lock), the
  release-on-error `match` (`services/rebalance.rs:45-59`, `:87-95`,
  `api/admin/api.rs:438-468`) and the post-mint commit
  (`services/rebalance.rs:97-114`, `api/admin/api.rs:470-498`) are written
  twice or more. Extend them into one helper before adding a third caller.
- **The rate-limit windows written three times.** `validate_rate_limits`,
  `available_mint_budget` and `check_rate_limits` each list the five windows,
  and `HOURS_PER_*` and `NANOS_PER_*` are two parallel constant sets
  (`services/reserve.rs:12-36`, `:59-65`, `:204-210`, `:244-250`). One table,
  derived units (rule 5, one value, one place).
- **Repeated result literals.** `compute_rebalance` builds the same zero
  `Skipped` result five times (`services/reserve.rs:316-419`); `list_reserves`
  and `get_reserve_impl` build `ReserveInfo` twice (`api/query/api.rs:18-22`,
  `:31-35`).
- **Twin structs.** `AddReserveArg` and `ReserveConfig` hold the same fields,
  copied one by one in `api/admin/api.rs:44-58`.
- **`expect` outside tests and a magic page size.** `state.rs:67` and `:79`
  call `expect` (pinned by the quality gate), and the 64 KiB page size is
  the literal `65536` twice (`state.rs:72`, `:106`).
- **Variants nothing returns.** `NotAuthorized`, `AutoRebalanceNotAllowed` and
  `IdempotencyKeyAlreadyUsed` are kept only for the Candid interface. Do not
  add more reserved variants.
- **Scripts.** `build.ledger.args.sh` and `build.index.args.sh` call `ECHO`
  without defining it (only `build.minter.args.sh` does), which works only on
  a case-insensitive file system; the logo paths point at an `assets/` folder
  that does not exist; the ledger-suite release tag is written in five places
  (`dfx.json`, `download.ledger.sh`) and `DECIMALS=4` in two
  (`build.ledger.args.sh`, `init.reserves.sh`).
