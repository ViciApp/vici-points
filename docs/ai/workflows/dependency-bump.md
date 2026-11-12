# Workflow: bump a dependency

Most PRs here are Dependabot bumps (`.github/dependabot.yml`: weekly, 30-day
cooldown, prefixes `chore(cargo-deps)`, `chore(npm-deps-dev)`, `chore(rust)`,
`chore(github-actions)`). Adding a new dependency is an ask first (rule 12).

## Crates

1. Versions live only in the root `Cargo.toml` `[workspace.dependencies]`;
   `src/minter/Cargo.toml` uses `{ workspace = true }`
   (`scripts/lint.cargo-workspace-dependencies.sh` enforces it).
2. For a security advisory, update only the affected crate:
   `cargo update -p <crate> --precise <version>`, no blanket `cargo update`.
   Say whether the crate reaches the canister wasm
   (`cargo tree -i <crate> --target wasm32-unknown-unknown`) or only dev
   tooling such as `pocket-ic`;
   [`#35`](https://github.com/ViciApp/vici-points/pull/35) is the model.
3. `ic-cdk` and `ic-cdk-timers` move together; a major `ic-cdk` or `candid`
   bump can change the `.did`: run `npm run did` and check the diff is empty
   or explained.

## Rust toolchain

`rust-toolchain.toml` pins the build and clippy toolchain; `dev-tools.json`
pins the nightly used by `cargo fmt`. A bump can add clippy lints: fix them in
the same PR (they are the bump's purpose), never relax `[workspace.lints]`.

## npm and GitHub Actions

- npm packages are dev tooling only (prettier and plugins, bindgen); run
  `npm ci` and `npm run format` and check nothing reformats unexpectedly.
- Actions are pinned by commit sha with the version in a comment; keep both in
  step, and run `scripts/lint.github.sh` (zizmor).

## Gates

```bash
cargo build --locked --target wasm32-unknown-unknown --release -p minter
cargo test --locked -p minter
npm run lint
```
