# Energy-Denominated Economy

This document defines the current Slice 1 economy. Todo 006 is the authoritative design source; the initial-prototype document remains historical.

## Compatibility

The prototype economy was replaced rather than migrated. Persistence does not exist, so there is no save-schema compatibility requirement. Runtime and content use the checked integer `Energy(i64)` newtype; the former `Money` API and abstract market/trader balances no longer exist.

## Physical model

`core:energy` is both the numéraire and a physical good. Every price, embodied cost, reservation, and settlement is denominated in energy minor units.

Each market has one canonical `core:energy` inventory line. It is simultaneously storage, production input, purchasing power, and tradable stock. Reservations and protected budgets are claims or spending constraints, not additional inventories.

A trader has two distinct physical stores:

- **Energy tank:** wallet and travel-burn source, bounded by tank capacity.
- **Cargo bay:** ordinary inventory, including haulable `core:energy`, bounded by cargo capacity. Bay energy cannot pay prices or power travel.

Buying energy cargo transfers market stock into the bay and pays the ask from the tank. Selling it transfers bay stock into the market and pays the funded bid into the tank. Tank deposit/withdraw and idle-NPC balancing are direct physical transfers; energy is never bought with a second abstract currency during refueling.

The closed loop is generation → market stock → trade proceeds → trader travel burn or spending at another market. Markets gain energy through generation and goods sold to traders. Travel is a universal sink.

## Generation, life support, and storage

`economy.ron` authors fixed-point `star_luminosity` and `collector_efficiency` (0–1000). `game-content` compiles the runtime rate with checked integer arithmetic:

```text
energy_output_per_tick = floor(star_luminosity × collector_efficiency / 1000)
```

`game-core` receives only that compiled rate. Solar quality is deliberately anti-correlated with major raw-resource placement, creating structural exporters, importers, and knife-edge systems.

Every tick generates before mandatory life support. Stock is capped at `energy_storage_cap`; overflow is lost and recorded as curtailed energy. Life support assesses:

```text
required = life_support_burn_per_capita × population
```

Available stock burns first. Any shortfall leaves stock at zero and increments `life_support_unsupplied`; population consequences are deferred to Slice 2. Production and purchasing cannot pre-empt this obligation. Source output uses one fixed-point percentage contract everywhere: `floor(quantity_per_tick × source_output_percent / 100)`. The scaled quantity drives runtime output and extraction burn, operating-reserve compilation, role classification, and bootstrap runway calculations. Active reservation claims and the protected liquidation budget are both retained by every source and recipe burn independently of the operating-reserve policy. The operating reserve remains a tunable purchasing policy rather than a correctness guard for discretionary burn.

## Cost basis and pricing

Each market/good stores `(stock_quantity, total_embodied_energy)`. Unit cost uses checked ceiling division. `core:energy` has embodied cost exactly 1 per unit; this anchors the chain but does **not** clamp its bid or ask to 1.

Raw output receives extraction-energy cost. Recipe output receives removed input cost plus operating energy. A recipe's authored margin override, when present, replaces the market producer margin for that recipe's outputs; if multiple local recipes produce one good, the highest explicit override is the deterministic floor. Multi-output recipes author positive `cost_weight` values. Runtime first sorts outputs by stable `ContentId`, floors every proportional share against the full weight sum, then assigns each remaining unit to the earliest IDs in that order. Allocation is therefore invariant to authored output order while preserving the exact total.

Cost-aware normal asks compound a sustainable margin and a bounded scarcity multiplier:

```text
sustainable = ceil(cost_basis × (100 + producer_margin_percent) / 100)
scarcity = 1.000 + ceil(0.500 × min(shortage, target) / target)
ask = ceil(sustainable × scarcity)
```

The fixed-point scarcity multiplier is bounded to 1.000–1.500. Every multiply and rounding step is checked integer arithmetic. Energy follows the same formula: its basis remains exactly 1, while its integral ask can rise through margin and scarcity. Explicit liquidation pricing applies the configured discount to the good's validated bootstrap-cost reference through one shared content/runtime contract and is the only below-floor path. Dynamic bids cannot change this guarantee price. For processor inputs, each eligible recipe derives a non-recursive maximum input budget from current output cost-basis asks and yield, subtracts operating energy, and distributes that budget deterministically across inputs in proportion to grounded embodied costs. A good consumed by multiple local recipes uses the minimum eligible ceiling, guaranteeing every eligible process remains structurally solvent; import priority can lower but never raise that ceiling. The liquidation threshold is a percentage of the cheapest adjacent-leg burn; a laden trader below that tank threshold liquidates the minimum whole-unit payout needed to reach it. Scarcity mode remains available only for deterministic A/B diagnostics.

## Reserves, funding, and anti-strand protection

The authored operating reserve and computed liquidation budget are independent:

```text
operating reserve = life-support burn over the horizon
  + scheduled source/recipe burn over the horizon from cloned current carries
unreserved purchasing energy =
  energy stock
  − active reservation claims
  − operating reserve
  − protected liquidation budget
funded quantity = floor(max(0, unreserved purchasing energy) / bid)
```

The operating reserve is a tunable policy knob. Its horizon simulation uses distinct source and recipe schedule keys and never mutates persistent production carries. One shared core contract computes `protected_liquidation_budget` from graph adjacency, trader travel burn/capabilities, eligible cargo bootstrap references, and the policy's liquidation settings. `game-content` uses it during compilation; an atomic whole-policy replacement recomputes and validates it from current runtime inputs before applying either the policy or budget. It is never authored and changing operating-reserve ticks cannot weaken it.

Every laden trader can sell the funded liquidation sub-quantity needed to afford the cheapest adjacent jump. The same checked funded-settlement primitive handles ordinary sales, energy cargo, reservations, partial sales, and liquidation. Remaining cargo is deterministically rerouted rather than retried as an ignored full-stack failure.

## Reservations and deterministic execution

A reservation encumbers existing destination energy; it does not transfer stock into escrow. It records trader, destination, good, quantity, claim, floor unit price, expiry tick, and status.

Command-driven, laden-reroute, and automated commitment requests enter one pending queue. It resolves once per tick in a stable total order: opportunity score, trader ID, good ID, destination ID, then request kind. Each acceptance recalculates available funding. Creation, refresh, cancellation, expiry, partial fulfillment, and release update the claim exactly once without entering the physical flow ledger. En-route reservations refresh their TTL. Mandatory life support may exhaust stock that was claimed earlier. On arrival, settlement therefore recomputes the quantity funded by current physical stock after other claims, operating reserve, and protected budget, then applies cargo quantity and tank-headroom limits. It settles that partial quantity at no less than the locked floor, releases the entire unused claim exactly once, and sends remaining automated cargo through liquidation or deterministic rerouting without failing the tick for expected insufficiency. Integrity and overflow failures still propagate.

Travel energy is the checked sum of `ceil(leg_distance × energy_per_distance)` once for every route leg. Planning, departure, rerouting, and bootstrap validation use that same rule. Bay energy is excluded. Multi-component operations calculate complete checked next state before applying it or emitting events.

## World-dynamics contracts

Slice 2 extends the same physical economy additively. Its compatibility defaults are zero-amplitude seasons, static population, a fixed NPC fleet, and disabled investments.

### Brownout ladder and throughput

After generation, storage capping, and mandatory life support, each market computes integer runway as `floor(energy stock / life-support obligation)`. A zero obligation has unlimited runway. Unsupplied life support forces **Starvation**; otherwise ordered authored entry thresholds select **Normal**, **Throttled**, **Emergency**, or **Starvation**. Authored recovery thresholds are higher than their corresponding entry thresholds, and a minimum stage duration prevents edge chatter. A severe shock may cross several entry bands in one transition, while recovery proceeds one band at a time. Core owns per-stage occupancy and transition counts.

The current stage derives an operating profile without rewriting authored market policy. Normal retains 100% throughput. Throttled reduces industrial throughput. Emergency and Starvation allow demand only for authored survival goods (which must include `core:energy`), disable investment eligibility, and raise the energy bid toward—but never above—the authored emergency ceiling. Core validates that ceiling against each compiled market's conservative maximum normal energy bid, so entering distress can never lower the bid. Suppression governs newly advertised demand and overrides future route-subsidy premiums; it does not cancel existing reservations. Those reservations continue through the normal funded partial-settlement and release lifecycle.

Production uses one fixed-point contract:

```text
effective = floor((base × stage_percent × labor_percent + carry) / 10_000)
next_carry = (base × stage_percent × labor_percent + carry) mod 10_000
```

Stage and labor are multiplied before rounding. There is one final carry per production schedule, never one carry per modifier. Source execution, recipes, and operating-reserve estimates call the same checked helper. Static population supplies a 100% labor modifier in the current checkpoint.

Every ordinary quote, funded quantity, reservation, settlement, and tank withdrawal continues to derive spendable energy from canonical stock after active claims, the stage-aware operating reserve, and the graph-compiled protected liquidation budget. Source and recipe burn independently retain active claims and protected liquidation energy. Stage transitions therefore change policy capacity without creating a second treasury or revoking anti-strand protection.

### Deterministic seasons

Each system has a compiled base generation rate and an integer triangle-wave definition with amplitude, period, and phase. The pure waveform is periodic, bounded to non-negative output, and uses checked integer arithmetic only. Amplitude zero returns the base rate exactly without evaluating phase arithmetic. The current brownout checkpoint still generates the base rate; the seasonal runtime writer remains deferred. Collector investment will change the base rate, and future seasonal output will always derive from that base. Repository defaults remain zero amplitude.

### Population, fleet, investment, and governance scaffolding

Population state records current/reference population, carrying capacity, a bounded sufficiency window and sum, a checked change remainder, and growth/decline counters. Population tuning resolves authored essential goods, per-thousand tertiary-demand mappings, a fixed-point growth sufficiency gate, and strictly increasing tier thresholds. Authored decline must be five to ten times the growth rate. Future growth uses checked logistic arithmetic,

```text
delta = floor(population × (cap − population) × growth_rate / (cap × 1000 × scale))
```

with the remainder retained and the result capped at `cap − population`. Population remains static in the current checkpoint.

Fleet configuration is explicitly `Fixed` or `Dynamic`. Dynamic configuration carries initial/max count, opportunity persistence, spawn cooldown, and retirement thresholds/windows, while the current checkpoint executes fixed mode only. Opportunity persistence increments only while the normalized threshold is met and otherwise resets to zero. Future spawn tanks must be withdrawn atomically from protected market surplus; retirement must defer until reservations, cargo, transit, and tank energy are reconciled. Protected liquidation budgets are compiled from validated trader/archetype capabilities rather than a fleet-size multiplier, so count changes cannot weaken the guarantee.

All four investment kinds—collector, storage, population support, and route subsidy—share a typed shape: enabled state, checked base cost, multiplicative cost growth, maximum level, cooldown, and effect per level. Cost level `n` repeatedly applies checked ceiling multiplication by the authored growth percentage. Allocation percentages are unique and total at most 100%. Investments are disabled by default and, when enabled later, may spend only the same stage-aware unreserved purchasing pool. Emergency suppression has final authority over subsidy advertising and spending. Governance is separate typed authority state. Public/player policy commands require the sole player's stable ID to match the market's authored player authority; autonomous/unowned markets reject before policy or protection mutation. Future internal AI execution remains a separate core application path.

Aggregate simulation history, not formatted UI history, owns stage occupancy/transitions and future population, fleet, and investment outcomes. Event labels and immutable snapshots are presentation projections. Long-run diagnostics must pass exact physical-energy reconciliation, retain nonzero final-window activity, and report stage occupancy/transition evidence; intervention diagnostics must start from identical content and seed, explicitly account for external inflow, and identify the first bounded stage or population divergence.

## Tick phases

The headless core executes explicit deterministic phases for the current brownout checkpoint:

1. complete travel and refresh/expire reservation state;
2. generate the configured base output, cap storage, and record curtailment;
3. assess mandatory life support;
4. classify the brownout stage and derive the effective operating profile;
5. execute sources and recipes with composed stage/labor throughput and operating energy;
6. settle arrivals and funded liquidation fallback;
7. collect and resolve automated commitments in stable order;
8. buy cargo, depart, advance the clock, and publish events/snapshots.

ECS iteration order is never used to choose contention winners.

## Content and validation

- `goods.ron`: `core:energy`, category, and energy-denominated `bootstrap_cost`.
- `recipes.ron`: operating energy, output quantities, and allocation weights.
- `economy_config.ron`: pricing mode, policy defaults, reservation TTL, life-support rate, and diagnostic comparison controls.
- `economy.ron`: starting energy, solar/collector inputs, storage, population, sources, risk acknowledgement, and policy overrides.
- `traders.ron`: tank stock/capacity, cargo capacity, speed, travel burn, and an authored physical-transfer refuel policy (`DepositAndWithdraw`, `DepositOnly`, or `Disabled`) compiled into each runtime trader. Withdrawals retain reservation claims, operating reserve, and protected liquidation budget; deposits retain storage headroom. Either direction is one physical transfer.

Validation checks IDs and references, exact energy identity/cost 1, checked ranges and generation, positive capacities/weights, stock caps, policy merging, exporter/importer/knife-edge roles, solar/resource anti-correlation, liquidation feasibility, and graph-aware importer runway. It verifies exporter surplus and available stock after protection, energy-cargo purchase affordability from the trader tank, exporter storage headroom, per-leg route burn, cargo delivery capacity, settlement tank headroom, and scheduling time.

Importer runway must exceed plausible first-delivery time plus one scheduling tick. It fails by default. `acknowledge_bootstrap_risk: true` downgrades only that failure to a structured source/system warning surfaced by content validation and diagnostics.

## Observability and reconciliation

Snapshots and the TUI distinguish market stock/cap, active claims, operating reserve, protected liquidation budget, unreserved purchasing energy, canonical effective advertised/funded demand, health/deficit, trader tank/cap, bay energy, cargo capacity, and runway. Frontends and diagnostics consume the core demand projection rather than reconstructing funding.

The energy ledger separately tracks generation, life support, source/production/travel burn, curtailment, market↔tank and market↔energy-cargo transfers, and unsupplied life support. Per-market counters remain checked `Energy`; global reporting aggregates exactly in wider `i128` counters and never saturates or clamps. Reservation claim changes are non-physical and excluded. Diagnostics reconcile:

```text
initial physical stock + generated − burned − curtailed = final physical stock
```

The CLI supports identical-seed scarcity versus cost-aware A/B runs and reports interval activity, stationary-laden traders, realized processor input cost, operating energy, output revenue, realized margin, structural processor solvency, reserves, claims, and storage. It displays market↔tank and market↔energy-cargo transfer dimensions separately while excluding those internal transfers from exact external-flow reconciliation.
