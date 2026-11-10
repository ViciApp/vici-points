# Workflow: add or change a minter endpoint

For a new Candid method on the minter, or a change to the arguments or result
of an existing one. Removing or renaming a method, field or `MinterError`
variant is an ask first: vici-app and vici-maker call the minter.

1. **Pick the area.** Writes and anything that mints go in
   `src/minter/src/api/admin/`; reads in `api/query/`. A read that calls the
   ledger is still an `#[update]` (like the previews).
2. **Types.** The argument goes in that area's `params.rs`, unless it is a
   domain type already in `model.rs` (check
   [reusability.md](../reusability.md)). The success payload goes in
   `results.rs`.
3. **Result enum.** Add `<Method>Result { Ok(T), Err(MinterError) }` with its
   `From<Result<T, MinterError>>` impl, next to the others in `results.rs`.
   Box a large `Ok` payload.
4. **Logic.** Write `fn <method>_impl(...) -> Result<T, MinterError>`. If it
   mints, it is a flow: put it in `services/` and follow
   [the mint pattern](../patterns.md#mint). Rules on plain values go in
   `services/reserve.rs` with unit tests.
5. **Endpoint.** A thin `#[update(guard = "caller_is_controller")]` or
   `#[query(guard = "caller_is_not_anonymous")]` function with a `///` doc
   comment that returns `<method>_impl(arg).into()`.
6. **Errors.** Reuse a `MinterError` variant if one fits; a new variant goes
   at the end of the enum, with a doc comment.
7. **Candid.** Run `npm run did` and commit `src/minter/minter.did`. The diff
   must show only your method and types; `lint.did.sh` rejects `Result` names.
8. **Docs.** Add the method to the API reference in
   `src/minter/README.md`, and new shared types to
   [reusability.md](../reusability.md).
9. **Gates.** `npm run format`, `npm run lint`, `cargo test --locked -p minter`.
   PR title `feat(minter): ...`.
