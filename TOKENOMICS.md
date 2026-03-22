# VICI tokenomics

This document describes the intended economic design for VICI. On-chain, VICI is an **ICRC** token: the ledger allows new supply to be created from the designated minting account. **Economic policy** therefore does not rely on a hard-coded fixed supply in the ledger; it relies on **how the minter is configured**—in particular [reserve accounts](src/minter/README.md) with **lifetime mint caps** and other limits.

## ICRC reality vs. target economics

| Aspect                     | ICRC / ledger                           | Target policy                                                                                                  |
| -------------------------- | --------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| New tokens                 | Minted from the minting account         | Same mechanism                                                                                                 |
| Maximum circulating design | Not “1B hard cap” in the token standard | **1,000,000,000 VICI** as the **maximum amount we commit to mint in total**, enforced by reserve configuration |
| Where minted tokens go     | Transfers from minter                   | Only to **pre-approved reserve accounts**; the minter does not mint to arbitrary users                         |

**Operational model:** treat each major allocation (treasury, community incentives, team, etc.) as one or more **minter reserves**. Set each reserve’s `lifetime_received_maximum` (and related fields) so that the **sum of lifetime maximums across all reserves does not exceed 1B VICI** (in base units, respecting decimals). Additional safety layers (`max_balance`, `max_topup_per_rebalance`, `rate_limits`, global `minting_enabled`, `max_mint_per_operation`) align short-term flows with long-term emission intent.

The [minter README](src/minter/README.md) documents the exact fields (`lifetime_received_minimum`, `lifetime_received_maximum`, rebalance rules, and so on).

---

## Total supply and initial allocation (policy)

**Target maximum minted supply:** **1,000,000,000 VICI** (fixed for economic planning).

**Allocation (authoritative model; supersedes any older placeholder splits such as 20/20/20/20/20 in draft whitepaper material):**

| Category             | Share   | Tokens | Reserve role                                                   |
| -------------------- | ------- | ------ | -------------------------------------------------------------- |
| Community incentives | **45%** | 450M   | Emissions, programmes, usage-linked distribution               |
| Treasury             | **20%** | 200M   | Ecosystem, liquidity, R&D, discretionary grants                |
| Team                 | **15%** | 150M   | Team allocation (with vesting off-chain or via separate locks) |
| Investors            | **15%** | 150M   | Investor allocation (with vesting)                             |
| Advisors             | **5%**  | 50M    | Advisor allocation (with vesting)                              |

**Interpretation:** the majority of tokens are **not** notionally “pre-sold”; they are released over time through usage, incentives, and governance-aligned programmes—implemented by **routing mints into the corresponding reserves** and distributing from those accounts according to product rules.

---

## Emissions (community pool)

From the **450M** community allocation, the **intended release shape** over time:

| Phase     | Share of community allocation | Approx. from 450M |
| --------- | ----------------------------- | ----------------- |
| Years 1–3 | **50%**                       | ~225M             |
| Years 4–7 | **35%**                       | ~157.5M           |
| Year 8+   | **15%**                       | ~67.5M            |

This is **policy and scheduling**, implemented by:

- reserve **rate limits** and rebalance parameters,
- programme-level distribution from community/treasury reserves,
- and (where applicable) burning or locking outside the minter.

**Intent:** stronger early incentives for bootstrapping, then a long tail to reduce speculative inflation pressure.

### Community sub-reserves

The 450M community allocation is **not** a single blob. It is split into six on-chain minter reserves, each with its own lifetime cap and rate limits enforced by the minter:

| Reserve        | Cap (VICI) | % of 45% | Purpose                                   | Auto-rebalance |
| -------------- | ---------- | -------- | ----------------------------------------- | -------------- |
| **forecast**   | 135M       | 30%      | Forecast/prediction rewards               | yes            |
| **liquidity**  | 112.5M     | 25%      | Liquidity provider incentives             | yes            |
| **onboarding** | 67.5M      | 15%      | New user signup and activation bonuses    | yes            |
| **oracle**     | 67.5M      | 15%      | Market creation and oracle resolution     | yes            |
| **campaign**   | 45M        | 10%      | Ecosystem campaigns, partnerships, events | yes            |
| **buffer**     | 22.5M      | 5%       | Strategic reserve for future needs        | no (manual)    |

Sub-reserve caps sum exactly to 450M. Each is registered as a separate minter reserve with its own principal, so per-category caps are enforced on-chain — not just in application logic.

### Daily emission budget

Year 1–3 target: **~75M/year = ~205k VICI/day** across all community buckets.

| Reserve        | Daily budget | Target balance (7 d) | Min balance (2 d) | Daily rate limit | Yearly rate limit |
| -------------- | ------------ | -------------------- | ----------------- | ---------------- | ----------------- |
| **forecast**   | 80k          | 560k                 | 160k              | 160k             | 29.2M             |
| **liquidity**  | 60k          | 420k                 | 120k              | 120k             | 21.9M             |
| **onboarding** | 30k          | 210k                 | 60k               | 60k              | 10.95M            |
| **oracle**     | 20k          | 140k                 | 40k               | 40k              | 7.3M              |
| **campaign**   | 10k          | 70k                  | 20k               | 20k              | 3.65M             |

The minter's auto-rebalance timer checks every hour. When a reserve's balance drops below its target, the minter refills it — subject to all configured caps, rate limits, and the lifetime maximum. Daily rate limits are set at 2x the daily budget to allow catch-up after downtime.

All parameters are adjustable at runtime via `update_reserve` — no canister redeployment required.

---

## Incentives: who earns VICI

Core principle: VICI is earned through **useful behaviour**, not passive holding.

| Activity             | Reward (design intent) |
| -------------------- | ---------------------- |
| Correct predictions  | VICI                   |
| Liquidity provision  | VICI                   |
| Market creation      | VICI                   |
| Dispute resolution   | VICI                   |
| Community moderation | VICI                   |

**Design principle:** avoid “passive yield” or “hold to earn” as the primary story—rewards are tied to **accuracy, participation, and contribution**. This supports a compliance-conscious framing (EU and similar jurisdictions).

---

## Staking and alignment (product layer)

Staking mechanics are specified at the **application / protocol** layer (not in the ICRC ledger). Intended roles:

1. **Market creation staking** — Stake (e.g. 500 VICI) to create a market; good behaviour → stake returned plus rewards; abuse or spam → **slash / burn** (policy-defined).
2. **Reputation / confidence staking** — Stake behind predictions (e.g. 1,000 VICI); correct → higher rewards and reputation; wrong → loss of stake or opportunity cost.
3. **Oracle staking** — Oracles stake to resolve markets; incorrect resolution → **slashed** stake.
4. **Governance / utility staking (where applicable)** — Fee discounts, voting weight, feature access.

Exact percentages, lock durations, and formulas are **not** fixed in this repository; they belong in protocol specs and on-chain logic above the ledger.

---

## Liquidity incentives

- **Who:** liquidity providers, market makers, early traders (as defined by each programme).
- **Rewards:** trading fees plus **VICI emissions** from the incentive reserves.
- **Dynamic:** incentive intensity **decreases over time**—bootstrap liquidity early, then rely more on organic fees and market depth.

---

## Revenue model and distribution (example)

**Example trading fee:** **1.5%** per trade (illustrative; actual fees are set by product).

**Illustrative split of protocol revenue:**

| Destination          | Share   |
| -------------------- | ------- |
| Treasury             | **50%** |
| Staking incentives   | **30%** |
| Liquidity programmes | **20%** |

**Important:** VICI is **not** designed as a profit-sharing security. Revenue **feeds treasury, incentives, and liquidity programmes** rather than guaranteeing pro-rata cash flow to token holders. Settlement of trades can remain in stablecoins (e.g. USDC); VICI acts as **coordination, staking, and governance** (where enabled).

---

## Treasury

The **treasury** allocation (200M in the table above) plus ongoing revenue supports:

- ecosystem grants,
- liquidity programmes,
- R&D,
- further community incentives,

subject to **token-holder governance** where implemented.

---

## Vesting (supply pressure)

| Group     | Vesting (intent)         |
| --------- | ------------------------ |
| Team      | 4 years + 12 month cliff |
| Investors | 3 years                  |
| Advisors  | 24 months                |

Vesting is enforced by **legal agreements**, vesting schedules in distribution contracts, and/or locked accounts—not by the ICRC ledger alone.

---

## Structural choices (summary)

| VICI is not (by design)          | VICI is                                |
| -------------------------------- | -------------------------------------- |
| Primary settlement currency      | A coordination and staking asset       |
| Stablecoin                       | Governance and utility (where enabled) |
| Direct profit-sharing instrument | Subject to incentive and policy design |

---

## Mental model (flow)

```text
Users trade with stablecoins (e.g. USDC)
        ↓
Protocol earns fees
        ↓
Split: treasury / staking incentives / liquidity programmes
        ↓
Users earn VICI through: accuracy, liquidity, creation, resolution, moderation
        ↓
Stake VICI for: markets, conviction, oracles, governance (where enabled)
        ↓
Good behaviour → rewards; bad behaviour → slashing / burns (policy)
```

---

## Operational architecture

The minter and the app backend have distinct responsibilities:

| Layer           | Responsibility                                                              |
| --------------- | --------------------------------------------------------------------------- |
| **Minter**      | Protocol-level emission control: lifetime caps, rate limits, auto-rebalance |
| **App backend** | User-level distribution: who earns rewards, signup bonuses, per-user caps   |

The app backend holds a funded reward wallet (one per community sub-reserve). The minter refills these wallets automatically. The backend distributes tokens to users based on application logic — the minter does not need to know about individual users, activity stats, or reward formulas.

### Refill flow

```text
Minter timer fires (every 1 hour)
  → for each auto-rebalance reserve:
      → query ledger balance
      → balance < target_balance?
      → compute refill amount (capped by all limits)
      → mint to reserve account
  → backend wallet stays funded
  → backend distributes to users based on app logic
```

## Engineering and design items still to specify

The minter enforces **per-reserve** and **global** limits; it does not encode prediction markets or vesting. Open items for protocol specs:

- **Reward formulas** per action type (backend responsibility).
- **Per-user caps** and anti-Sybil enforcement (backend responsibility).
- **Slash** percentages and beneficiaries (burn vs. treasury).
- **Staking** lock durations and unstaking delays.
- **Reputation** ↔ token weighting.

---

## Future improvement: governance-controlled minting

Today, minting policy is enforced by **minter configuration** (admin-controlled reserves, caps, and rate limits). A natural upgrade is to place the **minter under on-chain governance**—for example via the [NNS](https://internetcomputer.org/docs/building-apps/governing-apps/overview) or a dedicated governance canister—so that **changes to minting limits, new reserves, and emission parameters** require a vote by **owners and/or stakers**, aligned with the long-term token design.

---

## See also

- [Minter canister — reserves and minting](src/minter/README.md)
- [Project README](README.md)
