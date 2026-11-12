# Reusability catalog

Everything shared in the repo, one line each. Search here before writing a
helper, a constant, a type or a script (rule 4, reuse before you write). A PR
that adds a shared piece adds its row. Paths are relative to
`src/minter/src/` unless they start with `scripts/`.

## Guards (`guards.rs`)

| Name                      | What it does                                      |
| ------------------------- | ------------------------------------------------- |
| `caller_is_controller`    | Rejects callers that are not canister controllers |
| `caller_is_not_anonymous` | Rejects the anonymous principal                   |

## State (`state.rs`)

| Name                | What it does                                                             |
| ------------------- | ------------------------------------------------------------------------ |
| `State`             | All canister state: ledger id, reserves, policy, idempotency, mint locks |
| `STATE`             | The thread-local `RefCell<State>`; `with_borrow` / `with_borrow_mut`     |
| `save_state`        | Candid-encodes `State` into stable memory (pre_upgrade)                  |
| `try_restore_state` | Decodes `State` from stable memory, `false` if empty or invalid          |

## Pure rules (`services/reserve.rs`)

| Name                       | What it does                                                              |
| -------------------------- | ------------------------------------------------------------------------- |
| `validate_add_reserve_arg` | Balance invariants plus rate-limit progressivity for a new reserve        |
| `validate_reserve_config`  | Same checks on a full config after a partial update                       |
| `validate_rate_limits`     | Larger windows must be strictly more restrictive                          |
| `available_mint_budget`    | Minimum remaining budget across configured windows, `None` without limits |
| `check_rate_limits`        | Hard error if minting an amount would breach a window                     |
| `prune_old_events`         | Drops `MintEvent`s older than the largest window (one year)               |
| `compute_rebalance`        | Mint amount from cyclical and lifetime deficits, after every cap          |
| `WINDOWS`                  | Every rate-limit window: name, length in hours, its cap (private)         |
| `NANOS_PER_HOUR`           | The unit window lengths are counted in (private)                          |

## Flows and clients (`services/`)

| Name                           | What it does                                                                                  |
| ------------------------------ | --------------------------------------------------------------------------------------------- |
| `rebalance::rebalance_one`     | Lock, read balance, compute, rate-cap, mint, commit for one reserve                           |
| `rebalance::rebalance_all`     | `rebalance_one` for every reserve, sequentially, failures isolated                            |
| `ledger::get_balance`          | `icrc1_balance_of` on the ledger, errors mapped to `LedgerError`                              |
| `ledger::mint_to`              | `icrc1_transfer` from the minting account, memo truncated to 32 bytes, `Duplicate` is success |
| `timer::start_rebalance_timer` | Arms the serial hourly rebalance (`REBALANCE_INTERVAL`)                                       |

Private helpers worth reusing inside their file: `release_mint_lock`
(`services/rebalance.rs:10`), `rollback_manual_mint_begin` and
`validate_manual_topup_inputs` (`api/admin/api.rs:264`, `:327`).

## Types (`model.rs`)

| Name                                    | What it is                                                              |
| --------------------------------------- | ----------------------------------------------------------------------- |
| `Arg`, `InitArg`                        | Install and upgrade argument (`Init { ledger_id }` or `Upgrade`)        |
| `ReserveConfig`                         | A reserve's account, thresholds, caps, rate limits, flags, label        |
| `ReserveRecord`                         | `ReserveConfig` plus `lifetime_minted` and `mint_events`                |
| `AddReserveArg`                         | Argument of `add_reserve` (same fields as `ReserveConfig`)              |
| `RateLimits`                            | Optional caps per hour, day, week, month, year                          |
| `MintEvent`                             | One mint: `timestamp_ns`, `amount`                                      |
| `GlobalPolicy`                          | `minting_enabled`, `max_mint_per_operation`; default enabled, no cap    |
| `IdempotencyRecord`, `IdempotencyEntry` | Manual top-up key state: `Pending { reserve_id }` or `Completed(entry)` |
| `MinterError`                           | Every error a caller can receive                                        |
| `RebalanceAction`                       | `Minted`, `AlreadyFunded`, `Skipped { reason }`                         |
| `RebalanceResponse`                     | Result of one rebalance, shared by api and services                     |
| `ComputedRebalance`                     | Internal output of `compute_rebalance` (not in Candid)                  |

Endpoint-only types: `UpdateReserveArg`, `ReleaseStuckMintArg`,
`ManualTopupArg` (`api/admin/params.rs`); `ManualTopupResponse`,
`PreviewRebalanceResponse` and the `*Result` enums (`api/admin/results.rs`);
`ReserveInfo` (built from a stored record with `ReserveInfo::new`),
`GetReserveResult` (`api/query/results.rs`).

## Scripts (`scripts/`)

| Script                                           | What it does                                                                              |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| `build.sh`                                       | Release wasm of the minter, gzipped, plus `download.ledger.sh`                            |
| `did.sh`                                         | Builds the wasm and extracts `minter.did` with candid-extractor                           |
| `build.minter.args.sh`                           | Writes the minter `Arg` (Init or Upgrade) for `DFX_NETWORK`                               |
| `build.ledger.args.sh`                           | Writes the ledger init or upgrade argument (name, symbol, decimals, fee, minting account) |
| `build.index.args.sh`                            | Writes the index-ng init or upgrade argument                                              |
| `check.canister.installed.sh`                    | Exit 0 if a canister has a module; picks Init vs Upgrade                                  |
| `download.ledger.sh`, `download-immutable.sh`    | Fetch ledger and index wasm and did into `target/ic/`, skipping repeats                   |
| `init.reserves.sh`                               | Registers the six gameplay reserves (`DRY_RUN=1` prints the calls)                        |
| `init.reserves.config.example.sh`                | Template for the gitignored reserve principals                                            |
| `lint.sh`                                        | Runs every `lint.*.sh`                                                                    |
| `lint.rust.sh`                                   | Clippy on wasm32, tests, examples and benches                                             |
| `lint.file-size.sh`                              | Line cap per `.rs` file, with a shrink-only baseline                                      |
| `lint.fn-size.sh`                                | Measures each function pinned with `#[expect]` and fails if it grew or shrank             |
| `lint.did.sh`                                    | No `Result` or `Result_N` in a `.did`                                                     |
| `lint.cargo-workspace-dependencies.sh`           | Every crate dependency comes from the workspace                                           |
| `lint.github.sh`                                 | zizmor, pedantic, on `.github`                                                            |
| `lint.sh.sh`, `format.sh.sh`                     | shellcheck and shfmt on every shell script                                                |
| `format.sh`, `format.rust.sh`, `format.cargo.sh` | cargo fmt (nightly from `dev-tools.json`), cargo sort, shfmt                              |
| `setup`, `setup-rust`, `setup-cargo-binstall`    | Install a tool pinned in `dev-tools.json`                                                 |
