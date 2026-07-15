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

Each system has a compiled base generation rate and an integer triangle-wave definition with amplitude, period, and phase. A nonzero amplitude requires an even period so the sampled wave reaches exact trough and crest extrema; fixed-output zero-amplitude definitions may use any valid period of at least two ticks. The pure waveform is periodic, bounded to non-negative output, and uses checked integer arithmetic only. Amplitude zero returns the base rate exactly without evaluating phase arithmetic. Effective output is derived from the mutable base before life support on every tick; collector investment will therefore change the base while seasons continue to derive from it. Snapshots expose base/effective output, integer phase, trend, and the next turning point. Exactly three repository systems have nonzero amplitudes; all others retain exact fixed-output compatibility.

### Population, fleet, investment, and governance

Population is explicit runtime state with current/reference population, a supply-history carrying cap, an authored support cap, a bounded oldest-to-newest sufficiency window and sum, separate checked growth/decline remainders, trend/counter history, and tier milestones. Each post-trade tick records the minimum 0–100 sufficiency of mandatory energy delivery and authored essential/tertiary goods. The `VecDeque` moving window evicts deterministically and content cannot request more than 10,000 samples, bounding memory and per-tick history work. Its average scales the support cap; growth requires a full window, a Normal stage, and the authored average gate.

Starvation decline uses `floor((population × decline_rate + remainder) / 1000)`. Recovery is five to ten times slower and uses checked logistic arithmetic,

```text
delta = floor((population × (cap − population) × growth_rate + remainder)
              / (cap × 1000 × scale))
```

Both paths retain their own remainder. Logistic growth stores the denominator paired with its remainder and atomically preserves the finer denominator when old and new capacity denominators divide exactly, scaling the new tick's numerator into that representation without rounding. Incompatible denominators convert with checked round-half-to-even rather than repeatedly flooring away tiny-population progress or directionally advancing it. An old remainder is therefore never interpreted against an unrelated denominator. Constructed states validate the pair before simulation starts. Property-style deterministic tests compare intermittent cap sequences with an exact rational reference, bound cumulative error to one population unit, reject discontinuous jumps, and preserve tiny-population progress. Growth is capped at `cap − population`, and zero population never regrows without future migration. A settled change occurs after all trade/fleet work, so the new population first affects the next tick's life-support obligation. Labor is `min(100, floor(current × 100 / reference))`; stage and labor percentages pass through the one final-carry throughput helper. Authored tertiary targets scale by current/reference population (or by the configured per-thousand rate when no base target exists) and also take effect next tick.

Fleet configuration is explicitly `Fixed` or `Dynamic`. Fixed mode is a strict lifecycle bypass: trader count is stable and profitability, persistence, cooldown, spawn, retirement, and lifecycle events do not mutate. Production content uses Dynamic with an authored initial/max count, opportunity threshold/window, spawn cooldown, and retirement threshold/window.

Dynamic opportunity uses the canonical automated-request net-margin-after-route-burn score. Unique profitable market/good/destination routes are sorted stably; one highest route is considered served per idle NPC, and the remaining score is normalized by system count. Persistence increments on consecutive ticks at or above threshold and resets immediately below it. A spawn occurs only after the full window and cooldown, at the highest stage-aware unreserved-surplus market with stable system-ID ties. Its authored starting tank is withdrawn atomically from that market, and its monotonic `_dynamic_########` ID is never reused. Because lifecycle evaluation follows request resolution, the new trader first becomes eligible on the next tick.

Every dynamic NPC retains a bounded rolling window of realized sales minus purchases and route burn, plus failed liquidation state. Sustained unprofitability or repeated inability to clean up marks retirement without despawning. Retirement releases an active reservation once, skips new automated work, and defers through transit and cargo cleanup. Laden cleanup explicitly reuses Slice 1's worthless-cargo resolution: every permitted cargo good has a validated positive liquidation reference, and the protected anti-strand budget guarantees that any laden retirement candidate can sell the funded whole-unit sub-quantity needed for the cheapest adjacent jump. Remaining cargo follows deterministic rerouting until it can settle or liquidate; it is never abandoned as worthless. An entity is removed only when idle, empty, reservation-free, and able to return its entire tank to local storage atomically. A full market defers the return. Protected liquidation budgets are compiled and recomputed from configured player/archetype capabilities rather than active NPC count, so spawning and retirement cannot weaken the guarantee. The focused `laden_sustained_unprofitable_trader_uses_anti_strand_cleanup_and_retires` test derives retirement from the profitability window, observes liquidation and claim cleanup, and requires despawn within a bounded tick loop without any stationary-laden terminal state.

All four investment kinds—collector, storage, population support, and route subsidy—share a typed shape: enabled state, checked base cost, multiplicative cost growth, maximum level, cooldown, and effect per level. Cost level `n` repeatedly applies checked ceiling multiplication by the authored growth percentage. Allocation percentages are unique and total at most 100%. On each tick, one common stable-ID-ordered executor derives each market's spendable amount from the stage-aware unreserved purchasing pool. Eligible kinds rank by descending allocation with `InvestmentKind` as the stable tie-break. The executor prepares the complete stock, level, cooldown, effect, history, ledger, and event result before mutation, burns exactly the selected checked cost, and completes at most one investment per market per tick. Disabled, stage-blocked, unallocated, cooling-down, maximum-level, insufficient-funds, ready, and completed states are typed and visible without deferred-event log spam.

Collectors add to the mutable seasonal base output, so the existing triangle wave derives its next effective output from the upgraded base. Storage adds cap without changing stock. Population support raises the support-cap input used by bounded supply-history carrying capacity and a per-market bonus to the approved logistic growth rate; it does not directly add population or bypass the long-average, Normal-stage, or carrying-cap growth gates.

For an allowed non-survival good, route subsidy uses the exact checked-integer formula below, where `ceil_ratio(x, p, 100) = ceil(x × p / 100)`, `priority` is the import-priority percentage, `effect` is the configured premium percentage per level, and `processor_ceiling` is present for cost-aware recipe inputs:

```text
normal_bid     = max(1, min(ceil_ratio(ask, priority, 100), processor_ceiling))
premium_pct    = 100 + effect × subsidy_level
subsidized_bid = min(ceil_ratio(normal_bid, premium_pct, 100), processor_ceiling)
```

When no processor ceiling applies, the corresponding `min` is omitted. The ceiling is a solvency bound, not the unsubsidized quote: whenever headroom exists, subsidy raises the bid strictly above `normal_bid`; it only clamps at the maximum input cost recoverable from recipe output revenue. The focused `route_subsidy_raises_solvent_bid_and_canonical_dynamic_backlog` test proves both the strict increase and the bound, then proves the higher canonical quote increases normalized unserved opportunity seen by fleet routing. The premium is not a second treasury: advertised quantity, funded quantity, reservation, and final payout all use canonical market settlement and therefore retain claims, operating reserve, protected liquidation, tank headroom, and physical-energy accounting. It never changes the energy/survival bid, so it cannot bypass the emergency energy ceiling. Emergency and Starvation suppression runs first and returns zero advertised/funded non-survival demand regardless of subsidy level. The authorization remains configured, and the same premium automatically resumes when recovery permits non-survival demand.

Governance is separate typed authority state. Repository content grants the sole player one starting market and gives every market a default AI investment allocation. Player policy and allocation commands require the player's stable ID to match that market authority; autonomous/unowned markets reject with typed governor feedback before policy, protected budget, or allocation mutation. Both player-configured and AI-defaulted markets then execute through the same autonomous investment phase. Immutable app/TUI views expose reserve horizon, margin, import priorities, allocations, levels, next costs, cooldown/status, subsidy state, population tier, and per-market ladder history. TUI controls submit typed configuration requests only; there is no direct ECS access or per-tick investment button.

Aggregate simulation history, not formatted UI history, owns stage occupancy/transitions and future population, fleet, and investment outcomes. Event labels and immutable snapshots are presentation projections. Long-run diagnostics must pass exact physical-energy reconciliation, retain nonzero final-window activity, and report stage occupancy/transition evidence; intervention diagnostics must start from identical content and seed, explicitly account for external inflow, and identify the first bounded stage or population divergence.

## Tick phases

The headless core executes these explicit deterministic phases:

1. complete travel, including contract arrivals at a source, destination, or recovery source;
2. refresh and expire ordinary reservation state;
3. derive seasonal output, generate Energy, cap storage, record ordinary curtailment, and assess mandatory life support;
4. classify the brownout stage and derive the effective operating profile;
5. execute sources and recipes with composed stage/labor throughput and operating Energy;
6. maintain pre-load Energy contracts in ascending contract-ID order, revoking unsafe claims or atomically loading arrived carriers;
7. settle, retry, or time out Energy deliveries and settle recovery arrivals in their stable destination/source orders;
8. settle ordinary laden arrivals and liquidation fallback, then rebalance idle NPC tanks;
9. execute one rate-limited autonomous investment per eligible market from protected surplus;
10. derive Energy offers/requests and collect one canonical positive-profit opportunity for each idle NPC across Energy contracts and ordinary trade;
11. resolve selected Energy intents by severity, runway, payload, and stable IDs, rejecting stale exact payloads without downsizing;
12. resolve ordinary trade intents for carriers that did not select Energy work;
13. evaluate dynamic-fleet profitability, deferred retirement cleanup, opportunity persistence, and at most one funded spawn for next-tick eligibility;
14. record bounded Energy/goods sufficiency, settle population decline/growth, labor, tertiary demand, tiers, and milestones for next-tick effect;
15. advance the clock and publish events/snapshots.

ECS iteration order is never used to choose contention winners.

## Content and validation

- `goods.ron`: `core:energy`, category, and energy-denominated `bootstrap_cost`.
- `recipes.ron`: operating energy, output quantities, and allocation weights.
- `economy_config.ron`: pricing mode, policy defaults, reservation TTL, life-support rate, brownout/population rules, all four investment shapes, default AI allocations, and diagnostic controls.
- `economy.ron`: starting energy, solar/collector inputs, storage, population, sources, risk acknowledgement, policy/allocation overrides, and optional starting governor.
- `traders.ron`: tank stock/capacity, cargo capacity, speed, travel burn, and an authored physical-transfer refuel policy (`DepositAndWithdraw`, `DepositOnly`, or `Disabled`) compiled into each runtime trader. Withdrawals retain reservation claims, operating reserve, and protected liquidation budget; deposits retain storage headroom. Either direction is one physical transfer.

Validation checks IDs and references, exact energy identity/cost 1, checked ranges and generation, positive capacities/weights, stock caps, policy merging, exporter/importer/knife-edge roles, solar/resource anti-correlation, liquidation feasibility, and graph-aware importer runway. It verifies exporter surplus and available stock after protection, energy-cargo purchase affordability from the trader tank, exporter storage headroom, per-leg route burn, cargo delivery capacity, settlement tank headroom, and scheduling time.

Importer runway must exceed plausible first-delivery time plus one scheduling tick. It fails by default. `acknowledge_bootstrap_risk: true` downgrades only that failure to a structured source/system warning surfaced by content validation and diagnostics.

## Observability and reconciliation

Snapshots and the TUI distinguish market stock/cap, active claims, operating reserve, protected liquidation budget, unreserved purchasing energy, canonical effective advertised/funded demand, health/deficit, trader tank/cap, bay energy, cargo capacity, and runway. Frontends and diagnostics consume the core demand projection rather than reconstructing funding.

The energy ledger separately tracks generation, life support, source/production/travel burn, curtailment, market↔tank and market↔energy-cargo transfers, and unsupplied life support. Per-market counters remain checked `Energy`; global reporting aggregates exactly in wider `i128` counters and never saturates or clamps. Reservation claim changes are non-physical and excluded. Diagnostics reconcile:

```text
initial physical stock + recorded external inflow + generated − life support − source burn − production burn − investment burn − travel burn − curtailed = final physical stock
```

The CLI supports identical-seed scarcity versus cost-aware A/B runs and reports interval activity, per-system net stock flow/storage/stage history, network stage percentages, stationary-laden traders, seasonal phase and cycle amplitudes, realized processor input cost, operating energy, output revenue, realized margin, structural processor solvency, reserves, claims, and storage. Diagnostics report active NPC fleet size, normalized unserved profitable opportunity per system, persistence, spawn/retirement totals, rolling profitability, cleanup state, and failed-liquidation ticks. They also expose population/trend/cap/tier, bounded recent sufficiency trajectory, settled changes/milestones, and aggregate stage-history score inputs. A 10,000-tick run is an explicit CLI acceptance path that enforces exact reconciliation, no system extinction or global collapse, no final aggregate ratchet below 90% of initial population, final-window trade and stage activity, a post-midpoint transition, a changed population stable over the final 100 ticks, and no final-window stationary-laden deadlock. Short deterministic tests compare system-only and trader-only insertion permutations using exact final population/stage maps, market ledgers, energy-flow/reconciliation outcomes, and aggregate dynamics; long insertion permutations are not part of routine tests. Typed spawn/retire events use resolved app-log trader and system labels. Diagnostics display market↔tank and market↔energy-cargo transfer dimensions separately while excluding those internal transfers from exact external-flow reconciliation.

`--player-impact` runs two identical deterministic sessions and applies exactly one typed `RecordExternalDelivery` to the intervention session. Target, delivery tick, good, quantity, and bounded horizon are explicit CLI inputs. The core validates the full delivery before mutation, emits one recorded-delivery event, and accounts energy deliveries as external inflow in exact reconciliation. The probe reports the first target stage/population divergence or fails when none occurs within the requested horizon.

### Enforced world-dynamics pre-merge gate

Any change to core economy scheduling/arithmetic, world-dynamics content, trader lifecycle, population, investments, or diagnostics is not merge-ready until the following pre-merge gate succeeds and its concise output is appended to `docs/world-dynamics-validation.md`:

```bash
cargo test -p game-content \
  tests::repository_energy_economy_remains_active_and_deterministic_for_1000_ticks \
  -- --ignored --exact --nocapture
cargo run -p game-cli --release -- --economy-diagnostics 10000
cargo run -p game-cli --release -- --player-impact \
  --impact-target frontier:system_04 --impact-tick 300 \
  --impact-good core:energy --impact-quantity 500 --impact-horizon 500
```

The 1,000-tick gate requires deterministic replay, continuing late trade/production, healthy fleet/cargo behavior, and exact reconciliation; it reports population changes but does not require one before the 500-tick sufficiency window and slower demographic response have had enough time. The 10,000-tick gate owns the stronger metastability criteria: no error/extinction/global ratchet, exact reconciliation, final-window trade and stage churn, a post-midpoint transition, and at least one stable changed population. The player-impact gate requires a bounded stage/population divergence with exact baseline and intervention reconciliation. These expensive checks remain outside the routine workspace suite but are an explicit pre-merge requirement, not an optional manual diagnostic.
