# Vici XP

Vici XP (**VXP**) is the gameplay point system for the [Vici](https://vici.com) prediction platform, built on the [Internet Computer](https://internetcomputer.org/) using the **ICRC** family of standards. This repository contains the full on-chain infrastructure: ledger, index, and minter canisters.

XP is the **non-monetary, high-volume gameplay layer** of Vici's dual-token model. Every user earns XP instantly through participation — predicting, streaking, climbing leaderboards. It powers the engagement loop with zero friction.

For the scarce reward/coordination token, see [vici-icrc](https://github.com/AntoninoVentworthy/vici-icrc) (VICI).

## Dual-token model

| Token          | Symbol | Role                                   | Repo                                                         |
| -------------- | ------ | -------------------------------------- | ------------------------------------------------------------ |
| **Vici XP**    | VXP    | Gameplay / onboarding — everyone earns | **this repo**                                                |
| **VICI Token** | VICI   | Reward / coordination — top users earn | [vici-icrc](https://github.com/AntoninoVentworthy/vici-icrc) |

## Standards implemented

| Standard                                                                         | What it defines                                                            | Specification       |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------- |
| [ICRC-1](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-1/README.md) | Fungible token interface -- transfers, balances, metadata, minting account | Core token standard |
| [ICRC-2](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-2/README.md) | Approve & transfer-from (allowance model)                                  | Extends ICRC-1      |
| [ICRC-3](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-3/README.md) | Transaction log (block archive)                                            | On-chain history    |

The ledger and index canisters are the official DFINITY implementations from the [IC repository](https://github.com/dfinity/ic), deployed as pre-built WASM modules.

## Architecture

```
                          ┌────────────┐
                          │   Minter   │
                          │  (Rust)    │
                          └─────┬──────┘
                                │ icrc1_transfer (mint)
                                │ icrc1_balance_of
                                ▼
┌──────────┐           ┌────────────────┐
│  Users / │  ICRC-1   │                │
│  dApps   │◄─────────►│     Ledger     │
│          │  ICRC-2   │                │
└──────────┘           └───────┬────────┘
                               │ block feed
                               ▼
                        ┌──────────────┐
                        │              │
                        │    Index     │
                        │              │
                        └──────────────┘
```

### Ledger

The **ledger canister** is the source of truth for VXP balances and transactions. It implements ICRC-1, ICRC-2, and ICRC-3.

Key properties:

- Maintains an append-only transaction log.
- Every transfer, mint, burn, and approval is recorded as a block.
- Has a designated **minting account** (the minter canister's principal). Only transfers from this account create new tokens.
- Exposes standard `icrc1_transfer`, `icrc1_balance_of`, `icrc2_approve`, `icrc2_transfer_from`, and metadata methods.
- Deployed from the official DFINITY WASM release; no custom code.

### Index

The **index canister** continuously syncs blocks from the ledger and provides efficient lookup capabilities that the ledger itself does not offer:

- List transactions for a given account.
- Query an account's balance (redundantly, for faster reads).
- Track all accounts that have ever interacted with the token.

It is deployed from the official DFINITY `icrc1-index-ng` WASM release and depends solely on the ledger canister.

### Minter

The **minter canister** is a custom Rust canister that acts as the ledger's minting account. It manages a set of _reserve accounts_ — trusted system accounts whose XP balance the minter keeps topped up via configurable rebalancing rules.

The minter never mints tokens to arbitrary users. All minting flows through the reserve system with multiple layers of safety controls: global policy flags, per-reserve balance targets, lifetime minimum/maximum guarantees, per-operation caps, and sliding-window rate limits.

A recurring **auto-rebalance timer** (1-hour interval) runs inside the canister, automatically refilling reserves when their balance drops below the configured target. This makes the minter self-operating — no external cron or scheduler required.

The **app backend** holds funded reserve wallets and distributes XP to individual users based on gameplay logic. The minter does not know about individual users.

See the [minter README](src/minter/README.md) for a detailed description of its logic, API, and configuration.

## Tokenomics

XP uses a gameplay-focused reserve structure with **6 reserves** (forecast, onboarding, streaks, leaderboard, campaign, buffer) — no corporate allocations. The full design is described in the [Tokenomics](TOKENOMICS.md) document.

## Project structure

```
vici-points/
  dfx.json                   Canister declarations (minter, ledger, index)
  Cargo.toml                 Rust workspace root
  TOKENOMICS.md              XP gameplay economics and reserve allocation
  src/
    minter/                  Minter canister (Rust)
      Cargo.toml
      minter.did             Auto-generated Candid interface
      README.md              Minter documentation
      src/                   Source code
  scripts/
    build.ledger.args.sh              Generates ledger init/upgrade arguments
    build.index.args.sh               Generates index init arguments
    init.reserves.sh                  Register minter reserves (6 gameplay reserves)
    init.reserves.config.example.sh   Copy to init.reserves.config.sh (6 principals, gitignored)
    did.sh                            Regenerates .did files from compiled WASM
    format.sh                         Code formatting
    lint.sh                           Linting
```

## Development

### Prerequisites

- [dfx](https://internetcomputer.org/docs/building-apps/getting-started/install) (Internet Computer SDK)
- [Rust](https://rustup.rs/) with the `wasm32-unknown-unknown` target
- [candid-extractor](https://crates.io/crates/candid-extractor) (for `.did` generation)

### Local deployment

```bash
dfx start --background
dfx deploy
```

### Regenerate Candid interface

```bash
bash scripts/did.sh
```

### Lint and format

```bash
bash scripts/lint.sh
bash scripts/format.sh
```

## Links

- [VICI Token (vici-icrc)](https://github.com/AntoninoVentworthy/vici-icrc) — the reward/coordination token
- [ICRC-1 Standard](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-1/README.md)
- [ICRC-2 Standard](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-2/README.md)
- [ICRC-3 Standard](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-3/README.md)
- [DFINITY Ledger Suite Releases](https://github.com/dfinity/ic/releases)
- [Internet Computer Documentation](https://internetcomputer.org/docs)
