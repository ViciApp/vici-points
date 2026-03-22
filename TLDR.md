# Vici XP — TL;DR

**What:** Gameplay point system (symbol: **VXP**). Non-monetary, high-volume, zero friction.

**Total supply:** 1,000,000,000 XP

**Who earns:** Everyone. Every user, instantly, by participating.

---

## Reserves

| Reserve         | Cap  | %   | Who gets it                                       | Auto-refill       |
| --------------- | ---- | --- | ------------------------------------------------- | ----------------- |
| **forecast**    | 400M | 40% | Every user who makes a prediction (win or lose)   | yes, ~220k XP/day |
| **onboarding**  | 200M | 20% | New users (signup bonus, activation, tutorials)   | yes, ~130k XP/day |
| **streaks**     | 200M | 20% | Users maintaining daily activity (login, predict) | yes, ~110k XP/day |
| **leaderboard** | 100M | 10% | Users placing in periodic competitions            | yes, ~55k XP/day  |
| **campaign**    | 50M  | 5%  | Promotion participants, referrals, events         | yes, ~33k XP/day  |
| **buffer**      | 50M  | 5%  | Reserved for future gameplay features             | manual only       |

No treasury, team, investor, or advisor reserves — those belong to [VICI](https://github.com/AntoninoVentworthy/vici-icrc).

---

## Emission schedule

| Phase     | Amount | Daily rate   |
| --------- | ------ | ------------ |
| Years 1–3 | ~600M  | ~548k XP/day |
| Years 4–7 | ~300M  | decreasing   |
| Year 8+   | ~100M  | long tail    |

---

## How it flows

```
Minter (auto, every hour)
  → checks each reserve's balance
  → below target? mint to refill (capped by lifetime max + rate limits)
  → reserve wallets stay funded

App backend (holds reserve wallets)
  → user predicts → XP from forecast reserve
  → user signs up → XP from onboarding reserve
  → user maintains streak → XP from streaks reserve
  → user places on leaderboard → XP from leaderboard reserve
  → user joins campaign → XP from campaign reserve

Users never interact with the minter. They just see XP arrive.
```

---

## Key design choices

- **Not money.** XP has no monetary value, is not traded, not redeemable.
- **Abundant.** Everyone earns. The goal is to make the gameplay loop feel rewarding.
- **Instant.** Swipe, predict, get feedback, see score change.
- **Minter is dumb.** It only refills wallets. The app decides who gets what.

---

## See also

- [Tokenomics](TOKENOMICS.md) — full economic design
- [Minter README](src/minter/README.md) — how the reserve system works
- [VICI Token](https://github.com/AntoninoVentworthy/vici-icrc) — the scarce reward/coordination token
