# Architecture

## Current boundary: Stage 4b constructive frontier

The workspace is a non-playable, data-driven headless simulation with exactly
two crates:

```text
game-content ──► game-core
```

`game-core` owns the format-independent world model, runtime authorities,
commands, player-safe projection, and global simulation tick. `game-content`
owns strict RON schemas, filesystem loading, profile normalization and
validation, canonical profile encoding, SHA-256 fingerprints, and deterministic
revisioned generation. RON, provenance, hashing, and filesystem access do not
enter `game-core`.

There is no application/session crate, CLI, TUI, save format, production startup
path, or playable content bundle. `content/profiles/starter.ron` is an editable
generation/gameplay baseline consumed explicitly by callers and tests; it is not
a canonical universe or startup contract.

The workspace uses Rust 2024 with MSRV Rust 1.97. `game-core` depends only on
`thiserror`. `game-content` depends on `game-core`, `serde`, `ron`, `sha2`, and
`thiserror`. No crate depends on a renderer, terminal library, network runtime,
or ECS framework.

## Implemented module boundaries

`game-core` keeps its modules private and re-exports the public domain surface
from `lib.rs`:

| Module | Current ownership |
| --- | --- |
| `ids.rs` | Stable content, project, ship, population, transmission, observer, and reservation-owner IDs plus monotonic counters. |
| `world.rs` | Aggregate definitions and `WorldState`; immutable map/runtime separation; validation; commandability; construction commands; player and diagnostic projections. |
| `resources.rs` | Resource stores, body/map/runtime resource shapes, developments, construction, tuning, accounting, and checked transfers. |
| `population.rs` | Communities, the sole population-token registry, Habitat generation/support state, and population transition accounting. |
| `routing.rs` | Fixed-point positions, checked distance/rate arithmetic, deterministic geometric shortest routes, and route redaction. |
| `knowledge.rs` | Knowledge levels and keyed facts, observations, delayed transmissions, deterministic fact merge, and origin-facing mission state. |
| `ships.rs` | Shipyard projects and assets, probe/expedition launch, world-owned transit, observations, reservations, founding, and typed loss. |
| `simulation.rs` | The world clock and the single phase-major, whole-world atomic tick. |

`game-content` exposes only its loading/compilation/generation hooks while
keeping source schemas internal:

| Module | Current ownership |
| --- | --- |
| `schema.rs` | Strict authored-world and profile RON source types with unknown-field rejection. |
| `diagnostics.rs` | Deterministic source-aware compilation diagnostics. |
| `profile.rs` | Validated normalized gameplay and generator tuning. |
| `fingerprint.rs` | Canonical normalized profile encoding and SHA-256 fingerprinting. |
| `generator.rs` | `core:frontier_world@1`, complete generation identity/provenance, domain-separated SplitMix64 streams, and deterministic frontier generation. |
| `lib.rs` | Public string/file compilation APIs and source-to-core translation. |

No additional crate boundary is implied by this module split.

## World ownership and simulation

`WorldDefinition` is the validated construction input. Every location has an
always-present system definition, fixed-point `Position3`, stellar strength,
ordered bodies and slots, initial body resources, initial stocks, and
infrastructure. Explicit route graphs and standalone deposits no longer exist.
Routes are derived from committed positions and a ship's jump limit.

`WorldState` owns the authoritative runtime:

- `map_systems` owns normalized immutable system/body/slot map facts;
- each `SystemState` owns mutable stocks, remaining body-resource quantities,
  developments, construction and Shipyard queues, completed local assets,
  reservations, counters, overflow, and resource accounting;
- initial body-resource quantities exist only in map definitions, while
  remaining quantities exist only in body runtime state;
- `PopulationRegistry` is the sole mutable population authority; community
  population and Habitat occupancy are derived from resident tokens;
- `transit` is the sole physical authority for launched ships and in-transit
  populations are reconciled bijectively with expeditions;
- `KnowledgeState` owns origin knowledge, pending/received transmissions,
  player-facing mission outcomes, and fact-level merge state; and
- one world-level `SimulationTime` advances all systems in stable system-ID
  order and ships in stable ship-ID order.

`WorldState::advance_tick` clones the complete candidate state, executes all ten
approved phases globally, validates runtime integrity, builds the player view,
and commits only on success. A rejection therefore preserves the clock,
counters, stocks, body resources, populations, queues, transit, knowledge,
accounting, and evidence records together.

Stable typed IDs, rather than runtime entities, identify definitions, projects,
ships, populations, observers, transmissions, and reservation owners.
Body/slot and FIFO queue order are semantic state; unordered definition
collections and profile maps normalize deterministically.

## Public player boundary

`WorldState` fields are crate-private. Callers mutate it through validated
commands such as construction, Habitat controls, Shipyard enqueue/cancellation,
probe/expedition launch, and `advance_tick`.

`WorldState::player_view` and the value returned by `advance_tick` are the
player-adapter boundary. `PlayerWorldView` contains:

- the world time;
- identified systems and their received `SystemKnowledge`;
- authoritative `SystemSnapshot` local state only for the origin or a founded
  system whose successful report has been received;
- anonymous indication count;
- received/awaiting mission states; and
- active routes redacted so an unidentified intermediate stop is named only
  after the ship reaches it.

It intentionally omits the global population registry, pending transmissions
and hidden mission outcomes, global accounting, neutral-system runtime, and
unreceived founding-loss evidence. Physical arrival can therefore occur before
the player learns the result or gains remote commandability.

## Test-support boundary

The `test-support` feature exposes privileged deterministic diagnostics, not a
player API:

- `WorldState::debug_snapshot`, `WorldState::debug_system_snapshot`,
  `WorldSnapshot`, and `CommunitySnapshot` are compiled only under
  `cfg(test)` or `feature = "test-support"`;
- complete-state `Clone`, `Debug`, `Eq`, and `PartialEq` for `WorldState` are
  likewise feature/test gated; and
- the four cross-crate integration-test targets require `test-support` in their
  Cargo manifests.

Production adapters must use `player_view`; they must not enable diagnostic
snapshots to bypass knowledge or mission redaction.

## Content and generation pipeline

Authored fixture compilation remains available for small deterministic Tier 1
worlds:

```text
RON world source
→ strict parse with logical provenance
→ deterministic schema/reference/value diagnostics
→ normalized WorldDefinition
→ optional WorldState construction
```

Authored Tier 1 definitions may seed coherent resident population tokens backed
by functional Habitats and communities. Initialization establishes an explicit
population-accounting baseline and advances birth-system counters beyond every
seeded ID. Initial in-transit tokens remain outside this fresh-world input because
Stage 4b has no complete runtime-restoration format.

Procedural generation uses a separate explicit pipeline:

```text
RON profile
→ strict parse and normalized validation
→ canonical bytes + SHA-256 fingerprint + logical source provenance
→ GenerationRequest(version, seed, compiled profile)
→ GeneratedWorldArtifact(identity, provenance, normalized WorldDefinition)
```

Complete generation identity is generator family/revision, unsigned 64-bit
seed, and normalized-profile fingerprint. Revision 1 constructs only the
approved origin scaffold as a structural guarantee. Frontier count may differ
from its approximate target, and generation makes no connectivity,
reachability, solvency, favorable-distribution, survival, or qualitative-world
claim.

## Testing and evidence

The workspace has 56 focused deterministic tests: 28 in `game-core` and 28 in
`game-content`. They cover fixed-point routing, strict profiles and canonical
fingerprints, revisioned generation and the origin scaffold, body-resource
ownership, retained Stage 4 resource mechanisms, global tick atomicity,
Habitats and token population, knowledge/transmission merge, Shipyards, probes,
expeditions, founding/loss, player-view redaction, and exact resource/population
reconciliation.

The acceptance surface is formatting, all-target/all-feature compilation,
Clippy with warnings denied, and the all-feature workspace test suite. Generated
seed outcomes fail acceptance only when they violate an active invariant or the
constructive origin guarantee; local collapse and qualitative frontier texture
are not failures.

## Future adapters

Stage 5 may add an application/session, truthful startup path, CLI, and terminal
rendering around the headless core and `PlayerWorldView`. Those adapters must
keep input and presentation outside `game-core` and must not expose unrestricted
runtime mutation or privileged test-support snapshots.

Persistence, event-log replay, reclamation, automated freight, wider logistics,
delegation, and cultural influence remain future work. The retired
trader/market prototype and the replaced Stage 3/4 schemas are not compatibility
targets; Git history is the recovery path.
