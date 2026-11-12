# PRs and CI

## Scope

One purpose per PR, each opened from `main` and mergeable in any order; stack
only when one PR needs the other's code. These are rules
[2 (scoped)](https://github.com/AntonioVentilii/officina/blob/main/standards/code.md#2-scoped)
and
[3 (parallel, not stacked)](https://github.com/AntonioVentilii/officina/blob/main/standards/code.md#3-parallel-not-stacked)
of the Officina code standard.

## Title

Conventional commits, as in the history: `type(scope): description`,
lowercase, imperative, no trailing period. The scope is required: the
`pr-checks` title check rejects a title without one.

- Types seen: `feat`, `fix`, `chore`, `docs`, `ci`.
- Scopes seen: `deps` for security bumps, and Dependabot's `cargo-deps`,
  `cargo-deps-dev`, `npm-deps-dev`, `rust`, `github-actions`. Use `minter` for
  canister code, `scripts` for scripts, `ai` for agent docs.
- Examples: `chore(ai): point repo cross-links at the ViciApp org`,
  `fix(deps): bump rustls-webpki, quinn-proto and rand past security advisories`.

## Body

Fill the sections of
[`.github/pull_request_template.md`](../../.github/pull_request_template.md):
Summary, Risk Classification, Checklist, Evidence of Verification.

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

## Quality gate

`npm run lint` fails on size and smell, not only on style. The limits for new
code live in one place each; read the number there:

| Check                                         | Limit lives in                                                               |
| --------------------------------------------- | ---------------------------------------------------------------------------- |
| Lines per function                            | `clippy.toml` `too-many-lines-threshold`                                     |
| Cognitive complexity per function             | `clippy.toml` `cognitive-complexity-threshold`                               |
| Arguments per function                        | `clippy.toml` `too-many-arguments-threshold`                                 |
| Block nesting                                 | `clippy.toml` `excessive-nesting-threshold`                                  |
| No `unwrap`, `expect`, `panic!` outside tests | `Cargo.toml` `[workspace.lints.clippy]`, `allow-*-in-tests` in `clippy.toml` |
| Lines per `.rs` file                          | `MAX_LINES` in `scripts/lint.file-size.sh`                                   |
| Size of each pinned function                  | `BASELINE` in `scripts/lint.fn-size.sh`                                      |

Code over a limit when the gate was added is pinned, and the pins only go
down:

- Functions carry `#[expect(clippy::<lint>, reason = "baseline, ...")]`, and
  their measured value is listed in
  `BASELINE` in `scripts/lint.fn-size.sh`. That script measures each pinned
  function with clippy and fails if the value grew (split the function) or
  shrank (lower the pin). When a function gets under the limit, clippy no
  longer fires, the `expect` fails the build: delete it and its `BASELINE`
  rows in the same PR.
- Files over `MAX_LINES` are listed in `BASELINE` in
  `scripts/lint.file-size.sh` with their size rounded up to the next 50. The
  script fails if a file grows past its cap, and also if a cap is 50 or more
  above the file (lower it) or the file is back under `MAX_LINES` (remove it).
- Never raise a limit, add a new `expect` for these lints, or add a baseline
  entry to get a PR green: split the function or the file (rule 11, respect
  CI).

## CI

- `.github/workflows/checks.yml` on every PR, push to `main`, tag and merge
  group:
  - `format`: runs `npm run format` and fails if it changed any file.
  - `lint`: runs `npm run lint`.
  - `checks-pass`: fails unless both jobs succeeded.
- `.github/workflows/pr-checks.yml` on every PR and merge group: calls the
  org-wide pull request rules in `ViciApp/.github`
  (`.github/workflows/pull-request.yml`, the one copy every ViciApp repository
  runs): a Conventional Commit title with a scope, and no empty change, checked
  again for each merge queue entry. The org `baseline` ruleset requires its
  `pr-checks / pass`.
- `main` merges through the merge queue (this repo's `merge-queue`
  ruleset; GitHub has no merge queue rule at org level), so the
  `main` ruleset no longer asks for branches to be up to date. Queuing a PR is
  merging it: only the owner does it.
- Not in CI: `cargo test`, `npm run build`, and a check that `minter.did`
  matches the code. Those are on you.
- `.github/workflows/release.yml` on a `v*` tag or a published release: builds
  the minter wasm, downloads ledger and index, and attaches the wasm and `.did`
  files to the GitHub release that vici-app pins.
- Nothing deploys from CI. An agent opens the PR and stops there.
