# docs/ai

Agent docs for `vici-points`. Start from [`AGENTS.md`](../../AGENTS.md); the
universal rules are in
[`officina` `standards/code.md`](https://github.com/AntonioVentilii/officina/blob/main/standards/code.md)
and are not repeated here.

| Page                                 | What it holds                                                         |
| ------------------------------------ | --------------------------------------------------------------------- |
| [`governance.md`](./governance.md)   | Protected and generated paths, what needs asking, the meta-update map |
| [`structure.md`](./structure.md)     | Folder taxonomy, where new code goes, dependency direction            |
| [`patterns.md`](./patterns.md)       | Patterns in use, with `path:line` examples, and named anti-patterns   |
| [`reusability.md`](./reusability.md) | Catalog of every shared function, constant, type and script           |
| [`testing.md`](./testing.md)         | Where tests live, how they are written, commands                      |
| [`pr-and-ci.md`](./pr-and-ci.md)     | PR title and body, local gates, what CI and release do                |
| [`workflows/`](./workflows/)         | Step-by-step recipes for the changes this repo sees most              |

Workflows:

- [`new-endpoint.md`](./workflows/new-endpoint.md): a new or changed Candid method on the minter.
- [`change-persisted-type.md`](./workflows/change-persisted-type.md): a field on a reserve, the global policy or `State`.
- [`canister-args.md`](./workflows/canister-args.md): ledger, index or minter init and upgrade arguments.
- [`dependency-bump.md`](./workflows/dependency-bump.md): a crate, the Rust toolchain, an npm package or a GitHub Action.

Product docs stay where they are and are linked, not restated:
[`README.md`](../../README.md) (architecture, standards, local deploy),
[`TLDR.md`](../../TLDR.md) and [`TOKENOMICS.md`](../../TOKENOMICS.md) (the XP
economy, reserves and emissions),
[`src/minter/README.md`](../../src/minter/README.md) (minter concepts, API
reference, access control).
