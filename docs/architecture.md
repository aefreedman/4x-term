# Architecture

## Current boundary: Stage 3 origin-and-frontier substrate

The current workspace is a non-playable, data-driven simulation foundation. It
contains exactly two crates:

```text
game-content ──► game-core
```

`game-core` is the headless, format-independent domain and runtime owner.
`game-content` is an adapter that parses and compiles one RON world source into
`game-core::WorldDefinition`. There is no application, CLI, terminal UI,
production content bundle, save system, or startup path in the workspace.

The workspace uses Rust 2024 with MSRV Rust 1.97. Current dependencies are
`bevy_ecs` and `thiserror` in `game-core`; `game-content` additionally uses
`serde` and `ron`. RON and filesystem access end at `game-content`; terminal,
network, and presentation concerns do not enter `game-core`.

## Core model

`game-core` defines and validates a small, format-independent world:

- `ResourceDefinition`: a stable ID and display name; `core:energy` is the
  canonical physical Energy ID.
- `LocationDefinition`: a stable ID, display name, and finite `Position3`.
  Locations are neutral geography and have no implicit population, inventory,
  policy, production, or market state.
- `OriginCommunityDefinition`: the sole living community, with a stable ID, one
  location reference, nonzero population, and physical `ResourceStore` stocks.
- `ResourceDepositDefinition`: a stable ID, known location and resource, and a
  nonzero quantity.
- `ReclaimableSiteDefinition`: a stable ID and known location. Site internals,
  bodies, slots, surveys, yields, and outcomes are not yet modeled.
- `TopologyDefinition`: explicit undirected location-ID edges. Edge distances
  are derived from finite positions; endpoint order is canonicalized.

A `WorldDefinition` owns exactly one origin rather than a community list. During
`WorldState` construction, locations, deposits, sites, topology, and that one
origin are instantiated only after complete validation and normalization.
Snapshots expose the same substrate deterministically. Physical stocks belong
to the origin community, not to geography.

Explicit topology is independent of all frontier records. Empty and
disconnected graphs are valid. The core can return deterministic shortest paths
where a path exists, but it makes no connectivity, navigability, generation, or
world-quality claim.

## Core contracts

Runtime ECS entity IDs are internal and ephemeral. Stable `ContentId` values
identify definitions and references. Core collections and snapshots use stable
ordering so equivalent input permutations have equivalent normalized results.

Physical resources use checked quantities. `transfer_resource` calculates the
source, destination, and ledger results before mutating any of them; rejected
transfers leave all three unchanged. `Energy` remains checked physical
arithmetic, not currency or a commercial contract.

`game-core` owns no parser, filesystem operation, terminal type, frontend
command loop, or presentation model. It has no simulation `step` API, trader
command, market query, pricing, wallet, reservation, fleet, or market-per-
location behavior.

## Content compilation

`game-content` accepts one schema-specific RON world source containing
resources, locations, one origin, optional deposits and sites, and optional
explicit topology edges:

```text
RON source
→ parse with document provenance
→ schema/reference/value validation
→ deterministic aggregated diagnostics or WorldDefinition
→ optional WorldState instantiation by the caller
```

It rejects unknown source fields and validates stable IDs, duplicate IDs,
finite coordinates, nonzero origin population and deposit quantities, known
references, and self or duplicate edges. Independent semantic failures are
aggregated in deterministic source/definition/field order. A parse or read error
also retains document provenance. No repository-directory loader or production authored bundle is a
current contract; the only sources are focused test fixtures.

## Testing and evidence

The workspace currently has 15 focused deterministic tests: nine in
`game-core` and six in `game-content`. They provide Tier 1 evidence for:

- stable IDs and checked Energy arithmetic;
- normalized snapshots and permutation-invariant compilation/instantiation;
- one living origin and neutral, including isolated, frontier locations;
- empty or disconnected topology and deterministic rejection of self,
  duplicate, or unknown edges;
- duplicate, non-finite, zero, and unknown-reference definition rejection;
- exact physical-resource reconciliation and validate-before-mutate rejection
  atomicity; and
- RON parse provenance plus deterministic, source-aware aggregated diagnostics.

This is the current acceptance surface, alongside formatting, workspace check,
Clippy with warnings denied, and workspace tests. A generated seed outcome is
not a failure unless it violates a named engine invariant or a future G18
constructive guarantee. Local collapse, authored-world counts, global
connectivity, and statistical world-quality gates are not Stage 3 acceptance.

## Migration boundary and future work

Stage 4 will add the authored, headless origin resource/infrastructure engine:
deterministic ticks, G13 bodies/slots/developments, system-owned stocks and
construction, population-zero bootstrap work, seasonal Energy, Batteries,
extraction, refining, and exact accounting. It will not add population
arrival/change, scouting/outward commands, map generation, or playable startup.
The existing standalone reclaimable-site substrate remains unchanged.

Stage 4b will derive structural G18 requirements from implemented gameplay, add
constructive generated topology/frontier placement, record complete generation
identity/provenance, and implement one approved bounded outward action. It must
construct mandatory records before optional texture and never reject, reroll, or
screen worlds for economic quality.

Stage 5 may introduce a new application/session, CLI, and terminal rendering
adapter around the headless core. If added, those are future adapters: terminal
input and rendering must remain outside `game-core`, and they must not expose
unrestricted ECS mutation. They are not present or implied by the current
workspace. The retained
[Frontend Architecture Lessons](2026-07-20-frontend-architecture-lessons.md)
record useful prototype patterns and removed dependencies for consideration,
not compatibility requirements.

Later work may add persistence, population arrival/change, deeper outward
actions, reclamation, community dynamics, and player-owned logistics only when
their concrete contracts exist.
The retired trader/market prototype is not a compatibility target. Delete
obsolete code and content rather than preserving adapters, shells, or archives;
Git history is the recovery path.
