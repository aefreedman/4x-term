---
title: "Energy and Seasons"
type: design
status: approved
source: "../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md"
---
# Energy and Seasons

Energy is a physical system stock. It is produced by Collectors, consumed by
ordered simulation activities and explicit commands, and retained only after all
other tick activity has completed. It is not currency and is not generated as a
body deposit.

## Physical inputs

Collector production combines three map/time inputs:

- **system strength** controls the Collector's complete ten-phase Energy budget;
- **body eccentricity** redistributes that fixed budget among phases; and
- the current global simulation tick selects the seasonal phase.

Strength and eccentricity are immutable physical map properties, not development
state. Every Collector on a body uses that body's eccentricity and its system's
strength.

The origin has strength `1.00`, and all origin bodies have eccentricity `1.00`.
Frontier strength is represented in hundredths in `0.10..=3.00`; frontier body
eccentricity is represented in hundredths in `0.00..=1.50`.

System, body, stock, and resource ownership are defined in
[Systems and Resources](systems-and-resources.md).

## Ten-phase seasonal curve

All bodies share the normalized ten-phase shape derived from:

```text
[40, 40, 30, 20, 10, 10, 20, 30, 40, 40]
```

Its baseline average is `28`, and its baseline complete-cycle total is `280`.
For a phase shape value `p`, `phase_multiplier = p / 28`. Eccentricity then
scales that phase's deviation from the baseline:

```text
seasonal_multiplier = 1 + eccentricity × (phase_multiplier - 1)
```

The consequences are deliberate:

- eccentricity `0.00` removes seasonal variation;
- eccentricity `1.00` applies the standard curve;
- eccentricity `1.50` amplifies it while keeping phase weights nonnegative; and
- changing eccentricity never changes complete-cycle output.

## Integer production contract

Strength determines each Collector's integer complete-cycle budget from
`280 × strength`. The exact fixed-point result rounds up only when its
fractional part is at least `0.8`; otherwise it rounds down. Fractions do not
carry into later cycles.

Eccentricity redistributes that integer budget across the ten phases.
Deterministic largest-remainder apportionment turns exact phase shares into
integer Energy, with ascending phase order breaking equal remainders. Therefore:

- complete-cycle production depends only on strength;
- seasonal timing depends on eccentricity;
- every phase produces an integer quantity;
- the phase quantities sum exactly to the fixed cycle budget; and
- there is no hidden fractional production account.

## Tick spending order

Collector output is produced before operational and construction spending. The
Energy-relevant order within a tick is:

1. Collector production;
2. life support and supported-population work derivation;
3. Extractor operation;
4. Refinery operation;
5. the head project of each Shipyard queue;
6. enabled empty-Habitat population-generation accumulation;
7. general construction work;
8. ship movement and arrival; and
9. Energy retention and overflow.

The complete cross-system phase order and activation rules are in
[Simulation Timing](simulation-timing.md).

Within the Habitat phase, eligible Habitats consume in stable body/slot order.
Each takes as much available Energy as possible up to its remaining generation
cost, so earlier Habitats can consume all Energy. Habitat progress persists when
generation is disabled and is never refunded; see
[Population and Habitats](population-and-habitats.md).

A Shipyard project advances exactly one step only when its complete authored
per-tick Energy requirement is available. Otherwise it consumes no Energy and
pauses. Idle Shipyards and Habitats have no additional upkeep under the approved
contract.

## Command-time Energy

Commands execute only between ticks. Launching a probe or expedition spends its
complete route Energy atomically from the source system and records that spend
in source accounting. Travel carries no Energy balance and cannot later request
more.

For a fixed route:

```text
leg_distance       = ceil_sqrt(squared_coordinate_distance)
total_route_distance = sum(leg_distance)
launch_energy      = ceil(total_route_distance × ship_energy_rate)
```

Probe and expedition Energy rates are separately authored tuning. A command
fails without mutation if distance, cost, stock, reference, or other validation
fails.

Construction and Shipyard enqueue costs are physical material commitments, not
operational Energy spending. Shipyard per-progress-tick Energy and ship launch
Energy remain separate expenditures from those commitments.

## Retention, receipts, and overflow

Energy retention and overflow resolve after movement and settlement. Founding
Energy received by an expedition is therefore subject to target-system retention
on its arrival tick. Receipts use checked capacity handling and produce explicit
overflow accounting rather than silently discarding excess stock.

Energy accounting distinguishes production, available stock, construction or
project commitment, operational spending, travel spending, arrival receipt, and
overflow. Tick failure leaves every one of those balances and records unchanged.

## Authored tuning boundary

Seasonal shape, recipes, capacities, upkeep, action costs, project Energy,
travel rates, and other balance values are required validated world-level
content. The engine does not supply fallback balance constants. The fixed
mechanical contracts on this page are the relationship between those values,
the deterministic integer rules, and the order in which Energy is allocated.

## Related design

- [Systems and Resources](systems-and-resources.md)
- [Population and Habitats](population-and-habitats.md)
- [Simulation Timing](simulation-timing.md)
