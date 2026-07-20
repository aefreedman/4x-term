---
status: complete
priority: p1
issue_id: 006
tags: [economy, pricing, markets, currency, simulation, design]
supersedes: [004, 005]
dependencies: []
---
# Slice 1: Energy-Denominated Economy Foundation

## Purpose

First of two successive design slices. This slice delivers a solvent,
deadlock-free economy foundation: energy as the currency, cost-aware
pricing, funded trade commitments, and the physical energy layer
(generation, life-support burn, storage) that the economy runs on.

This document **supersedes** `todos/004-*` (trade commitments and market
liquidity) and `todos/005-*` (cost-aware price formation). Their findings,
diagnostics, and acceptance criteria are folded in here; mark both
superseded when this slice is accepted. Slice 2
(`todos/007-*`) builds world dynamics and player progression on top of this
foundation.

## Worldbuilding premise

**Energy is life support, and supply is physics.** Every populated system
burns energy each tick to keep its population alive. Each system's
generation is determined by its star and collector configuration, so energy
is positional: good sunlight and good resources are rarely in the same
place, and trade exists to move energy from where it pools to where life
needs it. Traders are circulation, not merchants.

**Naming:** the internal/content ID is the generic `energy` (e.g.
`core:energy`). The player-facing name and physical fiction are a
presentation decision; the simulation depends only on the internal ID.

## Core decisions

- **D1. Energy is the numéraire.** All prices, balances, cost bases, and
  settlements are denominated in energy units. The existing `Money` integer
  newtype survives, re-denominated as energy minor units. One currency-good;
  goods trade against energy, not against each other.
- **D2. Markets are hard-budget actors.** A market's purchasing power is its
  unreserved energy stock. Liquidity refills through generation and through
  energy paid in by traders and production — a flow, not a fixed pool.
- **D3. Currency is physical.** Energy in a market is one inventory line
  that is simultaneously purchasing power, a production input, and a
  sellable good. For traders, energy-in-tank is the wallet; tank capacity is
  bounded, so wealth above the cap must convert into goods (natural
  anti-hoarding pressure). Cargo bay and tank are separate capacities.
- **D4. Commitments: funded quantity reservations with partial-sale
  fallback.**
- **D5. Pricing: cost-aware automated market maker** with weighted-average
  cost basis and scarcity as an adjustment above a sustainable floor.

## Rationale (condensed from superseded todos)

- The tick-300 stall reproduced in todo 004 came from a conserved currency
  pool (¤210,000) concentrating away from consuming systems while traders
  retried atomic full-stack sales forever. `economy_loop_mock.rs` showed the
  fix shape: baseline liquidity died at cycle 166, while the
  energy-spending + tertiary-support loop survived all 1,000 cycles. Energy
  as currency makes that loop the base mechanic: traders burn currency to
  move, pay it back into markets when refueling, and generating systems
  continuously mint liquidity.
- Todo 005's circular-pricing risk (output revenue → input bids → output
  revenue, plus tertiary-supports-extraction loops) terminates cleanly:
  energy's cost is definitionally 1, and every other good's cost basis
  grounds out in energy burned during extraction, processing, transport, and
  upkeep. Cost basis = embodied energy, physically conserved and auditable.
- Global solvency reduces to one flow balance — energy generated vs. burned —
  with slight overproduction as the healthy default. Local energy
  scarcity/abundance stays uneven, which is the desired lumpy gameplay.

## Instructions: physical energy layer

1. **Generation.** Each system has `energy_output_per_tick`, authored
   directly or compiled from star parameters (luminosity × collector
   efficiency). The content pipeline compiles to a single per-tick rate;
   simulation systems consume only the compiled rate.
2. **Life-support burn.** Each populated system burns
   `life_support_burn_per_capita × population` per tick, unconditionally.
   Every system — including structural exporters — therefore has an energy
   sink. Population is a static authored integer in this slice; dynamics
   arrive in Slice 2.
3. **Storage.** Each system has `energy_storage_cap`. Generation above the
   cap is wasted (pick lost vs. blocked and document it). A full battery
   forces exporters to sell cheap or waste output; a draining battery is a
   visible countdown. Surface market energy stock vs. cap in the TUI as the
   system health readout.
4. **Emergent roles.** Net flow (generation − burn) sorts systems into
   exporters, importers, and knife-edge systems. Author content to
   anti-correlate solar quality with raw material sources so trade is
   structurally mandatory.
5. **Bootstrap viability check (required).** Baseline burn is a hard clock.
   Add a content-validation or diagnostic check that each net-importer's
   starting energy stock covers its burn for longer than a plausible
   time-to-first-delivery given map distances and initial fleet capacity.

## Instructions: trade commitments and liquidity

1. **Reservation object.** When a trader selects an opportunity, the
   destination reserves (good, quantity, energy amount) where
   `quantity <= funded quantity = floor(unreserved_energy_available_for_purchases / bid_price)`.
   Reserved energy is unavailable to competing reservations and to local
   production spending. (Validated arithmetic: a ¤313 market funds 24 units
   of a 30-unit stack at ¤13; the remainder is unavailable to a competing
   reservation.)
2. **Available-for-purchases definition.** `unreserved energy = energy stock
   − outstanding reservations − operating reserve`, where operating reserve
   is an authored parameter expressing N ticks of the market's own
   production, upkeep, and life-support burn. This parameter is the primary
   designer lumpiness knob.
3. **Deterministic contention resolution.** Collect all reservation requests
   raised in a tick and resolve them in a stable total order (opportunity
   score, then trader stable content ID). Resolution must be independent of
   system iteration order.
4. **Price timing.** Reservation locks a floor price; settlement may float
   upward with market conditions but never below the reserved floor, so
   trader price risk is one-sided and analyzable.
5. **Lifecycle.** Define expiry (ticks-to-live, refreshed while the trader
   is en route to that destination), cancellation, and partial fulfillment.
   Expired reservations release energy atomically.
6. **Partial-sale fallback.** A laden trader may always sell any sellable
   sub-quantity. Sale failures must be handled explicitly — the current
   `lib.rs` path that discards the sell error and retries the full stack is
   the direct cause of the stall and must be removed.
7. **Anti-strand invariant (required).** Any market will buy, at liquidation
   price, at least enough of a laden trader's cargo to fund one jump to the
   nearest neighboring system. Specify the liquidation price rule
   explicitly. This guarantees no trader is permanently stationary for
   economic reasons.

## Instructions: price formation

1. **Cost basis.** Per market per good, maintain a weighted-average
   acquisition cost in energy units. Recipe execution transfers consumed
   input cost plus energy burned as operating cost into output cost basis,
   distributed across multi-output recipes by an explicit authored
   weighting.
2. **Terminal anchors.** Energy = 1 by definition. Raw sources' output cost
   basis = energy burned per unit extracted, giving every chain a
   non-recursive ground truth. Initial world inventory takes its cost basis
   from authored bootstrap values in `goods.ron` (reference prices
   re-interpreted as energy-denominated bootstrap cost).
3. **Asks.** `ask = cost_basis × (1 + margin) × scarcity_adjustment`, with
   the scarcity multiplier bounded below by 1.0 in normal operation. The
   current scarcity formula survives as the adjustment term.
4. **Bids.** Processor input bids bounded above by
   `(expected output ask × yield − required margin) / input quantity`. Safe
   from recursion because expected output ask derives from the output's
   current cost basis, which chains to energy.
5. **Liquidation policy.** Below-cost sales occur only under an explicit
   liquidation rule (inventory above target by an authored threshold) and
   under the anti-strand invariant. Today's accidental below-cost behavior
   (five of six recipes losing money at two units of untargeted inventory)
   must become impossible outside those rules.
6. **Untargeted goods.** Give untargeted goods a real default inventory
   target or an explicit no-discount-until-threshold rule, replacing the
   implicit target of one.
7. **Advertised demand.** A market's advertised bid quantity is always its
   funded quantity per the commitments section — one shared definition, one
   implementation.

## Instructions: per-market policy component (plumbing only)

Introduce the data seam now; the feature that uses it ships in Slice 2.

1. Every market carries an explicit policy component: operating reserve
   (ticks), margin, import priorities, liquidation threshold/discount.
2. `economy_config.ron` values become *defaults* for this component;
   per-system overrides live in `economy.ron`. Market systems read
   pricing/liquidity parameters only through the market's policy component,
   never from global config directly.
3. Policy mutation is a normal typed command through the existing command
   boundary. In this slice the only writers are content loading and tests.

## Configuration surface (expected `economy_config.ron` additions)

- Pricing mode selector (existing scarcity model kept as comparison
  baseline).
- Producer margin default; optional per-recipe override in `economy.ron`.
- Operating reserve default (ticks of burn withheld from purchasing).
- Reservation TTL; liquidation discount and threshold.
- Life-support burn per capita; per-system generation, storage cap, and
  starting population in `economy.ron`.
- Trader energy burn per distance and refuel behavior in `traders.ron`.

All new fields validated by `game-content` with source-aware diagnostics.

## Diagnostics and validation

Extend `--economy-diagnostics`:

- Replace the fixed-total conservation check with an energy flow ledger:
  generated − burned (travel + production + upkeep + life support) − Δstock
  ≡ 0 globally, deterministic and overflow-safe.
- Per-market: realized input cost, output revenue, margin, funded demand,
  reserved energy, stock vs. storage cap, inventory vs. target.
- Health targets: global net energy flow slightly positive and flat; high
  per-system stock variance (lumpiness is the goal, not a defect).

Required tests (absorbing the superseded todos' acceptance criteria):

- Multi-trader same-tick contention over one opportunity resolves
  deterministically and never over-reserves funded quantity.
- A destination that cannot afford a full stack triggers partial sale and
  reroute; the anti-strand test shows no trader permanently stationary.
- Cost propagation through single-output, multi-output, and consuming
  recipes.
- Bootstrap viability check catches an importer authored with insufficient
  starting stock.
- 1,000-tick deterministic run sustains trade activity past tick 300 with no
  stationary-laden NPCs and no processor structural insolvency.
- A/B harness: old scarcity model vs. new model over identical content and
  seed.

## Sequencing within this slice

1. Record the durable energy-as-numéraire contract in `archive/market-trading-prototype/docs/energy-economy.md`; keep `archive/market-trading-prototype/docs/initial-prototype.md` historical.
2. Physical layer: generation, life-support burn, storage, bootstrap check.
3. Price formation (upstream of what a reservation is worth).
4. Commitments and liquidity, sharing the funded-quantity definition.
5. Policy component plumbing (any time after step 1; earlier is cheaper).
6. Implement behind the pricing-mode selector; run the A/B diagnostics
   comparison before making the new model the default.
7. Retire the mock-only assumptions in `pricing_model_mock.rs` and
   `economy_loop_mock.rs` that the real implementation supersedes, and mark
   todos 004 and 005 superseded by this document.

## Known risks / watch items

- **Death spirals replace liquidity freezes** as the failure shape: broke =
  stranded. The anti-strand invariant is the mitigation; test it explicitly.
- **Energy price shocks propagate everywhere** because every quote is
  denominated in energy. Consider smoothing or rate-limiting energy price
  movement.
- **Bootstrap starvation** is a new deadlock class distinct from the tick-300
  stall; the viability check is the guard.
- **Tank-as-wallet UX:** the buy/sell UI must make clear that spending
  currency and consuming travel energy draw from the same stock.
- **Currency concentration** should now self-correct through universal burn;
  if diagnostics still show harmful pooling, energy decay (demurrage) is the
  reserve tool to evaluate before any credit mechanism.

## Work Log

### 2026-07-12 - Implement and accept Slice 1

**By:** OpenAI

**Actions:**
- Replaced abstract currency with physical `Energy`, canonical market `core:energy` inventory, trader tanks, haulable bay energy, generation, life support, storage, embodied costs, policy, reservations, partial settlement, and anti-strand routing across all five crates.
- Migrated and validated all repository content, including fixed-point solar compilation, system roles, bootstrap feasibility, recipe energy/weights, and refuel policy.
- Added immutable application views, TUI health states, exact CLI flow/processor diagnostics, pricing A/B support, and `archive/market-trading-prototype/docs/energy-economy.md`.
- Ran format, Clippy, workspace tests, content validation, and deterministic 1,000-tick cost-aware/A-B diagnostics.

**Learnings:**
- Reservation claims may be degraded only by mandatory life support; settlement must then recompute a funded partial quantity and release the remainder without failing the tick.
- The computed liquidation budget must be recomputed atomically when mutable liquidation policy changes, while remaining independent of operating reserve.
- Sustainable asks and non-recursive processor bid ceilings are both required to keep processors structurally solvent.

## Notes

- Preserve validate-before-mutate transaction guarantees and deterministic
  integer arithmetic throughout.
- All new state is `game-core` components/resources; all parameters flow
  through `game-content` validation.
- The unrelated untracked `.obsidian/` directory must remain untouched.
