---
title: "Stage 4b: Constructive Frontier and Bounded Expansion"
type: feature
date: 2026-07-20
status: planned
---
# Stage 4b: Constructive Frontier and Bounded Expansion

## Purpose

Stage 4b implements the constructive-world and bounded-expansion intent handed off by the completed Stage 4 origin resource/infrastructure engine. This plan is the implementation contract; implementation must not re-open approved gameplay or preserve the replaced Stage 4 schema for compatibility.

Stage 4b owns:

- a generated origin system with its own validated parameter set, distinct from
  every generated frontier system;
- deterministic noise-shaped system positions, runtime-derived geometric jump
  routes, and optional procedural texture;
- generator configuration validation, versioning, seed, fingerprint, and provenance;
- the constructive G18 origin guarantee and unconstrained generated frontier;
- player information state with generated initial knowledge and probe reports;
- Shipyard, Habitat, probe, and expedition-ship construction;
- bounded ship travel and population movement from a source system to establish
  population at a target system; and
- the first multi-system expansion loop.

It does not own reclamation transitions, automated freight/logistics networks,
delegation policy, terminal UI, or full event-log replay beyond the bounded ship
and population-movement contracts approved here.

## Preserved design direction

- The origin system alone uses the player-start generation parameter set. Every
  other system uses frontier parameters; there is no larger start-area region.
- The origin is not an ordinary frontier system selected after generation.
- The origin uses a constructive hybrid: mandatory origin records are created
  first, then optional origin variation is generated from origin-only
  parameters. The frontier remains independently generated from frontier
  parameters.
- Tests exist to verify mechanics and expose bugs: deterministic construction,
  validated configuration, identity, references, arithmetic, and named
  structural invariants. Tests do not play or strategically evaluate generated
  worlds, classify seeds by gameplay quality, require favorable distributions,
  or use statistical desirability as acceptance.
- A difficult frontier, local collapse, unusual quantities, and unfavorable
  distributions are valid outcomes unless they violate a named engine invariant
  or an approved structural placement guarantee.
- G18 guarantees structural prerequisites demonstrated by implemented gameplay;
  it does not require a per-seed solvency inequality, surplus margin,
  affordability floor, long-run stability, or favorable distribution.
- A seed alone is not generation identity. Successful output records generator version, seed, validated configuration fingerprint, and source provenance.
- `ReclaimableSiteDefinition` remains unchanged until a later reclamation design. Whether Stage 4b must place one is an explicit future design decision, not an inherited requirement.

## Implemented Stage 4 handoff

Stage 4 established these structural inputs without making them generator
floors:

- a location is the stable system identity;
- each authored system may own persistent stocks and complete resource-engine
  prerequisites independently of community population;
- the current map-facing engine definition carries one ten-phase per-Collector
  Energy profile, stable bodies, ordered generic slots, and optional installed
  developments with condition and Extractor-deposit assignment;
- Stage 4b will replace the single system-wide seasonal interpretation with a
  generated system strength scalar and body-level eccentricity scalar while
  preserving deterministic phase output;
- deposits are currently standalone stable same-system records with exclusive
  Extractor assignment; Stage 4b will replace them with body-owned material
  resource quantities, allow multiple same-body Extractors to draw from one
  resource total, and retain derived system-level totals; and
- the demonstrated origin fixture has one body, six slots, one functional
  Collector, and one Ore deposit.

The fixture's `10 Energy/10 Ore/0 Alloy` stocks and Collector profile remain
approved origin content. The `200 Ore` fixture deposit, development recipes,
capacities, upkeep, and cycle values remain authored Tier 1 tuning and are not
universal generation requirements. Generated systems use the map/runtime split
specified below: every system has map records, while stocks, resource depletion,
developments, queues, projects, assets, accounting, world-owned population
tokens assigned to communities/Habitats or transit, and in-transit state are
runtime records. Gameplay tuning is world-level validated
configuration shared by systems rather than an optional copy embedded in each
neutral system.

`ReclaimableSiteDefinition` was not changed, and Stage 4 introduced no outward
command, generated positions/routes, generator identity, reroll, or quality-
screening surface. Stage 4b will migrate the current explicit edge topology to
runtime geometric reachability derived from positions and ship jump limits.

## Approved implementation contract

### B1 — Constructive origin guarantee

The gameplay origin uses a constructive hybrid. A mandatory origin scaffold is
created first, then optional origin variation is sampled from origin-only
parameters. The origin is never selected from ordinary frontier output.

The mandatory scaffold guarantees:

- system strength exactly `1.0`;
- body eccentricity exactly `1.0` for every origin body;
- `4..=12` bodies;
- `3..=8` generic construction slots on every body;
- a nonzero body-owned quantity for every deposit-bearing material resource,
  with that resource allowed on additional origin bodies;
- exactly one starting functional Collector in the first generated body's first
  slot, and no starting Battery, Extractor, or Refinery; and
- Stage 4 starting stocks unchanged at `10 Energy`, `10 Ore`, and `0 Alloy`.

Map definitions own system identity, strength, bodies, eccentricity, slots, and
each body's initial material-resource quantities. Runtime body-resource state
owns the corresponding remaining quantities. Stocks, developments, queues,
projects, assets, accounting, world-owned population tokens with tagged
resident/transit state, and travel are runtime state. Habitat occupancy and system
population are deterministic derived views of those tokens, never second mutable
owners. Only the origin uses origin generation
parameters; every other system uses frontier parameters. There is no mandatory
neighborhood witness, nearby-resource floor, adjacency guarantee, or connected-
component guarantee. The origin scaffold above is the complete constructive G18
guarantee for Stage 4b.

Origin-only body, slot, resource-bearing-body-count, resource quantity, and
placement distributions are required validated authored configuration within
the approved bounds. Resource presence is mandatory at the origin but does not
imply a universal quantity floor.

### B2 — Frontier catalog and distribution

Every frontier system has a **strength** scalar in the inclusive range
`0.1..=3.0`, sampled from a bounded triangular distribution with mode `1.0`. Strength represents
stellar Energy output and applies globally to Collector production on every body
in that system. Each body has a separate **eccentricity** scalar in `0.00..=1.50`, represented
in hundredths and sampled for frontier bodies from a bounded triangular
distribution with mode `1.00`. Origin-body eccentricity is always exactly
`1.00`. Eccentricity controls seasonal variation without changing a Collector's
complete-cycle output. `0.00` removes seasonal variation, `1.00` applies the
standard curve, and `1.50` amplifies it while retaining nonnegative phase
weights.

All bodies share the normalized ten-phase shape derived from the Stage 4 profile
`[40, 40, 30, 20, 10, 10, 20, 30, 40, 40]`, whose baseline average is `28` and
cycle total is `280`. Eccentricity linearly scales each phase's deviation from
that baseline:

`seasonal_multiplier = 1 + eccentricity × (phase_multiplier - 1)`

Strength, also represented in hundredths, determines each Collector's integer
ten-phase Energy budget from the baseline total `280 × strength`. The exact
fixed-point result rounds up only when its fractional part is at least `0.8`;
otherwise it rounds down. No fractional remainder carries between cycles.
Eccentricity redistributes that fixed budget across the ten phases. Deterministic largest-remainder apportionment converts exact phase
shares to integer Energy, using ascending phase order as the tie-breaker. Thus
strength controls complete-cycle output while eccentricity changes only its
phase-to-phase character. Strength, eccentricity, and phase are physical map
properties rather than development state.

Each resource definition explicitly declares whether it is naturally
deposit-bearing; generation never infers this property from the resource ID.
Every deposit-bearing material resource is independently eligible to be absent
or present on each frontier system, so a system with no generated body resources
is valid. When
present, the resource is assigned to one or more bodies. Each body directly owns
at most one remaining quantity for each material resource; there are no
standalone deposit IDs and duplicate occurrences on one body merge into that
single body/resource total. Initial quantity is sampled from a bounded
triangular distribution configured for that resource.

A body resource consumes no construction slot. An Extractor occupies a slot and
targets a resource total on its own body; cross-body extraction is invalid.
Multiple Extractors may target and draw from the same body/resource total, so
construction no longer reserves exclusive access to a deposit. Stable body/slot
order resolves same-tick contention when the remaining quantity cannot satisfy
all Extractors. Deterministic system-level totals aggregate body quantities for
reporting and do not duplicate mutable state. Energy is excluded from resource
presence because stellar strength and Collector output represent its physical
source. Frontier body count uses an exact discrete triangular
distribution over `1..=12` with mode `4`; origin body count uses the same family
over `4..=12` with mode `4`. Each frontier body independently receives slots
from a discrete triangular distribution over `1..=8` with mode `3`; each origin
body uses the same family over `3..=8` with mode `3`.

Origin and frontier generator configurations separately provide each
deposit-bearing resource's system-presence probability in integer basis points
`0..=10_000`, triangular resource-bearing-body count, and nonzero triangular
quantity distribution. Frontier presence rolls are independent by resource.
When a resource is present, its count distribution is truncated and
renormalized to `1..=actual_body_count`; the generator never clamps a sampled
result. That many distinct bodies are selected uniformly without replacement,
and one nonzero quantity is sampled independently for each selected body. The
origin replaces the presence roll with mandatory presence while using its
origin-specific count and quantity parameters.

Target system count is validated generator configuration and includes the
origin, but it is a density target rather than an exact output count. A generated
world with more or fewer systems is valid. The origin is fixed at `(0, 0, 0)`,
and configurable rectangular X/Y bounds are centered on it. Stage 4b generates a 2D frontier with `z = 0` while retaining the
existing three-coordinate position type. X/Y positions use validated fixed-point integer coordinates and are
sampled deterministically from a seeded fixed-point fractal-noise density field
so clusters and voids emerge without an authored edge graph. Generation divides
the bounds into fixed-point cells, evaluates seeded multi-octave integer value
noise into exact positive density weights `weight_i = raw_noise_i + 1`. For
`target_non_origin = target_system_count - 1`, each eligible cell independently
uses the exact rational placement probability
`min(1, target_non_origin × weight_i / sum_of_all_eligible_weights)`. The PRNG
compares that rational without floating point and places one deterministically
jittered system in each cell whose roll succeeds. Probability mass lost when a
cell caps at `1` is not redistributed, and generation does not add or remove
systems afterward to force the target count. Stage 4b defines no explicit minimum system separation; add one
only if concrete generation or presentation evidence requires it. Configuration validates that the target can be represented by the configured
cell/density probability model, but output-count deviation is never a generation
error. Generated systems need not form one
connected component. Runtime routes derive from geometric distance and the
selected ship's maximum jump distance. Deterministic routefinding may use
unidentified systems as intermediate stops, but those systems remain hidden in
player-facing route state until the ship reaches them. Disconnected or
unreachable regions are valid frontier texture rather than generation/test
failures.

The generator schema requires authored per-resource presence,
resource-bearing-body-count, and triangular-quantity parameters, plus target
system count, fixed-point coordinate resolution, 2D bounds, cell dimensions, and
validated integer noise/density/jitter parameters. These values have no engine
fallbacks. The implementation must publish the exact integer noise, cell
weighting, weighted sampling, jitter, and placement algorithms as part of
revision `core:frontier_world@1`; those algorithms are deterministic engineering
choices, not additional gameplay design gates. Validation and generation use
checked arithmetic throughout, and invalid configuration rejects before any
partial world is returned.

### B3 — Scouting, ships, habitats, and expansion

Stage 4b adds two developments:

- **Habitat:** each functional Habitat provides capacity for exactly one
  population in its system. New Habitats default population generation to
  enabled. An empty Habitat automatically accumulates consumed Energy toward the
  designer-authored `500 Energy` population-generation cost while generation is
  enabled. The player may enable or disable generation only while that Habitat
  is empty. Disabling retains already accumulated progress; consumed Energy is
  not refunded. After life support and existing Extractor/
  Refinery operation, enabled empty Habitats consume available Energy in stable
  body/slot order, each taking as much as possible up to its remaining cost.
  Earlier Habitats may therefore consume all available Energy unless the player
  disables them. A Habitat that reaches the complete cost is marked ready, then
  creates one stable population token in that system's community at the start
  of a following tick when the Habitat remains functional, empty, and enabled.
  The token references that Habitat. The world population-token registry is the sole mutable
  population authority; Habitat occupancy and system population are derived from
  those references. Habitat population generation is the
  Stage 4b origin bootstrap, so the generated origin starts with population `0`
  and no Habitat.
- **Shipyard:** a functional Shipyard owns an independent FIFO project queue for
  probes and expedition ships. Orders are explicit commands and atomically
  commit their complete designer-authored material cost at enqueue. One project
  advances one step per tick when its complete authored per-tick Energy
  requirement is available; otherwise it consumes nothing and pauses. Ship
  projects consume neither population construction work nor body slots. Each
  queue and its committed project resources belong to the specific Shipyard;
  completed ships leave the queue as system-owned assets stored at that
  Shipyard's system until launched. An order may be cancelled for a complete
  atomic refund only before its first progress step, using the existing explicit
  overflow rules. Removing or disabling a Shipyard is outside Stage 4b, so queue
  orphaning has no Stage 4b transition.

A new world starts at tick `0` with identified summary information observed and
received at tick `0` about every system within one probe-maximum jump of the
origin: body count, exact system strength, per-body
slot counts, and system-level material-resource presence. Each present resource
is labeled `Poor`, `Normal`, or `Rich` from the system's aggregate initial
generated quantity; initial information does not reveal which bodies hold it,
body eccentricity, or exact quantity. Systems reachable from the origin in two
or three probe-maximum-range geometric legs receive anonymous existence-only
indications. Intermediate systems used for this initialization need not satisfy
any constructive neighborhood guarantee. Knowledge levels are `Unknown`, `Anonymous`, `IdentifiedSummary`, and
`Complete`. Anonymous indications do not identify a target and are not
targetable. Initial adjacent information is `IdentifiedSummary`: it is
targetable but lacks exact slot identities/current availability, so expeditions
launch unreserved. `Complete` knowledge permits named slot selection and
reservation after current authoritative validation. More distant systems are
`Unknown`.

Probes are constructed at Shipyards and stored as system-owned assets. Probes
have a longer authored maximum jump distance than expedition ships, but each
probe launch accepts a desired jump limit no greater than that maximum so the
player can constrain route generation to expedition-capable legs. Launch is an
explicit command requiring an `IdentifiedSummary` or `Complete` target distinct
from the source and a nonempty route to it. The route is
fixed at launch and traversed leg by leg at an authored probe speed.
Routefinding may select unidentified intermediate systems from committed world
state; the player-facing route redacts them until the traveling ship reaches
each one. Any ship that reaches a visited system records a complete observation of that
system. Probes additionally apply their reveal-radius existence scan at every
stop; expeditions do not. The complete distance-based travel Energy
cost is spent atomically from the source system at launch and recorded in its
accounting; it is not an in-transit balance. A launched probe cannot be
cancelled, recalled, or retargeted, and Stage 4b adds no random travel failure.

At every intermediate stop and final target, a probe records exact map facts for
the visited system: strength; body identities and order; eccentricities; slot
identities and order; each body's initial generated and current remaining
material-resource quantities; and other map-owned properties. It also records
the dynamic inhabited/uninhabited fact. Resource depletion and inhabited status
are the only runtime facts in a detailed observation; observations do not reveal
population count, stocks, developments, queues, ships, support status, or other
runtime state. At each visited system the observer also records existence-only
facts for every system within the authored reveal radius. A probe is consumed
after observing its final target. Reveal radius is independent validated tuning,
not an alias for maximum jump distance.

Knowledge merges by independently keyed fact, never by replacing a whole system
record. A summary fact, exact map field, keyed body, keyed slot, keyed body-
resource quantity, and inhabited status each carry `tick_observed`, detail level,
and stable observer/ship ID; receipt adds `tick_received`. A lower-detail fact
never replaces a higher-detail fact. At the same detail, a newer observation tick wins; an equal-tick tie is won by
the lexicographically lower stable observer/ship ID regardless of receipt
iteration order. Higher detail may
replace a summary even when its observation is older because it adds rather than
removes facts. An existence-only or summary report therefore cannot erase exact
facts, and one stale dynamic fact cannot roll back another independently
observed field. Immutable exact map facts, once known, may only be repeated
identically; a contradiction rejects the entire receipt as invalid generated/
runtime state. `Poor`/`Normal`/`Rich` is a summary fact derived from initial
aggregate quantity; an exact resource observation supersedes that summary for
presentation without deleting unrelated facts.

Every probe/expedition stop observation uses the current simulation tick as
`tick_observed` and
creates an explicit pending transmission addressed to the origin knowledge
store. `tick_received` is checked at observation time as `tick_observed +
communication_delay`; zero-delay facts are received in the same movement phase,
and positive-delay facts are received in the movement phase of that exact tick.
Expedition observations follow the same rule. The origin knowledge store is the
sole Stage 4b information authority; founded remote systems do not own copies.
Every transmission has a stable ID; duplicate receipt is idempotent. Receipt
validates the complete transmission before applying all of its fact-level merges
atomically. Stage 4b retains no permanent report history beyond
current facts and pending transmissions. The `Poor`/`Normal`/`Rich` thresholds
are required ordered authored configuration.

Expedition ships are constructed at Shipyards, stored at their source system,
and launched explicitly over a fixed route whose legs do not exceed their
separately authored maximum jump distance. As with probes, routefinding may use
unidentified intermediate systems that remain hidden until reached. Their
complete distance-based travel Energy cost is spent atomically from the source
system at launch and recorded in its accounting. A launched expedition cannot
be cancelled, recalled, or retargeted, and has no random Stage 4b failure. Probe
and expedition ships have separately authored speeds and travel-Energy rates.

Expedition ships physically move one Habitat-referenced population token from a
source system to any distinct `IdentifiedSummary` or `Complete` target for which
deterministic routefinding finds a nonempty route;
probing is not required and unidentified intermediate stops may be redacted.
Each player-inhabited system has one stable population-only community. A single
world-owned population-token registry is the mutable authority, with every live
token in exactly one tagged state: `Resident { community_id, habitat_id }` or
`InTransit { ship_id }`. Resident state assigns a token to its community and
Habitat; in-transit state has no stale community/Habitat reference. Systems own
stocks, developments, queues, projects, assets, and accounting. Habitats own
generation enablement/progress but no duplicate occupant or population count.
Community population and Habitat occupancy are derived from resident token
states. The origin community exists at population zero; a target community is
created on first successful founding arrival.

The origin remains commandable at population zero. Every successfully founded
remote system is directly commandable without command delay while inhabited. A
remote system at population zero retains ownership, physical state, and
automatic simulation but rejects player commands until automatic Habitat
generation or a later arrival repopulates it. Neutral systems are not
commandable. Direct control exposes the current runtime state needed to
command an owned system; it does not create another scouting-knowledge store or
bypass delayed map observations. Designer-authored free origin construction work remains tied
only to the origin location; other systems derive construction work only from
supported local population tokens. Cultural influence, delayed authority,
delegation, ownership transfer, and loss of control remain outside Stage 4b.

Each expedition-ship project atomically commits the complete hull cost, fixed
founding stocks, and one deployable functional Collector at enqueue.
Cancellation before progress refunds that complete committed package. The
completed expedition asset owns its founding-stock store and Collector payload;
launch spends travel Energy and atomically changes the stable population token
in the first occupied Habitat by stable body/slot order from `Resident` to
`InTransit { ship_id }`. The vacated Habitat becomes empty, preserves its generation-enabled
state, and has zero generation progress after its prior completed population;
when enabled, automatic replacement generation resumes in the normal tick
phase. The in-transit snapshot
contains stable ship ID, kind, source, target, fixed route, current leg and
remaining leg ticks, jump limit, speed, paid travel cost, payload stocks,
Collector, population-token ID, and any reserved target slots. With `Complete` target knowledge, launch selects and reserves two specific empty
body slots after validating current authoritative availability:
one for the Habitat and one for the Collector. They may be on different bodies.
With `IdentifiedSummary` target knowledge, launch reserves no slots; arrival
selects the first two then-empty slots in stable body/slot order. `Anonymous` and
`Unknown` systems cannot be expedition targets. If
fewer than two remain, the ship, population token, Collector, and founding stocks
are lost with no refund. The token is removed from the live registry and the loss
is recorded explicitly. This is player-
authored under-scouting risk, not random failure.

Arrival resolves settlement/loss first, then emits the expedition's complete
visited-system observation so inhabited status reflects the post-arrival result;
the transmission survives ship consumption or loss. A successful arrival
consumes the expedition ship and transforms its hull into one functional Habitat in the selected slot, installs its functional Collector in
the other selected slot, deposits its founding stocks with checked receipt and
explicit overflow accounting, changes its carried population token from
`InTransit` to `Resident { target_community_id, habitat_id }`, and marks the
target system player-founded. It
creates no additional population token. Because each token references one
functional Habitat and each Habitat may be referenced by at most one token,
derived system population cannot exceed Habitat capacity. Stage 4b adds no damage/ruin/removal command for Habitats. If any applicable
validated state transition nevertheless makes an occupied Habitat nonfunctional
or removes it, the referencing population token is removed immediately and the
population loss is recorded explicitly.

A tick reads the current `SimulationTime.tick` for seasonal phase and all event
metadata, then executes:

1. finalize Habitat population generation that became ready on the prior tick;
2. Collector production;
3. life support and supported-population work derivation;
4. Extractor operation;
5. Refinery operation;
6. Shipyard project progress;
7. enabled empty-Habitat Energy accumulation;
8. general construction work;
9. ship movement and stop/arrival/loss resolution, observation creation, and
   transmissions whose `tick_received` equals the current tick; and
10. Energy retention/overflow, followed by checked increment of
    `SimulationTime.tick`.

Commands, including launch, occur only between ticks. A newly launched ship
executes its first movement step in phase 9 of the next tick. Each in-transit
ship advances at most one tick of its current leg per movement phase. Reaching a
stop at zero remaining leg ticks records the observation in that phase; a later
leg begins on the following tick. This produces the route duration defined
below with no hidden stop delay. Newly completed Shipyard assets can be launched
only after their completion tick. Newly constructed Shipyards/Habitats and
newly arrived Collectors/populations first operate on the following tick. Founding
Energy is subject to retention on its arrival tick. Stable ship ID orders
same-tick movement, arrival, observation, and loss processing; target slot
reservation and stable body/slot selection resolve arrival contention.

Distance and travel use checked fixed-point arithmetic:

- jump eligibility compares squared coordinate distance with squared jump range;
- a leg's integer distance unit is the checked ceiling integer square root of
  its squared distance;
- routefinding minimizes the sum of leg-distance units and breaks equal-cost
  paths by lexicographically ordered stable system-ID sequence;
- leg duration is `ceil(leg_distance / ship_speed)` and route duration is the
  sum of leg durations;
- launch Energy is `ceil(total_route_distance × ship_energy_rate)`; and
- communication delay is `ceil(direct_distance_to_origin × communication_rate)`.

Arithmetic/reference failures reject commands atomically before committing
resources, population tokens, slots, or sequence IDs; a tick arithmetic/reference
failure leaves all world, travel, knowledge, and accounting state unchanged.
Shipyard/Habitat construction recipes, Shipyard project material costs and
per-tick Energy/durations, founding stocks, ship jump limits/speeds/travel-Energy
rates, reveal radius, communication rate, and fixed-point coordinate resolution
are required validated authored configuration. The Habitat population-generation
cost is the already approved `500 Energy`. In-transit ownership, direct player
control, movement timing, and accounting are fixed above and are not deferred
design questions. Automated freight and general logistics remain later work.

### B4 — Runtime ownership, identity, accounting, and generator identity

`WorldState` owns one global `SimulationTime`. Every numbered tick phase runs
across all applicable systems in stable system-ID order before the next phase
begins; Shipyards use body/slot order and ships use stable ship-ID order within
their phases. Tick advancement validates a complete cloned world result before
commit, so a late failure leaves every system, community, transit record,
knowledge fact, pending transmission, reservation, counter, ledger, and the
clock unchanged.

Projects, completed ships, population tokens, transmissions, and reservations
use typed stable domain IDs independent of ECS entities. `ProjectId` and
`ShipId` contain building-system ID plus a system-scoped monotonic sequence
allocated at enqueue; their wrappers remain distinct even when they share the
same originating sequence. `PopulationId` contains birth-system ID plus that
system's never-reused population sequence. `TransmissionId` contains stable
observer/ship ID plus the observer's never-reused observation sequence, persisted
on the asset/in-transit record. All counters and IDs appear in snapshots, are
validated globally unique with valid references, and are never reused after
cancellation, completion, movement, receipt, or loss. Counter overflow rejects
atomically. Slot reservations carry a typed owner—
construction sequence or expedition ship ID—so unrelated numeric sequences
cannot collide. A known-slot expedition reservation blocks construction and
other expeditions until atomic arrival replaces it with occupancy. Reservation
mismatch is invalid state and rejects the complete tick; only an unreserved,
under-scouted arrival can reach the approved gameplay-loss path.

Physical accounting distinguishes:

- system available stocks;
- Shipyard project material commitments;
- per-tick Shipyard Energy spent;
- completed expedition payload stocks and deployable Collector;
- in-transit payload and population;
- launch travel Energy spent;
- arrival receipt and Energy overflow;
- ship/payload/population loss; and
- population generated and removed.

Enqueue transfers physical materials into project commitment rather than
silently destroying them. Cancellation returns the unchanged commitment.
Completion records hull/probe construction expenditure while moving expedition
founding stocks and the Collector into the completed ship asset. Launch moves
that payload into transit. Arrival transfers it once; failure moves it once to
explicit typed loss evidence. Checked reconciliation fixtures cover every path.

`game-content` owns strict generator-source parsing, canonical configuration
normalization, SHA-256 fingerprinting, seeded generation, and source provenance.
`game-core` owns format-independent generated definitions, validation, and
runtime. Compilation returns a generated-world artifact containing reproduction
identity, provenance metadata, and normalized `WorldDefinition`; provenance is
not an output-affecting fingerprint input.

Revision 1 uses a documented domain-separated SplitMix64 integer PRNG. Streams
are derived from seed, generator family/revision, stable stage tag, and canonical
ordinal so adding an unrelated draw does not silently perturb earlier stages.
Bounded random values use unbiased integer rejection sampling; weighted choices
use checked cumulative integer weights in canonical candidate order. This PRNG
rejection is internal unbiased number generation, not world rejection or
quality screening.

Generator identity contains:

- a structured version with `ContentId` family plus nonzero monotonic revision;
  the initial canonical version is `core:frontier_world@1`, represented as
  family `core:frontier_world` and revision `1`;
- an unsigned 64-bit seed;
- a SHA-256 fingerprint of the canonical normalized generator configuration;
- source-document provenance;
- stable generated IDs derived from deterministic generation order and kind,
  never random UUIDs or ECS IDs; and
- the complete normalized generated definition.

The fingerprint uses a versioned canonical byte encoding with fixed-width
integers, length-prefixed UTF-8, and canonical stable-ID/key order. Logical source
identity and source-content hash are provenance; machine-local paths are
excluded. Identical generator family/revision, seed, and configuration
fingerprint must produce an equal normalized `WorldDefinition`. Any output-affecting behavior
change increments the generator revision. A newer implementation need not
reproduce an older revision unless that revision is explicitly retained. Full
runtime event-log replay remains Stage 6. Tests use exact tiny mechanic fixtures, one deliberately tiny revision-1
determinism fixture, permutation-independent fingerprinting, domain-stream isolation, and identity mechanics rather than seed
quality or statistical outcomes.

## Implementation boundary and schema migration

Stage 4b is a hard schema replacement, not a compatibility layer:

- Replace floating map coordinates with the validated fixed-point three-
  coordinate representation used by generation, distance, and routing. `z`
  remains present and is zero for revision 1 output.
- Move strength, bodies, eccentricity, ordered slots, and initial body-resource
  quantities into always-present system map definitions. Move the Stage 4
  per-system Collector profile and embedded body/slot definitions out of the
  optional resource-engine record. World-level gameplay configuration owns the
  seasonal shape, recipes, capacities, upkeep, and action tuning.
- Give every generated system persistent runtime state keyed to its map
  definition. Neutral systems begin with no founded control, population tokens,
  stocks, developments, queues, projects, or ships, but they do own mutable
  remaining body-resource quantities. Founding activates ordinary system
  gameplay without constructing a second map or engine definition.
- Add explicit Habitat and Shipyard development roles. An Extractor definition
  and construction item target `(body_id, resource_id)`, not a deposit ID.
  Validate the target on the Extractor's body; do not reserve exclusive access.
  Multiple Extractors may target it, and stable body/slot order resolves draws.
- Remove `ResourceDepositDefinition`, deposit IDs, global deposit collections,
  Extractor-deposit fields/reservations, and their source/snapshot validation.
  Initial and remaining body-resource quantities are the only quantity
  authorities; system totals are derived on demand.
- Remove `TopologyDefinition`, `TopologyEdge`, stored adjacency, authored edge
  source fields, and edge snapshots. Route queries construct geometric
  reachability for the requested jump limit from committed fixed-point
  positions. No generated or runtime edge graph is another authority.
- Replace the single-community definition and writable population aggregate with
  one stable population-only community per inhabited system. Communities own
  stable population tokens referencing functional Habitats; snapshots expose
  those identities plus derived Habitat occupancy, system population, and
  inhabited status.
- Add system-owned completed ship assets; Shipyard-owned queues/projects;
  explicit in-transit ship/payload state; origin-owned knowledge and pending
  transmissions; founded/control state; and the accounting described in B3.
- Extend each resource definition with an explicit naturally-deposit-bearing
  declaration. Energy is configured false and generation rejects any attempt to
  generate it as a body resource.
- Retain `ReclaimableSiteDefinition` unchanged and behaviorless. Its presence is
  neither required nor generated unless authored configuration later provides
  a separate, approved placement contract.

Delete superseded parser fields, constructors, snapshots, tests, and fixtures
rather than supporting both schemas. Focused Stage 4 mechanism fixtures may be
rewritten against the replacement schema to preserve their approved gameplay
oracles. Generation identity starts at `core:frontier_world@1`; it provides no
compatibility promise for pre-generator authored worlds or removed topology and
deposit records.

## Authored tuning and designer access

All gameplay and generator tuning is strict RON content under
`content/profiles/`, compiled and validated by `game-content`. `game-core`
contains no balance constants or implicit profile defaults. The repository ships
an explicitly selected `starter` profile as an iteration baseline, not a
canonical world, preferred seed, or acceptance oracle. Designers can change that
profile without Rust changes; its normalized fingerprint changes, while the
generator revision changes only when generation algorithms change.

The initial `starter` gameplay tuning is:

| Item | Complete material commitment | Work/duration | Per-progress-tick Energy |
| --- | --- | ---: | ---: |
| Habitat | `40 Energy + 4 Alloy` | `8 work` | none |
| Shipyard | `80 Energy + 8 Alloy` | `12 work` | none while idle |
| Probe project | `20 Energy + 2 Alloy` | `4 ticks` | `10 Energy` |
| Expedition hull | `40 Energy + 6 Alloy` | `8 ticks` | `10 Energy` |

An expedition project additionally commits its deployable Collector using the
normal `10 Energy + 2 Alloy` recipe and founding stocks of `10 Energy + 10 Ore +
0 Alloy`. Its complete enqueue commitment is therefore `60 Energy + 10 Ore + 8
Alloy`; per-tick project Energy remains separate operational spending. Habitat
population generation costs `500 Energy`. Habitats and idle Shipyards have no
additional upkeep.

The initial `starter` travel/information tuning is:

- probe maximum jump `1_500` coordinate quanta;
- expedition maximum jump `1_000` quanta;
- probe speed `500` quanta per tick;
- expedition speed `250` quanta per tick;
- probe travel Energy `1` per `200` quanta;
- expedition travel Energy `1` per `100` quanta;
- probe reveal radius `1_500` quanta; and
- communication delay `1` tick per `500` quanta.

The initial `starter` generator profile is only one editable procedural profile:

- coordinate scale `100` quanta per map unit;
- target system count `128`, including the mandatory origin;
- centered X/Y bounds `-5_000..=5_000` quanta and generated `z = 0`;
- placement cells `500 × 500` quanta;
- four integer-noise octaves, base wavelength `4_000` quanta, lacunarity `2`,
  and persistence `1/2`;
- full-cell deterministic jitter;
- `core:ore` naturally deposit-bearing and `core:energy`/`core:alloy` not;
- origin Ore body-count triangle `1/2/4` and per-body quantity triangle
  `200/300/500`;
- frontier Ore presence `6_500` basis points, body-count triangle `1/1/4`, and
  per-body quantity triangle `50/200/500`; and
- Ore aggregate richness `Poor = 1..=199`, `Normal = 200..=499`, and
  `Rich = 500+`.

Triangle notation is `minimum/mode/maximum`. Other profiles and future resources
must explicitly provide the same validated fields. Tests use dedicated tiny
profiles and hand-computable fixtures rather than locking mutable `starter`
balance values. Acceptance verifies mechanics, deterministic reproduction,
ranges, references, and invariants; it never requires the actual count to equal
target system count or judges a generated distribution.

## Implementation outline

1. Replace the Stage 4 map, topology, deposit, and population schemas at the
   hard migration boundary above while preserving approved resource-engine
   mechanics in rewritten Tier 1 fixtures.
2. Add validated authored gameplay/generator configuration and generation
   identity records, including canonical normalization and fingerprinting.
3. Construct the mandatory origin scaffold, then add optional origin variation
   from origin-only parameters without replacing guaranteed records.
4. Generate unconstrained frontier locations, body resources, body counts,
   slots, distributions, and texture from validated frontier parameters; derive
   all reachability geometrically at query time.
5. Normalize deterministically and validate named structural invariants without
   invoking gameplay or a qualitative world evaluator.
6. Implement fact-level initial knowledge and transmissions, Shipyard/Habitat
   construction, probe construction/travel/observation, and expedition payload
   movement/founding in the approved tick order.
7. Add exact Tier 1 mechanic/action fixtures and direct constructor tests for
   mandatory origin placement, identity, references, migration invariants,
   field-level knowledge merge, and atomic travel/founding. Do not add seed-
   quality evaluation or tests that play the generated world.

## Acceptance boundaries

Stage 4b acceptance must not introduce:

- tests that play generated worlds to classify seeds as good or bad;
- `is_solvent`, `is_playable`, or other qualitative post-generation test
  oracles;
- economic surplus, runway, affordability, survival, or stability guarantees;
- statistical pass-rate thresholds or favorable-distribution gates;
- automatic tuning of reviewed generator ranges to repair one seed-specific
  outcome;
- reclamation behavior or migration of the standalone site model without a later approved design; or
- full replay/event-log compatibility beyond complete generation identity.

## Dependencies

- Completed Stage 4 origin resource and infrastructure engine.
- Approved Stage 4 authored fixture and structural handoff.
- G8–G10, G17–G18, and G22 as controlling direction, interpreted through the structural/no-economic-oracle stance above.
