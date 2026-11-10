# Workflow: change a persisted type

For a field on a reserve (`ReserveConfig`, `ReserveRecord`), on `RateLimits`,
`MintEvent`, `GlobalPolicy`, `IdempotencyRecord`, or on `State` itself. These
types are Candid-encoded into stable memory by `pre_upgrade` and decoded by
`post_upgrade` (`src/minter/src/state.rs:58`, `:90`); if the old bytes do not
decode into the new type, `post_upgrade` traps and the upgrade fails. This is a
protected change: ask first.

1. **Add, do not change.** A new field is `Option<T>`: Candid decodes a missing
   `opt` field as `null`, so state written by the old wasm still loads. A
   non-optional field, a renamed field, a changed type or a removed enum
   variant that may be stored needs a migration; stop and propose one.
2. **Default.** Decide what `None` means and handle it where the field is
   read, so a reserve created before the change behaves as before.
3. **Arguments.** If callers set the field, add it to `AddReserveArg` (and
   copy it in `add_reserve_impl`, `api/admin/api.rs:44-58`) and to
   `UpdateReserveArg` as `Option<T>`, or `Option<Option<T>>` when it can be
   cleared, applied in `update_reserve_impl`.
4. **Validation.** Any invariant involving the field goes in
   `validate_balance_invariants` or a sibling in `services/reserve.rs`, with a
   unit test for the accept and the reject case.
5. **Candid and docs.** `npm run did`; describe the field in
   `src/minter/README.md`; add it to [reusability.md](../reusability.md) if it
   is a new type.
6. **Prove the upgrade.** Deploy the current `main` wasm to a local replica
   (`npm run deploy`), create a reserve, then deploy your branch and check
   `list_reserves` still returns it. Put the commands and output in the PR's
   Evidence section.
7. **Gates.** `npm run format`, `npm run lint`, `cargo test --locked -p minter`.
   Mark the PR High risk.
