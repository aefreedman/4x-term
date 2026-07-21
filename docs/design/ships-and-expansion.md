---
title: "Ships and Expansion"
type: design
status: approved
source: "../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md"
---
# Ships and Expansion

Expansion is a bounded physical process: systems build ships, commit travel
Energy, move an existing population and founding payload, and either establish a
settlement or lose that expedition at its destination.

Prerequisite contracts:

- [Population and Habitats](population-and-habitats.md) owns population tokens,
  Habitat generation/capacity, departure, residence, and population loss.
- [Scouting and Knowledge](scouting-and-knowledge.md) owns knowledge levels,
  observations, reveal scans, transmissions, and fact merging.
- [World Generation](world-generation.md) owns positions, geometric route
  derivation, jump eligibility, distance, and route tie-breaking.
- [Simulation Timing](simulation-timing.md) owns phase order, movement timing,
  activation, stable IDs, atomicity, and physical reconciliation.
- [Tuning Profiles](tuning-profiles.md) owns all editable recipes, costs,
  durations, ranges, speeds, rates, and founding-stock values.

## Shipyards and projects

A functional Shipyard owns an independent FIFO project queue for probes and
expedition ships. Ship projects consume neither body slots nor population
construction work.

An explicit order atomically transfers the project's complete material cost from
system stock into a Shipyard-owned commitment. Only the queue head can advance.
It progresses once in a tick when its complete authored per-progress-tick Energy
is available; otherwise it consumes nothing and pauses.

A project may be cancelled for a complete refund only before its first progress
step. Completion records hull/probe construction expenditure and creates a
system-owned completed asset. A completed asset may launch only after its
completion tick. Shipyard removal/disable and queue orphaning are outside this
design.

`ProjectId` and `ShipId` are stable typed IDs based on building system and a
never-reused system sequence. Their lifecycle and counter rules are defined in
[Simulation Timing](simulation-timing.md#stable-identity-and-contention).

## Probe assets

A completed probe remains at its building system until explicit launch. Probe
launch, adjustable jump limit, route observation, reveal scans, delayed reports,
and final consumption follow [Scouting and Knowledge](scouting-and-knowledge.md#probes).
Probes carry no founding payload or population.

## Expedition commitment

An expedition project commits one indivisible package:

- the hull that will become a Habitat;
- fixed founding stocks; and
- one deployable functional Collector.

Cancellation before progress refunds the complete package. Completion accounts
for hull construction while keeping founding stocks and the Collector as payload
owned by the completed expedition asset. Launch moves that payload into transit;
arrival or loss transfers it exactly once.

The editable initial values are defined only in
[Tuning Profiles](tuning-profiles.md#construction-and-ship-projects).

## Expedition launch

An expedition may target a distinct `IdentifiedSummary` or `Complete` system
when routefinding finds a nonempty route within expedition jump range.
`Anonymous` and `Unknown` systems are not targetable. Probing the target is not
required, and a fixed route may contain redacted unidentified intermediate
systems.

Launch atomically:

1. selects the population token in the first occupied source Habitat by stable
   body/slot order;
2. changes it from `Resident` to `InTransit { ship_id }`;
3. vacates the source Habitat under the rules in
   [Population and Habitats](population-and-habitats.md#expedition-departure);
4. commits the fixed route and moves the expedition payload into transit; and
5. spends the complete distance-based travel Energy from source stock.

A launched expedition cannot be cancelled, recalled, or retargeted. Travel has
no random failure.

The transit record owns ship identity, source/target, fixed route and progress,
paid travel cost, payload, population-token ID, and target reservations. It never
contains a second population token or duplicate payload store.

## Knowledge and target-slot commitment

Founding requires two empty slots: one for the Habitat formed from the hull and
one for the Collector. They may be on different bodies.

- With `Complete` target knowledge, launch names and reserves two specific empty
  slots after authoritative validation. Typed expedition reservations block
  construction and other expeditions until arrival.
- With `IdentifiedSummary` knowledge, launch reserves no slots. Arrival chooses
  the first two then-empty slots in stable body/slot order.

A reservation mismatch is invalid simulation state and rejects the complete
tick. Only an unreserved summary-knowledge expedition can reach the approved
insufficient-slot gameplay loss.

## Founding resolution

Settlement/loss resolves before the expedition creates its final complete
observation. The delayed report records inhabited status plus a typed `Founded`
or `FoundingLost` outcome and survives ship consumption or loss. Origin-facing
mission state remains `AwaitingOutcome` until that report arrives; physical
success/loss is not leaked through immediate player-facing asset or ledger
state.

### Successful founding

When two applicable slots are available, one atomic transition:

- consumes the ship and converts its hull into a functional Habitat;
- installs the payload Collector in the other slot;
- receives founding stocks with checked Energy overflow accounting;
- creates the target's stable population-only community if needed;
- changes the carried token to
  `Resident { target_community_id, habitat_id }`; and
- marks the system player-founded.

The transition moves an existing population token; it never creates one.
Arrived Habitat, Collector, and population first operate on the following tick.
Founding Energy participates in retention/overflow on the arrival tick.

### Unreserved founding loss

If an unreserved arrival finds fewer than two empty slots, the ship, population
token, Collector, and founding stocks are lost without refund. The token leaves
the live registry and all losses are recorded explicitly. This is deterministic
under-scouting risk, not random travel failure.

## Ownership and control

Systems own physical stocks, developments, Shipyard queues/projects, completed
assets, and accounting. Population-only communities and token state follow
[Population and Habitats](population-and-habitats.md).

The origin is commandable at population zero. A founded remote system begins
physical automatic simulation on arrival and becomes directly commandable when
the origin receives its successful founding outcome. Once unlocked, it is
commandable without further delay while inhabited; at population zero it retains ownership and
automatic simulation but rejects player commands until repopulated. Neutral
systems are not commandable.

Cultural influence, delayed authority, delegation, ownership transfer,
automated freight, general logistics, reclamation, ship recall, and random travel
failure are outside this design.

## Related pages

- [Scouting and Knowledge](scouting-and-knowledge.md)
- [Population and Habitats](population-and-habitats.md)
- [Simulation Timing](simulation-timing.md)
- [Tuning Profiles](tuning-profiles.md)
