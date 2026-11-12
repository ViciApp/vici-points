# Testing

## Where tests live

- **Unit tests** sit in a `#[cfg(test)] mod tests` at the bottom of the file
  they test. Today that is `src/minter/src/services/reserve.rs:425`: 42 tests
  over validation, rate limits, pruning and `compute_rebalance`.
- **No integration tests yet.** `pocket-ic` is a dev-dependency of the minter,
  but no test uses it. Flows that lock, call the ledger and commit
  (`services/rebalance.rs`, the manual top-up in `api/admin/api.rs`) have no
  automated test today.

## How they are written

Copy the style of `services/reserve.rs`:

- One behaviour per `#[test]`, named for the behaviour in snake case with no
  `test_` prefix (`redundant_test_prefix` is denied):
  `caps_at_max_topup_per_rebalance`, `rejects_empty_label`.
- Fixtures are small builder functions in the test module (`sample_config`,
  `sample_policy`, `services/reserve.rs:439-465`); a test mutates the field it
  is about.
- Time is a parameter: pass `now_ns` built from the window constants
  (`NANOS_PER_HOUR * 2`), never call `time()`.
- Assert the outcome a caller sees: the `RebalanceAction`, the `mint_amount`,
  or `matches!(r, Err(MinterError::RateLimitExceeded { .. }))`.
- `unwrap()` is fine in tests; keep it out of production code (rule 6).

## Where a new test goes

| What you changed                                  | Test it                                                                                                                                |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| A rule in `services/reserve.rs`                   | a unit test in its `mod tests`                                                                                                         |
| New logic in a flow                               | extract the decision into a pure function in `reserve.rs` and unit-test that                                                           |
| Locking, idempotency, upgrade or ledger behaviour | a PocketIC integration test under `src/minter/tests/` (ask first: it is the first one, and needs the ledger wasm from `npm run build`) |

## Commands

```bash
cargo test --locked -p minter     # all unit tests, native target
npm run lint                      # clippy also checks the test code (--tests)
```

CI does not run `cargo test` (see [pr-and-ci.md](./pr-and-ci.md)): run it
before every PR that touches `src/minter/`.
