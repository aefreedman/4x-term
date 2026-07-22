---
title: "World Generation"
type: design-current
status: approved
authority: normative
horizon: current
design_ids:
  - worldgen.constructive-origin
  - testing.generated-world-invariants
---
# World Generation

Stage 4b defines a constructive origin inside an otherwise unconstrained procedural frontier. The origin is built from a dedicated parameter set; it is never selected from frontier output and there is no larger privileged start region.

See also:

- [Generator identity](generator-identity.md)
- [Frontier generator revision 1](generator-revision-1.md)
- [Tuning profiles](tuning-profiles.md)
- [Stage 4b implementation plan](../../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md)

## Constructive origin contract

Generation creates the mandatory origin scaffold first, then may add variation using origin-only parameters. Optional variation cannot replace or weaken the scaffold.

The origin guarantees:

- position `(0, 0, 0)`;
- system strength exactly `1.00`;
- eccentricity exactly `1.00` on every body;
- `4..=12` bodies, sampled from an exact discrete triangular distribution with mode `4`;
- `3..=8` generic construction slots on every body, sampled independently from an exact discrete triangular distribution with mode `3`;
- every naturally deposit-bearing material resource is present in a nonzero quantity on at least one body;
- exactly one starting functional Collector, in the first generated body's first slot;
- no starting Battery, Extractor, Refinery, or Habitat;
- profile-authored starting stocks; and
- an origin community at population `0`. The origin remains commandable at zero population.

The active `starter` stock values are owned by
[`content/profiles/starter.ron`](../../../content/profiles/starter.ron). Their
exact quantities are mutable tuning, not part of the structural guarantee.

Origin resource-bearing-body counts, per-body quantities, and placement use validated origin-specific distributions. Mandatory presence is not a universal quantity floor. A resource may occur on multiple origin bodies.

This scaffold is the complete constructive guarantee. It does **not** guarantee a favorable nearby resource, a neighborhood witness, adjacency, connectedness, solvency, affordability, long-run stability, or a per-seed economic surplus.

## Frontier system properties

Every non-origin system uses frontier parameters independently of the origin.

### Strength and seasonal output

Frontier system strength is represented in hundredths and sampled from a bounded triangular distribution over `0.10..=3.00` with mode `1.00`. It scales the complete-cycle output of every Collector in the system.

Each frontier body has an eccentricity represented in hundredths and sampled
from a bounded triangular distribution over `0.00..=1.50` with mode `1.00`.
Strength and eccentricity are immutable generated map properties. Strength
changes complete-cycle Collector production; eccentricity changes only its
phase distribution.

[Energy and Seasons](energy-and-seasons.md#ten-phase-seasonal-curve) owns the
profile-authored curve, scaling formula, fixed-point rounding, and deterministic
phase apportionment. World generation supplies the validated properties to that
mechanic rather than owning a second copy of its production contract.

### Bodies, slots, and material resources

Frontier body count follows an exact discrete triangular distribution over `1..=12` with mode `4`. Every frontier body independently receives `1..=8` slots from an exact discrete triangular distribution with mode `3`.

Natural deposit generation is explicit and resource-driven:

- each resource definition declares whether it is naturally deposit-bearing;
- Energy is not deposit-bearing because system strength and Collectors represent its physical source;
- each deposit-bearing resource independently rolls system presence using integer basis points in `0..=10_000`;
- a frontier system with no generated body resources is valid;
- when present, the configured resource-bearing-body-count triangle is truncated and renormalized to `1..=actual_body_count`; a sampled result is never clamped;
- distinct bodies are selected uniformly without replacement; and
- each selected body independently samples one nonzero quantity from that resource's configured bounded triangular distribution.

A body owns at most one quantity per material resource. Duplicate occurrences merge into that body/resource total. Body resources consume no slots and have no standalone deposit identity. System totals are derived views, not additional mutable quantities.

An Extractor occupies a slot and may target only a resource on its own body. Multiple Extractors may draw from the same total; stable body/slot order resolves same-tick contention.

## Spatial generation

Revision 1 creates a two-dimensional frontier while retaining three-coordinate positions: generated `z` is always `0`. Coordinates are validated fixed-point integers. Rectangular X/Y bounds are centered on the origin.

Target system count includes the origin but is a **density target**, not an exact output count. Let:

```text
target_non_origin = target_system_count - 1
weight_i          = raw_noise_i + 1
```

The bounds are divided into fixed-point cells. Seeded multi-octave integer
value noise gives every eligible cell an exact positive density weight. The
canonical byte encoding, random streams, triangular weights, noise interpolation,
cell domains, jitter, and generated-ID algorithm are frozen in
[Frontier Generator Revision 1](generator-revision-1.md). Each eligible cell independently places at most one system with exact rational probability:

```text
min(1, target_non_origin × weight_i / sum_of_all_eligible_weights)
```

The PRNG compares the rational value without floating point. A successful cell receives one deterministically jittered position. Probability mass lost when a cell caps at `1` is not redistributed, and generation never adds or removes systems afterward to force the target count.

The configuration must make the target representable by its cell/density model, but a result above or below the target is valid. Revision 1 has no explicit minimum system separation.

## Geometric reachability

There is no authored or generated edge graph. Reachability is derived at runtime from committed positions and the selected ship's maximum jump distance. Different jump limits can therefore produce different route graphs.

Distance and routing use checked fixed-point arithmetic:

- jump eligibility compares squared coordinate distance with squared jump range;
- a leg's distance is the checked ceiling integer square root of squared distance;
- routefinding minimizes the sum of leg distances;
- equal-cost routes are broken by lexicographic stable system-ID sequence;
- leg duration is `ceil(leg_distance / ship_speed)`;
- route duration is the sum of leg durations;
- launch Energy is `ceil(total_route_distance × ship_energy_rate)`; and
- communication delay is `ceil(direct_distance_to_origin × communication_rate)`.

Routefinding may use unidentified intermediate systems. Those systems remain hidden in player-facing route state until visited. Disconnected and unreachable regions are valid frontier texture.

## Generated initial knowledge

At tick `0`, the origin knowledge store receives:

- `IdentifiedSummary` information for every system within one probe-maximum jump of the origin; and
- anonymous existence-only indications for systems reachable in two or three probe-maximum-range geometric legs.

A summary includes body count, exact strength, per-body slot counts, and system-level material-resource presence. Present resources are categorized as `Poor`, `Normal`, or `Rich` from aggregate initial quantity. It does not reveal body resource locations, body eccentricity, or exact quantity. Anonymous indications do not identify a target and are not targetable. More distant systems begin `Unknown`.

Intermediate systems used to derive this knowledge do not create any neighborhood guarantee.

## Ownership of generated facts

Map definitions own immutable physical facts: system identity and position, strength, ordered bodies, eccentricities, ordered slots, and initial body-resource quantities. Runtime state owns remaining resource quantities, stocks, developments, queues, projects, assets, accounting, population tokens, and travel.

Every generated system has runtime resource state keyed to its map definition. Neutral systems begin without control, population, stocks, developments, projects, or ships, but their body-resource depletion state still exists. Habitat occupancy, system population, and aggregate system resources are derived views rather than duplicate mutable authorities.

## Valid outcomes and non-goals

Generation validates deterministic construction, configuration, arithmetic, references, and named structural invariants. It does not reroll, reject, screen, or tune worlds based on gameplay quality. Difficult distributions, local collapse, and isolated regions are valid unless they violate a named invariant or the constructive origin scaffold.

`ReclaimableSiteDefinition` has no generation requirement. Reclaimable-site placement remains a separate future design decision.
