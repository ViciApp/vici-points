# PRs and CI

## Title

Conventional commits, as in the history: `type(scope): description`,
lowercase, imperative, no trailing period.

- Types seen: `feat`, `fix`, `chore`, `docs`, `ci`.
- Scopes seen: `deps` for security bumps, and Dependabot's `cargo-deps`,
  `cargo-deps-dev`, `npm-deps-dev`, `rust`, `github-actions`. Use `minter` for
  canister code, `scripts` for scripts, `ai` for agent docs.
- Examples: `chore: point repo cross-links at the ViciApp org`,
  `fix(deps): bump rustls-webpki, quinn-proto and rand past security advisories`.

## Body

Fill the sections of
[`.github/pull_request_template.md`](../../.github/pull_request_template.md):
Summary, Risk Classification, Checklist, Evidence of Verification. Leave out
"Linked Artifacts": it points at local files nobody else can open.

- Any change under `src/minter/src/` is at least Medium risk; a change to
  minting, locks, idempotency, caps or persisted types is High.
- Evidence lists the commands run and their result (`cargo test`: 42 passed),
  and for an API change the `minter.did` diff.
- Call out any protected path touched (see [governance.md](./governance.md)).
- No AI attribution lines. Do not start a body line with `Test:` or `Fix:`.

## Local gates

Run before opening a PR:

```bash
npm ci
npm run format                  # then commit what it changed
npm run lint                    # prettier check, workspace deps, did, zizmor, clippy, shellcheck
cargo test --locked -p minter   # not in CI
npm run did                     # when an endpoint or a Candid type changed; commit minter.did
npm run build                   # when Cargo.toml, Cargo.lock or the build scripts changed
```

`npm run format` needs the nightly in `dev-tools.json` (rustfmt runs with
unstable options), cargo-sort and shfmt; `npm run lint` needs yq, zizmor and
shellcheck. `scripts/setup <tool>` installs each at its pinned version.

## CI

- `.github/workflows/checks.yml` on every PR, push to `main`, tag and merge
  group:
  - `format`: runs `npm run format` and fails if it changed any file.
  - `lint`: runs `npm run lint`.
  - `checks-pass`: fails unless both jobs succeeded.
- Not in CI: `cargo test`, `npm run build`, and a check that `minter.did`
  matches the code. Those are on you.
- `.github/workflows/release.yml` on a `v*` tag or a published release: builds
  the minter wasm, downloads ledger and index, and attaches the wasm and `.did`
  files to the GitHub release that vici-app pins.
- Nothing deploys from CI. An agent opens the PR and stops there.
