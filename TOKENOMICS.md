# Vici XP tokenomics

> [!TIP]
> **Looking for the short version?** See the [TL;DR](TLDR.md) — reserves, emission rates, and the full flow in one page.

This document describes the intended economic design for **Vici XP** (symbol: **VXP**), the gameplay point system for the Vici prediction platform. XP is an **ICRC** token on the Internet Computer, but it serves a fundamentally different purpose from the [VICI token](https://github.com/ViciApp/vici-icrc): XP is a **non-monetary, high-volume gameplay asset** designed for frictionless engagement.

## Dual-token context

Vici operates a dual-token model:

| Token                                                              | Symbol | Role                        | Who earns                        | Friction   |
| ------------------------------------------------------------------ | ------ | --------------------------- | -------------------------------- | ---------- |
| **Vici XP** (this repo)                                            | VXP    | Gameplay / onboarding layer | Everyone — every user, instantly | Zero       |
| **VICI Token** ([vici-icrc](https://github.com/ViciApp/vici-icrc)) | VICI   | Reward / coordination layer | Top / most active users — scarce | Higher bar |

A third layer — **settlement** (stablecoin for real-money prediction markets) — is a separate, future concern and is not part of either token.

### Why two tokens?

| Concern              | How the split helps                                                                                                                                                                    |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Regulation**       | XP is play money — no MiCA / e-money classification risk. VICI is a coordination/utility asset, not settlement currency. Clean separation avoids "1 token = 1 USD" ambiguity.          |
| **Growth**           | XP removes all friction: no wallet setup, no money feel. Critical for scaling in emerging markets and building an addictive gameplay loop.                                             |
| **Incentive design** | XP drives engagement (progression, streaks, leaderboard). VICI adds scarce upside (earned by top users, unlocks advanced features). Different distribution curves for different goals. |

---

## XP design principles

1. **Not money.** XP has no intended monetary value. It is not listed, not traded, not redeemable for cash.
2. **Abundant.** Every user earns XP by participating. The supply is generous — the goal is to make the gameplay loop feel rewarding, not scarce.
3. **Instant.** Earning XP should feel immediate: swipe, predict, get feedback, see your score change.
4. **Progression-driven.** XP powers leaderboards, streaks, levels, and reputation. Status comes from accumulated XP, not from buying it.

---

## ICRC reality vs. target economics

| Aspect                 | ICRC / ledger                   | Target policy                                                                           |
| ---------------------- | ------------------------------- | --------------------------------------------------------------------------------------- |
| New tokens             | Minted from the minting account | Same mechanism                                                                          |
| Maximum supply         | Not a hard ledger cap           | **1,000,000,000 XP** as the maximum committed amount, enforced by reserve configuration |
| Where minted tokens go | Transfers from minter           | Only to **pre-approved reserve accounts**; the minter does not mint to arbitrary users  |

**Operational model:** each gameplay category (predictions, onboarding, streaks, etc.) maps to a **minter reserve** with its own `lifetime_received_maximum` and rate limits. The sum of all reserve caps equals 1B XP. The [minter README](src/minter/README.md) documents the exact fields.

---

## Total supply and allocation

**Target maximum minted supply:** **1,000,000,000 XP**.

All XP is community/gameplay allocation. There are no corporate reserves (no treasury, team, investor, or advisor allocations — those belong to the [VICI token](https://github.com/ViciApp/vici-icrc)).

| Reserve         | Cap (XP) | Share | Purpose                                             | Auto-rebalance |
| --------------- | -------- | ----- | --------------------------------------------------- | -------------- |
| **forecast**    | 400M     | 40%   | Prediction participation rewards                    | yes            |
| **onboarding**  | 200M     | 20%   | Signup bonuses, activation, tutorial completion     | yes            |
| **streaks**     | 200M     | 20%   | Daily engagement, login streaks, streak multipliers | yes            |
| **leaderboard** | 100M     | 10%   | Periodic leaderboard prizes, competition rewards    | yes            |
| **campaign**    | 50M      | 5%    | Promotions, referral bonuses, events                | yes            |
| **buffer**      | 50M      | 5%    | Strategic reserve for future gameplay features      | no (manual)    |

Reserve caps sum exactly to 1,000M. Each is registered as a separate minter reserve with its own principal, so per-category caps are enforced on-chain.

---

## Emissions

### Emission schedule

XP emissions are generous — every user should feel rewarded for participating.

| Phase     | Share of total supply | Approx. from 1B |
| --------- | --------------------- | --------------- |
| Years 1–3 | **60%**               | ~600M           |
| Years 4–7 | **30%**               | ~300M           |
| Year 8+   | **10%**               | ~100M           |

**Intent:** strong early emissions to bootstrap the user base and establish the gameplay loop, then a long tail.

### Daily emission budget

Year 1–3 target: **~200M/year = ~548k XP/day** across all gameplay buckets.

| Reserve         | Daily budget | Target balance (7 d) | Min balance (2 d) | Daily rate limit | Yearly rate limit |
| --------------- | ------------ | -------------------- | ----------------- | ---------------- | ----------------- |
| **forecast**    | 220k         | 1.54M                | 440k              | 440k             | 80.3M             |
| **onboarding**  | 130k         | 910k                 | 260k              | 260k             | 47.45M            |
| **streaks**     | 110k         | 770k                 | 220k              | 220k             | 40.15M            |
| **leaderboard** | 55k          | 385k                 | 110k              | 110k             | 20.075M           |
| **campaign**    | 33k          | 231k                 | 66k               | 66k              | 12.045M           |

The minter's auto-rebalance timer checks every hour. When a reserve's balance drops below its target, the minter refills it — subject to all configured caps, rate limits, and the lifetime maximum. Daily rate limits are set at 2x the daily budget to allow catch-up after downtime.

All parameters are adjustable at runtime via `update_reserve` — no canister redeployment required.

---

## Who earns XP

Every user earns XP through participation — XP is not scarce or exclusive.

| Activity               | XP reward (design intent)                |
| ---------------------- | ---------------------------------------- |
| Making a prediction    | XP (win or lose — participation matters) |
| Correct predictions    | Bonus XP                                 |
| Daily login / streak   | XP (escalating with streak length)       |
| Completing onboarding  | XP                                       |
| Referring a friend     | XP                                       |
| Leaderboard placement  | XP prizes (periodic)                     |
| Campaign participation | XP (event-specific)                      |

**Design principle:** XP rewards **participation and consistency**, not just accuracy. The goal is to build habits (predict daily, maintain streaks, climb leaderboards) and make the app feel rewarding from the first interaction.

---

## The gameplay loop

```text
User opens app
  → swipe to predict (instant, no friction)
  → earn XP immediately (participation reward)
  → correct? bonus XP
  → streak maintained? multiplier XP
  → check leaderboard position
  → climb ranks, unlock status
  → come back tomorrow (streak incentive)
```

XP powers the entire engagement loop. The app backend decides the exact formulas (how much XP per prediction, streak multipliers, leaderboard prize pools) — the minter only ensures reserves stay funded within caps.

---

## Operational architecture

The minter and the app backend have distinct responsibilities:

| Layer           | Responsibility                                                              |
| --------------- | --------------------------------------------------------------------------- |
| **Minter**      | Protocol-level emission control: lifetime caps, rate limits, auto-rebalance |
| **App backend** | User-level distribution: who earns XP, how much, per-user caps, anti-Sybil  |

The app backend holds funded reward wallets (one per gameplay reserve). The minter refills these wallets automatically. The backend distributes XP to users based on application logic — the minter does not know about individual users, activity stats, or reward formulas.

### Refill flow

```text
Minter timer fires (every 1 hour)
  → for each auto-rebalance reserve:
      → query ledger balance
      → balance < target_balance?
      → compute refill amount (capped by all limits)
      → mint to reserve account
  → backend wallet stays funded
  → backend distributes XP to users based on app logic
```

---

## Relationship to VICI token

XP and VICI serve complementary but distinct roles:

|                          | XP                              | VICI                                |
| ------------------------ | ------------------------------- | ----------------------------------- |
| **Earning**              | Everyone, instantly             | Top/most active users only          |
| **Volume**               | High (generous emission)        | Low (scarce)                        |
| **Purpose**              | Engagement, progression, status | Rewards, utility, coordination      |
| **Tradeable**            | No                              | Yes (where applicable)              |
| **Corporate allocation** | None                            | Treasury, team, investors, advisors |

XP is the **on-ramp**: users start earning immediately, build habits, and establish reputation. VICI is the **upside**: the best users earn scarce rewards with real utility (feature access, advanced modes, private competitions).

---

## Engineering items still to specify

The minter enforces **per-reserve** and **global** limits; it does not encode gameplay logic. Open items for the app/protocol layer:

- **Reward formulas** per activity type (backend responsibility).
- **Per-user caps** and anti-Sybil enforcement (backend responsibility).
- **Streak multiplier** curves and reset rules.
- **Leaderboard** prize pool sizes and competition cadence.
- **Level / rank** thresholds tied to cumulative XP.

---

## See also

- [Minter canister — reserves and minting](src/minter/README.md)
- [Project README](README.md)
- [VICI Token (vici-icrc)](https://github.com/ViciApp/vici-icrc) — the reward/coordination token
