# Architecture

## Current boundary: Stage 4 origin resource engine

The current workspace is a non-playable, data-driven headless simulation. It
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

`game-core` defines and validates a format-independent world:

- `ResourceDefinition`: a stable ID and display name; `core:energy` is the
  canonical physical Energy ID.
- `LocationDefinition`: a stable ID, display name, and finite `Position3`.
- `OriginCommunityDefinition`: the sole community record, containing identity,
  location, and population only. Population zero is valid.
- `SystemDefinition`: persistent location-owned stocks and optional complete
  Stage 4 resource-engine prerequisites.
- `ResourceDepositDefinition`: a stable ID, known location and resource, and a
  nonzero mutable runtime quantity.
- `ReclaimableSiteDefinition`: the unchanged stable ID/location substrate.
- `TopologyDefinition`: explicit undirected location-ID edges with canonical
  endpoint order and derived finite distance.

A Stage 4 resource-engine definition contains a ten-phase per-Collector Energy
profile, generic bodies and stable slots, installed development identity and
condition, and validated designer-authored tuning. Runtime system state owns
available stocks, independent production cycles, construction commitments and
reservations, overflow evidence, and accounting. Community state never owns a
treasury or infrastructure.

`WorldState` validates and normalizes the complete definition before creating
runtime state. Stage 3 substrate definitions with no engine remain loadable and
snapshotable; `advance_tick` rejects them atomically with a missing-prerequisite
error.

Explicit topology is independent of all frontier records. Empty and
disconnected graphs are valid. The core can return deterministic shortest paths
where a path exists, but it makes no connectivity, navigability, generation, or
world-quality claim.

## Core contracts

Runtime ECS entity IDs are internal and ephemeral. Stable `ContentId` values
identify definitions and references. Core collections and snapshots use stable
ordering so equivalent input permutations have equivalent normalized results.

Physical resources use one checked unsigned `ResourceStore` quantity model.
`transfer_resource`, capacity-aware system receipts, construction enqueue and
cancellation, and complete ticks calculate all affected results before mutation.
Rejected operations leave their complete relevant state unchanged. Energy is a
physical resource, not currency or a commercial contract.

`game-core` owns no parser, filesystem operation, terminal type, frontend
command loop, or presentation model. It has no simulation `step` API, trader
command, market query, pricing, wallet, reservation, fleet, or market-per-
location behavior.

## Content compilation

`game-content` accepts one strict RON world source containing resources,
locations, one origin, optional systems and Stage 4 engine definitions, optional
deposits and unchanged sites, and optional explicit topology edges:

```text
RON source
→ parse with document provenance
→ schema/reference/value validation
→ deterministic aggregated diagnostics or WorldDefinition
→ optional WorldState instantiation by the caller
```

It rejects unknown source fields and validates stable IDs, duplicate IDs,
finite coordinates, system-owned stock references, nonzero deposit quantities,
Stage 4 profiles, bodies, slots, developments, recipes and numeric tuning, and
self or duplicate edges. Zero origin population is valid. Independent semantic
failures are aggregated in deterministic source/definition/field order. A parse
or read error also retains document provenance. No repository-directory loader
or production authored bundle is a current contract; the only sources are
focused test fixtures.

## Testing and evidence

The workspace currently has 40 focused deterministic tests: 31 in
`game-core` and nine in `game-content`. They provide Tier 1 evidence for:

- stable and collision-safe IDs plus normalized, permutation-independent state;
- neutral frontier locations and valid empty or disconnected topology;
- strict source-aware Stage 3/4 content validation;
- system-owned stocks and zero-population origin work;
- exact transfers, storage overflow, cancellation refunds, and checked
  reconciliation;
- deterministic development conditions, production cycles, role/body/slot
  ordering, and completion timing;
- FIFO construction, slot/deposit reservation, and rejected-command/tick
  atomicity; and
- the exact authored 20-tick Collector → Refinery → Battery → Extractor
  bootstrap.

This is the current acceptance surface, alongside formatting, workspace check,
Clippy with warnings denied, and workspace tests. A generated seed outcome is
not a failure unless it violates a named engine invariant or a future G18
constructive guarantee. Local collapse, authored-world counts, global
connectivity, and statistical world-quality gates are not Stage 4 acceptance.

## Migration boundary and future work

Stage 4 now provides the authored, headless origin resource/infrastructure
engine. It intentionally does not add population arrival/change,
scouting/outward commands, map generation, or playable startup. The standalone
reclaimable-site substrate remains unchanged.

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
