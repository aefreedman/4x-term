# Architecture

## Current playable origin-and-frontier boundary

The workspace is a data-driven headless simulation with a synchronous human terminal adapter:

```text
4x-term ──► game-tui ──► game-app ──► game-core
    └───────────────────────┘            ▲
                         game-content ────┘
```

`game-core` owns the format-independent world model, runtime authorities, validated commands, read-only assessments, player-safe projection, and global simulation tick. `game-content` owns strict RON schemas, filesystem loading, profile normalization and validation, canonical profile encoding, fingerprints, and deterministic revisioned generation. `game-app` owns startup coordination and the sole mutable `WorldState` in a running session. `game-tui` owns input, selection, drafts, pacing, rendering, terminal lifecycle, and an opt-in queue of player-safe semantic playtest events. The `game-play` package produces the `4x-term` binary and owns process composition, CLI parsing, and the optional local trace file sink.

The dependency direction is enforced by manifests: terminal dependencies exist only in `game-tui`; `game-app` has no terminal types; and `game-tui` has no direct `game-core` dependency. RON, provenance, hashing, and filesystem access do not enter `game-core`. Tokio, channels, asynchronous clocks, persistence, and agent-facing command protocols are absent.

The workspace uses Rust 2024 with MSRV Rust 1.97. Ratatui/Crossterm are used synchronously with minimal Ratatui features. `content/profiles/starter.ron` is the executable's editable convenience default, not a canonical universe.

## Implemented module boundaries

`game-core` keeps its modules private and re-exports the public domain surface from `lib.rs`:

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

The core exposes non-mutating construction, generic development-operation, Habitat-generation, probe, and expedition assessments. Each assessment and its command share the same private validation plan; commands revalidate against current state before atomic commit. Disabling a development preserves its cycle, Habitat-generation, and Shipyard-queue state while excluding it from production, support, capacity, and project progression. Local player projections include only derived resident counts and occupied Habitat coordinates, never population-token identity or transit internals.

`game-content` exposes only its loading/compilation/generation hooks while keeping source schemas internal:

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

`WorldDefinition` is the validated construction input. Every location has an always-present system definition, fixed-point `Position3`, stellar strength, ordered bodies and slots, initial body resources, initial stocks, and infrastructure. Explicit route graphs and standalone deposits no longer exist. Routes are derived from committed positions and a ship's jump limit.

`WorldState` owns the authoritative runtime:

- `map_systems` owns normalized immutable system/body/slot map facts;
- each `SystemState` owns mutable stocks, remaining body-resource quantities, developments, construction and Shipyard queues, completed local assets, reservations, counters, overflow, and resource accounting;
- initial body-resource quantities exist only in map definitions, while remaining quantities exist only in body runtime state;
- `PopulationRegistry` is the sole mutable population authority; community population and Habitat occupancy are derived from resident tokens;
- `transit` is the sole physical authority for launched ships and in-transit populations are reconciled bijectively with expeditions;
- `KnowledgeState` owns origin knowledge, pending/received transmissions, player-facing mission outcomes, and fact-level merge state; and
- one world-level `SimulationTime` advances all systems in stable system-ID order and ships in stable ship-ID order.

`WorldState::advance_tick` clones the complete candidate state, executes all ten approved phases globally, validates runtime integrity, builds the player view, and commits only on success. A rejection therefore preserves the clock, counters, stocks, body resources, populations, queues, transit, knowledge, accounting, and evidence records together.

Stable typed IDs, rather than runtime entities, identify definitions, projects, ships, populations, observers, transmissions, and reservation owners. Body/slot and FIFO queue order are semantic state; unordered definition collections and profile maps normalize deterministically.

## Public player boundary

`WorldState` fields are crate-private. Callers mutate it through validated commands such as construction, Habitat controls, Shipyard enqueue/cancellation, probe/expedition launch, and `advance_tick`.

`WorldState::player_view` and the value returned by `advance_tick` are the player-adapter boundary. `PlayerWorldView` contains:

- the world time;
- identified systems and their received `SystemKnowledge`;
- authoritative `SystemSnapshot` local state only for the origin or a founded system whose successful report has been received;
- anonymous indication count and position-derived fog texture points carrying only stable opaque visual-assignment keys, not system identities or facts;
- received/awaiting mission states; and
- active routes redacted so an unidentified intermediate stop is named only after the ship reaches it.

It intentionally omits the global population registry, pending transmissions and hidden mission outcomes, global accounting, neutral-system runtime, and unreceived founding-loss evidence. Physical arrival can therefore occur before the player learns the result or gains remote commandability.

## Test-support boundary

The `test-support` feature exposes privileged deterministic diagnostics, not a player API:

- `WorldState::debug_snapshot`, `WorldState::debug_system_snapshot`, `WorldSnapshot`, and `CommunitySnapshot` are compiled only under `cfg(test)` or `feature = "test-support"`;
- complete-state `Clone`, `Debug`, `Eq`, and `PartialEq` for `WorldState` are likewise feature/test gated; and
- the four cross-crate integration-test targets require `test-support` in their Cargo manifests.

Production adapters must use `player_view`; they must not enable diagnostic snapshots to bypass knowledge or mission redaction.

## Content and generation pipeline

Authored fixture compilation remains available for small deterministic Tier 1 worlds:

```text
RON world source
→ strict parse with logical provenance
→ deterministic schema/reference/value diagnostics
→ normalized WorldDefinition
→ optional WorldState construction
```

Authored Tier 1 definitions may seed coherent resident population tokens backed by functional Habitats and communities. Initialization establishes an explicit population-accounting baseline and advances birth-system counters beyond every seeded ID. Initial in-transit tokens remain outside this fresh-world input because the current product has no complete runtime-restoration format.

Procedural generation uses a separate explicit pipeline:

```text
RON profile
→ strict parse and normalized validation
→ canonical bytes + SHA-256 fingerprint + logical source provenance
→ GenerationRequest(version, seed, compiled profile)
→ GeneratedWorldArtifact(identity, provenance, normalized WorldDefinition)
```

Complete generation identity is generator family/revision, unsigned 64-bit seed, and normalized-profile fingerprint. Revision 1 constructs only the approved origin scaffold as a structural guarantee. Frontier count may differ from its approximate target, and generation makes no connectivity, reachability, solvency, favorable-distribution, survival, or qualitative-world claim.

## Application and terminal boundary

`StartupCoordinator` loads an explicit `ProfileDescriptor`, generates an allowlisted preview, marks it stale after edits, and consumes exactly the confirmed artifact. Its generated preview may render anonymous frontier fog at gameplay map scale without exposing neutral identities or facts. Session-owned map-visual assignments apply uniformly to Origin and frontier systems, remain stable as knowledge changes, and keep non-Plain pivots within four map units of actual systems while Plain stays exact; discovery overlays the chart point rather than removing its surrounding visual. Machine paths and reproduction metadata do not enter play views. `Session` exclusively owns mutable simulation state and accepts typed `SessionIntent` values. It returns immutable `PlayingView`, typed rejection and `DraftDisposition`, launch assessments, and one-step tick deltas.

The application catalogue resolves resource and FSC labels. Session-owned aliases never alter generated identity. Runtime projections derive solely from `PlayerWorldView`, so neutral local state, hidden route stops, pending report contents, and unreceived founding outcomes remain unavailable to the TUI. Slot-level probe and expedition actions are projected as typed launch, enqueue, or unavailable choices; the TUI does not reconstruct Shipyard or completed-asset rules from display rows. The core projects only the current physical coordinates of active player ships for map rendering; ship movement geometry is not reconstructed by the terminal.

`game-tui` routes arrows and the selected keyboard layout through one semantic input layer. It renders a `160x45` reference composition and blocks gameplay below that size. Explicit multi-tick requests call one application tick at a time through an injectable monotonic clock, preserving every committed intermediate view and stopping between ticks. Terminal setup uses staged RAII cleanup for raw mode, alternate screen, and cursor state.

Opt-in playtest tracing observes semantic TUI transitions and the existing typed application dispatch boundary. `game-tui` emits versioned events through a caller-owned `PlaytestObserver`; it performs no trace filesystem I/O. `game-play` parses `-T` / `--playtest-trace`, reserves non-overwriting raw and summary artifacts before terminal acquisition, writes compact RON-lines events, and finalizes a typed descriptive summary after terminal cleanup. The ordinary no-argument path installs no trace state.

Trace events use only admitted startup data, `SessionIntent`, `SessionOutcome`, `PlayingView`, assessments, and `TickDeltaView`. They omit raw keys, rendered text, aliases, machine paths, pending transmission contents, hidden route identities, authoritative neutral state, and every `test-support` projection. Trace write failures return through `TuiError` after staged terminal restoration; incomplete sessions receive an explicitly incomplete summary when finalization remains possible.

## Testing and evidence

Focused deterministic tests cover core rules and redaction, strict content and generation, startup staleness and exact artifact consumption, typed assessments and atomic rejection, aliases and projections, semantic input, paced batches, minimum-size safety, Ratatui rendering, and terminal lifecycle cleanup.

The acceptance surface is formatting, all-target/all-feature compilation, Clippy with warnings denied, and the all-feature workspace test suite. Generated seed outcomes fail acceptance only when they violate an active invariant or the constructive origin guarantee; local collapse and qualitative frontier texture are not failures.

## Future adapters

Persistence, event-log replay, an agent-facing command protocol, reclamation, automated freight, wider logistics, delegation, and cultural influence remain future work. Any adapter must continue to use typed application intents and player-safe views rather than unrestricted runtime mutation or privileged `test-support` snapshots. The retired trader/market prototype and intermediate migration schemas are not compatibility targets; Git history is the recovery path.
