---
title: "Simulation Timing"
type: design
status: approved
source: "../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md"
---
# Simulation Timing

The simulation has one world-owned clock and a phase-major global tick. Commands
are transactions between ticks; a tick is one atomic transaction across the
entire world.

## Global clock and ordering

`WorldState` owns one `SimulationTime`. A tick reads the current
`SimulationTime.tick` for seasonal phase and all event metadata, executes all ten
phases, then increments the clock with checked arithmetic.

Each numbered phase runs across every applicable system in stable system-ID
order before the next phase begins. Additional stable ordering is part of the
mechanic:

- developments use stable body/slot order;
- each Shipyard owns an independent FIFO queue;
- ships use stable ship-ID order for same-tick movement, arrival, observation,
  and loss; and
- equal-cost routes break ties by lexicographically ordered stable system-ID
  sequence.

These orders resolve contention without depending on collection or ECS
iteration order.

## Tick phases

A tick executes exactly:

1. finalize Habitat population generation that became ready on the prior tick;
2. Collector production;
3. life support and supported-population work derivation;
4. Extractor operation;
5. Refinery operation;
6. Shipyard project progress;
7. enabled empty-Habitat Energy accumulation;
8. general construction work;
9. ship movement, stop/arrival/loss resolution, observation creation, and
   receipt of transmissions due on the current tick; and
10. Energy retention and overflow.

After phase 10, `SimulationTime.tick` increments. The Energy implications of
this order are detailed in [Energy and Seasons](energy-and-seasons.md), and the
Habitat/population transitions are detailed in
[Population and Habitats](population-and-habitats.md).

## Command boundary and activation

Commands, including construction orders and ship launch, occur only between
ticks. They validate and commit atomically against current authoritative state.

Timing follows these boundaries:

- a Habitat marked ready during phase 7 can finalize only at phase 1 of a later
  tick;
- a ship launched between ticks takes its first movement step in phase 9 of the
  next tick;
- a Shipyard asset completed in phase 6 can be launched only after its
  completion tick;
- newly constructed Shipyards and Habitats first operate on the following tick;
  and
- newly arrived Collectors and population first operate on the following tick.

Founding stocks arrive during phase 9. Their Energy participates in retention
and overflow during phase 10 of that same tick.

## Shipyard progress

Each functional Shipyard owns its own FIFO project queue and committed project
resources. Only the head project is eligible to advance. It advances exactly one
step in phase 6 when the complete authored per-tick Energy requirement is
available; otherwise it consumes nothing and remains paused. Project duration is
therefore a count of successful progress ticks, not elapsed wall-clock ticks.

A project may be cancelled for a complete atomic refund only before its first
progress step. Completed probes and expedition ships leave the queue as
system-owned assets at the Shipyard's system.

## Movement duration

Routes are fixed at launch. Each in-transit ship advances at most one tick of its
current leg per movement phase.

Distance and duration use checked fixed-point arithmetic:

```text
jump eligible  iff squared_distance <= squared_jump_range
leg_distance      = ceil_sqrt(squared_distance)
leg_duration      = ceil(leg_distance / ship_speed)
route_duration    = sum(leg_duration)
launch_energy     = ceil(sum(leg_distance) × ship_energy_rate)
communication_delay = ceil(direct_distance_to_origin × communication_rate)
```

Routefinding minimizes the sum of integer leg distances. A ship that reduces its
remaining leg time to zero reaches the stop and records its observation in that
movement phase. If another leg remains, it begins on the following tick. There
is no extra hidden stop delay, so total travel time is exactly the sum of leg
durations.

Probe and expedition jump limits, speeds, and Energy rates are separate authored
values. Travel has no random failure. After launch, a Stage 4b ship cannot be
cancelled, recalled, or retargeted.

## Arrival and same-tick observation

At a final expedition stop, settlement or deterministic loss resolves first.
The ship then creates its complete visited-system observation, so the dynamic
inhabited fact describes the post-arrival state. The resulting transmission
survives ship consumption or expedition loss and carries the typed mission
outcome. Origin-facing mission state remains `AwaitingOutcome`, and successful
remote command access remains locked, until receipt.

Every probe or expedition stop uses the current tick as `tick_observed` and
creates a pending transmission to the origin knowledge store. At observation
time:

```text
tick_received = tick_observed + communication_delay
```

A zero-delay transmission is received in the same movement phase. A positive-
delay transmission is received in phase 9 of exactly its due tick. Stable
transmission IDs make duplicate receipt idempotent. Receipt validates the whole
transmission before applying all fact merges atomically.

## Atomic tick contract

Tick advancement computes and validates a complete candidate world before
commit. A failure in any phase leaves all of the following unchanged:

- every system and community;
- body-resource quantities and stocks;
- queues, projects, assets, and reservations;
- population tokens and in-transit ships;
- knowledge facts and pending transmissions;
- accounting ledgers and loss evidence;
- monotonic counters and stable IDs; and
- `SimulationTime` itself.

Arithmetic and reference errors are therefore world-transaction failures, not
partial local failures. The same validate-before-mutate rule applies to commands:
a rejected command does not commit resources, population changes, reservations,
sequence IDs, or accounting entries.

## Stable identity and contention

Projects, ships, population, transmissions, and reservations use typed stable
domain IDs rather than ECS entities.

- `ProjectId` and `ShipId` contain the building system ID plus a system-scoped
  monotonic sequence allocated at enqueue. Their types remain distinct even
  when the originating sequence is equal.
- `PopulationId` contains birth-system ID plus that system's never-reused
  population sequence.
- `TransmissionId` contains observer/ship ID plus that observer's never-reused
  observation sequence.
- Slot reservations identify a typed construction or expedition owner.

All counters and IDs are snapshot state, are globally validated for unique and
valid references, and are never reused after cancellation, completion,
movement, receipt, or loss. Counter overflow rejects the command or tick
atomically.

Stable ordering also distinguishes valid gameplay loss from invalid state. A
known-slot expedition reservation mismatch rejects the whole tick. An
unreserved arrival with only summary knowledge may instead fail settlement in
stable slot order and record the approved ship/payload/population loss.

## Accounting through time

Physical accounting follows ownership transfers, not only net stock changes:

```text
available stock
  -> project commitment
  -> hull/probe construction expenditure at completion
     + expedition payload moved into the completed asset
  -> in-transit expedition payload at launch
  -> arrival receipt OR explicit loss
```

Cancellation is the reverse transfer from an unstarted project commitment to
available stock under explicit overflow rules. Operational Shipyard Energy and
launch travel Energy are expenditures, not payload. Population generation and
removal are recorded explicitly; population movement changes token state rather
than creating or destroying a count.

The ledger distinguishes available stock, commitments, per-tick Energy spend,
completed and in-transit payload, travel spend, arrival receipt and overflow,
ship/payload/population loss, and population generation/removal. Checked
reconciliation applies to every path. Detailed resource ownership is in
[Systems and Resources](systems-and-resources.md).

## Related design

- [Systems and Resources](systems-and-resources.md)
- [Energy and Seasons](energy-and-seasons.md)
- [Population and Habitats](population-and-habitats.md)
