# Structure

One Cargo workspace with one crate, `src/minter` (a `cdylib` canister). The
ledger and index canisters are declared in `dfx.json` as `custom` canisters
pointing at a DFINITY release; there is no code for them here, only their
init arguments in `scripts/`.

## Folder taxonomy

```
Cargo.toml                 workspace: members, shared dependency versions, lints
clippy.toml rustfmt.toml   lint and format configuration
rust-toolchain.toml        toolchain for build and clippy (fmt uses dev-tools.json nightly)
dfx.json                   minter, ledger, index; init_arg_file + pre_install scripts
canister_ids.json          mainnet ids
dev-tools.json             pinned CLI tools, read by scripts/setup
package.json               npm scripts (the command surface) and prettier
src/minter/
  minter.did               generated Candid interface
  README.md                minter concepts, API reference, code tree
  src/
    lib.rs                 init, pre_upgrade, post_upgrade, export_candid!
    guards.rs              caller_is_controller, caller_is_not_anonymous
    model.rs               domain types, Candid arguments, MinterError, internal ComputedRebalance
    state.rs               State, the STATE thread-local, save_state / try_restore_state
    api/
      admin/               update endpoints
        api.rs             #[update] functions and their *_impl
        params.rs          argument types that are not domain types
        results.rs         response structs and one *Result enum per endpoint
      query/               query endpoints, same split (api.rs, results.rs)
    services/
      reserve.rs           pure rules: reserve validation, compute_rebalance (+ unit tests)
      rate_windows.rs      pure rules: the rate-limit windows and their validation (+ unit tests)
      rate_limit.rs        pure rules: mint events against the windows: budget, check, prune (+ unit tests)
      rebalance.rs         rebalance_one / rebalance_all: lock, ledger, commit
      ledger.rs            ICRC-1 client: get_balance, mint_to
      timer.rs             the hourly rebalance timer
scripts/                   one script per job; see reusability.md
.github/workflows/         checks.yml (format, lint), pr-checks.yml (org-wide PR rules),
                           release.yml (tag -> assets)
.github/actions/           prepare (node, rust, tools), needs_success, add-and-commit
```

## Where new code goes

| New thing                                              | Goes in                                                                                                                      |
| ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| A rule or computation on plain values                  | the pure module that owns it (`services/reserve.rs`, `rate_windows.rs`, `rate_limit.rs`), with unit tests in its `mod tests` |
| A flow that reads state, calls the ledger, commits     | `services/` (one file per flow, like `rebalance.rs`)                                                                         |
| A call to another canister                             | a client function in `services/` (`ledger.rs` for the ledger)                                                                |
| A controller-only method                               | `api/admin/api.rs`, arguments in `params.rs`, result in `results.rs`                                                         |
| A read-only method                                     | `api/query/api.rs` and `results.rs`                                                                                          |
| A type stored in `State` or shared by api and services | `model.rs`                                                                                                                   |
| A type only one endpoint returns                       | that area's `results.rs`                                                                                                     |
| A new caller check                                     | `guards.rs`                                                                                                                  |
| A periodic job                                         | `services/timer.rs`, armed from `init` and `post_upgrade`                                                                    |
| A build, lint or deploy step                           | a new `scripts/<verb>.<object>.sh`, wired from `package.json` or `scripts/lint.sh`                                           |

A new module, a new crate in the workspace or a new canister in `dfx.json` is
an ask first (rule 7, respect the structure).

## Dependency direction

```
lib.rs -> api/* -> services/* -> model, state
                \-> guards
services/reserve.rs, rate_limit.rs -> rate_windows.rs, model (pure)
services/rate_windows.rs           -> model only (pure)
services/ledger.rs                 -> model only (+ ic_cdk call)
```

- `api/**` may use `services`, `state`, `guards`, `model`; `services` never
  import from `api`.
- The pure modules (`services/reserve.rs`, `rate_windows.rs`, `rate_limit.rs`)
  import only `model`, `rate_windows` and `candid::Nat`: no `STATE`, no
  `time()`, no `ic_cdk`.
- `model.rs` imports nothing from the crate.
- `api/admin/params.rs` re-exports `AddReserveArg` from `model` so the
  endpoint signatures read from one place.
