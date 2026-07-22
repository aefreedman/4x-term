---
title: "Population and Habitats"
type: design-current
status: approved
authority: normative
horizon: current
source: "../../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md"
---
# Population and Habitats

Population is represented by stable, world-owned tokens. A token is always
backed by one functional Habitat while resident or carried by one expedition
ship while in transit. Habitat occupancy, community population, system
population, and inhabited status are derived views, never additional mutable
owners.

## Population authority

The world's population-token registry is the sole mutable population authority.
Every live token is in exactly one tagged state:

```text
Resident  { community_id, habitat_id }
InTransit { ship_id }
```

A resident token references exactly one community and one functional Habitat.
Each Habitat may be referenced by at most one token. An in-transit token has no
stale community or Habitat reference.

`PopulationId` contains the token's birth-system ID and that system's
never-reused population sequence. IDs and counters persist in snapshots and are
not reused after movement, loss, or removal.

Each inhabited player system has one stable population-only community. The
revision-1 origin community begins at population zero; a generated target
community is created only on its first successful founding arrival. Coherent
authored Tier 1 scenarios may begin with resident tokens backed by functional
Habitats and communities; this does not change the revision-1 generated start.
Initial in-transit population requires a complete runtime-restoration contract
and is not an authored-world input in Stage 4b. Communities do not own stocks,
infrastructure, queues, projects, assets, or accounting—systems do. See
[Systems and Resources](systems-and-resources.md).

## Habitat capacity

Each functional Habitat provides capacity for exactly one resident population
token in its system. Capacity follows from the token-to-Habitat reference:
therefore derived population cannot exceed the number of functional Habitats.
There is no separately writable occupancy or population count.

If an applicable validated transition makes an occupied Habitat nonfunctional
or removes it, its token is immediately removed and the population loss is
recorded explicitly. Habitat damage, ruin, and removal commands are not part of
the approved bounded-expansion design.

## Automatic population generation

New Habitats default population generation to enabled. Only an empty Habitat may
have generation enabled or disabled. Disabling preserves accumulated progress;
consumed Energy is not refunded.

An enabled, empty Habitat automatically accumulates consumed Energy toward the
profile-authored generation cost. The active `starter` value is owned by
[`content/profiles/starter.ron`](../../../content/profiles/starter.ron). After
life support and Extractor/Refinery operation, eligible Habitats consume
available Energy in stable body/slot order,
each taking as much as possible up to its remaining cost. Earlier Habitats may
consume all available Energy unless the player disables them.

Reaching the full cost marks the Habitat **ready**; it does not create population
immediately. At the start of a following tick, a ready Habitat creates one stable
resident token only if it is still functional, empty, and enabled. The token is
assigned to that system's community and references the Habitat. Completed
generation leaves the Habitat with zero progress for any future replacement.

The generated origin starts with no Habitat and population zero. Habitat
generation is its population bootstrap rather than a starting population grant.
Energy allocation details are in [Energy and Seasons](energy-and-seasons.md).

## Work and commandability

Life support resolves before supported-population work is derived. Non-origin
systems derive construction work only from supported local resident population.
Designer-authored free origin construction work remains tied to the origin
location and allows the origin to remain commandable at population zero.

A founded remote system becomes directly commandable when the origin receives
its successful founding outcome and remains commandable while inhabited. At population
zero it retains player ownership, all physical state, and automatic simulation,
but rejects player commands until automatic Habitat generation or a later
arrival repopulates it. Neutral systems are not commandable.

## Expedition departure

A completed expedition asset carries founding stocks, one deployable functional
Collector, and capacity for one population token. Launch is explicit and occurs
between ticks.

At launch, the source selects the token in the first occupied Habitat by stable
body/slot order and atomically changes it from `Resident` to
`InTransit { ship_id }`. The vacated Habitat:

- becomes empty;
- keeps its generation-enabled setting;
- has zero generation progress following its prior completed population; and
- resumes ordinary replacement accumulation when enabled.

Launch also moves the expedition payload into transit and pays the full travel
Energy from the source. Any validation failure leaves the token, Habitat,
payload, source stocks, reservations, accounting, IDs, and counters unchanged.

## Targeting and slot reservation

An expedition may target a distinct system with `IdentifiedSummary` or
`Complete` knowledge when deterministic routefinding finds a nonempty route.
Anonymous or unknown systems cannot be targets.

Settlement requires two empty target slots: one for the expedition hull's
Habitat and one for its Collector. They may be on different bodies.

- With `Complete` target knowledge, launch names and reserves two specific empty
  slots after validating their authoritative current availability.
- With `IdentifiedSummary` knowledge, launch makes no reservation. Arrival uses
  the first two then-empty slots in stable body/slot order.

A valid reservation blocks construction and other expeditions. A reservation
mismatch is invalid simulation state and rejects the whole tick; only an
unreserved summary-knowledge arrival can take the approved insufficient-slot
loss path.

## Founding arrival and loss

Arrival resolves settlement or loss before the expedition records its visited-
system observation. The observation therefore reports inhabited status after
the outcome and survives consumption or loss of the ship.

On successful arrival, one atomic transition:

1. consumes the expedition ship;
2. transforms its hull into one functional Habitat in the selected slot;
3. installs its functional Collector in the other selected slot;
4. receives founding stocks with checked overflow accounting;
5. creates the target community if this is the first successful founding;
6. changes the carried token to
   `Resident { target_community_id, habitat_id }`; and
7. marks the target player-founded.

Founding creates no new population token. The arrived Habitat, Collector, and
population first operate on the following tick. Founding Energy is still subject
to retention and overflow on the arrival tick.

If an unreserved arrival finds fewer than two empty slots, the expedition ship,
population token, Collector, and founding stocks are lost with no refund. The
token is removed from the live registry, and all losses are recorded explicitly.
This is deterministic under-scouting risk, not random travel failure.

## Population accounting

Accounting records population generation, movement into and out of transit,
successful residence transfer, explicit expedition loss, and removal caused by
loss of Habitat support. Population tokens are moved between states rather than
copied; no community, Habitat, ship, or system stores a duplicate mutable count.

The phase boundaries for generation readiness, launch, movement, arrival, and
first operation are specified in [Simulation Timing](simulation-timing.md).

## Related design

- [Systems and Resources](systems-and-resources.md)
- [Energy and Seasons](energy-and-seasons.md)
- [Simulation Timing](simulation-timing.md)
