# Project Context

## Scope

This map describes the repository as observed in manifests, source, tests, CI, and current architecture documents. It records implemented structure, not intended gameplay from exploratory design notes.

## Stack

- **Language and runtime:** Rust workspace, edition 2024, minimum supported Rust version 1.97.
- **Application shape:** synchronous native terminal game. There is no web server, network API, async runtime, database, persistence layer, or background worker.
- **Terminal UI:** Ratatui 0.30.2 with the minimal Crossterm 0.29 backend; Crossterm 0.29 owns terminal events and mode changes.
- **Data and configuration:** RON 0.12.2 and Serde 1.0.228. Active mutable tuning is repository-owned RON, currently `content/profiles/starter.ron`.
- **Errors:** `thiserror` 2.0.18 for typed library-boundary errors. The executable converts final failures to human-readable strings.
- **Determinism and identity:** SHA-256 fingerprints via `sha2` 0.10.9; ordered collections and canonical encoding support reproducible generation.
- **Text layout:** `unicode-width` 0.2.2 is used at the application and TUI boundaries for terminal-cell-safe labels.
- **Tooling:** Cargo; rustfmt; Clippy with warnings denied in CI; GitHub Actions on pushes and pull requests.
- **Safety policy:** workspace lint configuration forbids unsafe code. No unsafe blocks were found.
- **Dependency policy:** dependencies are deliberately small. There is no general CLI parser; `game-play` has a bounded, tested parser for its current flags.

## Workspace Architecture

The workspace is a directed layered system:

```text
4x-term workspace
  `game-play` executable composition
       |-> `game-tui` terminal adapter
       |      `-> `game-app` application/session boundary
       |             |-> `game-content` content and generation adapter
       |             |      `-> `game-core`
       |             `-> `game-core`
       `-> `game-app`

`game-core` -> thiserror only
```

The root `Cargo.toml` is a virtual workspace manifest with no root package; the running binary is `crates/game-play`.

### `game-core`: headless domain and simulation

- Owns world state, resources, population, ships, routing, knowledge, simulation order, typed identifiers, commands, assessments, and invariants.
- Keeps implementation modules private and exposes an intentional re-export surface through `crates/game-core/src/lib.rs`.
- Uses integer/fixed-rate arithmetic and checked operations rather than floating-point simulation arithmetic.
- Stores identity-sensitive state in typed IDs and deterministic `BTreeMap`/`BTreeSet` collections.
- `WorldState::advance_tick` is transactional: clone complete state, validate it, execute ten global phases in stable system-ID order, validate again, build the player view, and commit only on success.
- Read-only assessments and mutating commands share private validation-plan logic; a command revalidates at commit time instead of trusting a stale assessment.
- The ordinary adapter boundary is `PlayerWorldView`, which redacts unknown systems, routes, losses, and delayed information. Complete snapshots are compiled only for tests or the explicit `test-support` feature.

### `game-content`: strict content and deterministic generation

- Owns RON source schemas, compilation diagnostics, semantic validation, profile normalization, canonical encoding, provenance, fingerprints, and procedural generation.
- Source schemas consistently use `#[serde(deny_unknown_fields)]`; malformed or unknown data fails instead of being ignored.
- Content compilation accumulates ordered `ContentDiagnostic` values with source, definition, field, and message rather than failing at the first semantic problem.
- Machine-local profile paths are read at startup but excluded from generated provenance; logical source identity is retained instead.
- Generation is request/response based: normalized profile plus generator version and seed produce a reproducible `GeneratedWorldArtifact` with identity metadata.

### `game-app`: application and information boundary

- Owns startup coordination and the sole mutable `WorldState` during a session.
- `StartupCoordinator` owns generated artifacts before play; `Session` owns simulation state after confirmed startup.
- Adapters submit typed `SessionIntent` values and receive immutable `PlayingView`, assessment DTOs, typed outcomes, and `DraftDisposition` guidance.
- Domain command rejection is normally represented as `SessionOutcome::Rejected(ApplicationOutcome)` with `LimitingReason`; unexpected projection failure bubbles as `ApplicationError`.
- Player-facing labels, aliases, map visuals, outcome copy, and TUI-safe DTOs live here rather than in the core.
- Safe core IDs and enums are re-exported so `game-tui` does not need a direct dependency on `game-core`.

### `game-tui`: synchronous terminal adapter

- Owns keyboard mapping, startup and playing UI state, modal/draft state, paced tick batches, rendering, terminal lifecycle, and semantic playtest events.
- The main loop synchronously performs due tick work, renders, polls for at most 100 ms, and handles one terminal event. It deliberately uses no Tokio, channels, or concurrent simulation.
- `TuiState` converts keyboard actions into application intents and retains or invalidates drafts according to application outcomes.
- Rendering consumes application DTOs; it does not inspect or mutate core state.
- Event polling and terminal-mode transitions are isolated behind `EventSource` and `TerminalOps`; Ratatui terminal construction, drawing, sizing, and cursor display remain direct calls in the TUI loop. `TerminalGuard` uses staged RAII cleanup so partial mode-setup failures restore completed terminal modes in reverse order.
- Clock, event, terminal-operation, and observer boundaries are injectable. Focused tests exercise state with controlled timestamps and terminal cleanup with mock operations, but not every injectable loop seam has an integration test.

### `game-play`: executable composition and local artifact I/O

- Owns process arguments, help text, default profile selection, exit behavior, and the concrete local playtest recorder.
- `main` reports one final startup/runtime error to stderr and exits with status 1.
- Trace files are reserved before terminal acquisition, use create-new/no-overwrite behavior, and are finalized after terminal cleanup.
- `RonlRecorder` serializes versioned semantic events as append-only RON lines and writes a derived RON summary. This is local playtest evidence, not general production telemetry.

## Primary Data Flows

### Startup

```text
CLI/default profile
  -> game-tui TuiState
  -> game-app StartupCoordinator
  -> game-content strict RON load + normalize + fingerprint
  -> versioned deterministic generation
  -> game-app allowlisted preview
  -> explicit confirmation
  -> game-core WorldState validation
  -> game-app Session
```

The session starts from exactly the generated artifact shown in the current preview. Editing the profile or seed makes the preview stale and prevents startup until regeneration.

### Player action

```text
Crossterm event
  -> game-tui Action / draft
  -> game-app SessionIntent
  -> game-core assessment or command
  -> game-app SessionOutcome + player-safe PlayingView
  -> game-tui state update
  -> Ratatui render
```

There is no REST, GraphQL, or RPC API. The architectural API is a typed in-process Rust boundary. Naming follows Rust `snake_case` for fields/functions and `PascalCase` for types and enum variants.

### Tick

```text
SessionIntent::AdvanceOneTick
  -> clone WorldState
  -> pre-tick integrity validation
  -> ten deterministic global phases
  -> post-tick integrity validation
  -> knowledge-filtered PlayerWorldView
  -> commit cloned candidate
  -> TickStepView with player-visible delta
```

A failed phase does not partially mutate the live session.

### Opt-in playtest observation

```text
semantic TUI event / application outcome / tick delta
  -> PlaytestObserver
  -> game-play RonlRecorder
  -> local .ronl trace + .summary.ron
```

The trace intentionally excludes raw keys, rendered screens, aliases, machine paths, hidden world state, and unreceived information.

## Conventions Observed

### Error handling

- Libraries return `Result` and use `thiserror` enums at meaningful boundaries: `CoreError`, `ContentErrors`, `GenerationError`, `ApplicationError`, and `TuiError`. Type preservation is uneven: several variants intentionally flatten lower-level causes to `String`, and `game-play` uses string errors throughout.
- Validation and expected player rejection are data, not panics. Application outcomes include accepted/rejected state, intent kind, limiting reason, message, and draft disposition.
- Content diagnostics are structured and may aggregate multiple errors.
- I/O errors bubble through the TUI as `TuiError::Io`; observer failures use `TuiError::Playtest`; the binary adds operational context and presents the final message.
- Most production `expect` and `unreachable!` calls assert conditions established by prior validation or local control flow, such as stable map keys and matched CLI prefixes. One artifact-collision `unreachable!` instead relies on the operational assumption that every `u64` suffix cannot be occupied. Tests use panic/unwrap/expect conventionally.
- RAII protects terminal cleanup. Recorder finalization explicitly combines runtime and summary-writing failures so one does not silently hide the other.

### API and information design

- Cross-crate contracts use owned typed structs/enums, not stringly typed maps.
- `game-app` is both application service and anti-corruption/information boundary: it translates core errors and complete domain state into player-safe outcomes and DTOs.
- Mutation ownership is narrow: `Session` is the only mutable simulation owner after startup.
- Complete engine snapshots are unavailable to ordinary production adapters.
- Input schemas are strict; trace output schemas carry explicit version fields.

### Type safety

- Unsafe code is forbidden workspace-wide.
- IDs such as `ContentId`, `ProjectId`, `ShipId`, `PopulationId`, and transmission/observer IDs prevent accidental identity mixing.
- `NonZeroU64`, enums, private fields/modules, and constructor validation encode invariants.
- Checked arithmetic is pervasive in simulation, routing, resources, generation, trace sequencing, and tick progression.
- Dependency inversion is used selectively at I/O/time/observation seams through small traits. Core domain operations mostly use concrete types rather than pervasive trait abstraction.

### Determinism

- Simulation is synchronous and phase-major.
- Stable ordered collections and stable system-ID iteration define processing order.
- Generation identity includes generator version, seed, and normalized configuration fingerprint.
- Canonical encoding is separate from ordinary RON formatting.
- Seed-specific outcomes are not acceptance failures unless they violate a named invariant or the constructive-generation contract.

### Observability

- There is no always-on structured logger, metrics backend, tracing framework, health endpoint, or remote telemetry.
- Ordinary operation emits only concise CLI/startup/error text.
- Opt-in local playtest tracing is structured, versioned, append-only, and semantic. It supports Ring 1 playtest analysis but is not a substitute for runtime diagnostics or production monitoring.

### Testing

- Tests are split between colocated unit/boundary modules and public integration tests under crate `tests/` directories.
- The suite uses deterministic hand-constructed Tier 1 fixtures for domain behavior, content fixture files for parser/compiler boundaries, and fixed seeds for generation properties.
- Core integration coverage exercises global tick order, atomic rollback, routing, delayed knowledge, ships, expansion, and redaction.
- TUI tests operate headlessly against state/actions and controlled timestamps; renderer tests use test buffers, semantic trace tests drain the state event queue, and terminal lifecycle tests use mock terminal operations.
- `Clock`, `EventSource`, and `PlaytestObserver` are injectable at the run-loop boundary, but there is no end-to-end test that combines injected clock/events with a failing observer through `run_loop_observed`.
- Filesystem behavior uses temporary test directories and real local file semantics rather than a mocking framework.
- No third-party mocking or property-testing framework is present.
- CI runs format, all-target/all-feature check, Clippy with `-D warnings`, and all-feature workspace tests.
- The public `test-support` feature deliberately exposes complete engine snapshots for privileged invariant evidence. Repository convention restricts it to tests, but Cargo does not technically prevent a production consumer from enabling it.

## Signals / Active Considerations

### Established strengths

- The implemented dependency direction matches `docs/architecture.md`; no architectural surprise or hidden reverse dependency was found.
- The headless simulation boundary is real rather than aspirational: terminal dependencies stop at `game-tui`, and `game-tui` consumes only `game-app`.
- Transactional ticks, checked arithmetic, strict content parsing, deterministic generation identity, and player-safe redaction are unusually explicit for the current project size.
- I/O seams are narrow and independently testable without introducing broad framework abstractions.

### Cohesion and change-risk hotspots

These are planning signals, not established defects:

- `crates/game-app/src/lib.rs` is about 2,300 lines and combines startup, projection, presentation labels, visual mapping, assessments, intent dispatch, and outcome translation.
- `crates/game-tui/src/state.rs` is about 1,800 lines and owns a large interaction state machine spanning startup, editors, modals, drafts, missions, batches, selection, and semantic event emission.
- `crates/game-core/src/world.rs`, `crates/game-core/src/ships.rs`, and `crates/game-core/src/population.rs` are each large domain modules. Their size reflects substantial invariants and cross-cutting state; changes need focused impact analysis before extraction or refactoring.
- `crates/game-content/src/lib.rs` is about 1,800 lines and combines compilation orchestration, conversion helpers, validation, and extensive colocated tests.
- `crates/game-tui/src/render/mod.rs` is about 1,700 lines. Rendering remains adapter-only, but screen growth may increase review and merge-conflict cost.

Do not split these modules solely by line count. Preserve existing ownership and transaction/information boundaries; extract only around a concrete cohesion or testability need.

### Error-model boundaries

- Error types are strongest at core validation and structured content-diagnostic boundaries. Type erasure also occurs inside some nominally typed layers, including knowledge integration, generated-core failures, startup generation, projection, and observer errors; `game-play` is string-error based throughout. This is acceptable at the current human-facing boundary but will be a migration concern for persistence, replay, automation, or richer diagnostics.
- Trace tests cover selected semantic events and logical startup identity, but no complete serialized-trace privacy regression explicitly proves that aliases, raw keys, machine paths, and privileged state remain absent.
- `ApplicationError` currently represents only projection failure; ordinary domain rejection is intentionally an outcome. Future callers must preserve that distinction rather than converting all rejection into exceptional errors.

### Toolchain and CI gap

- The manifest declares Rust 1.97 as the MSRV, but CI installs `stable` and does not separately test the declared minimum version. As dependencies or language usage evolve, the MSRV can drift without CI detecting it.

### Current product signal

- The complete technical Ring 1 loop is present. Current planning evidence classifies it as **player-complete, decision-quality validation pending**.
- The primary open question is experiential dependability: whether players can understand and repeat the bootstrap-to-founding cycle and make legible bank/develop/expand decisions.
- Opt-in local playtest analytics exists to gather descriptive evidence. It must not become a statistical world-quality gate or expose privileged state.

### Future integration boundaries

Planned persistence, replay, or agent automation should enter through application intents and player-safe views. They must not mutate `game-core::WorldState` directly or use `test-support` snapshots as a production API.

Adding async execution, channels, a network protocol, a database, new dependencies, or new crate boundaries requires a concrete need and architecture review. The current synchronous in-process model is deliberate.

## Repository Authorities

- `AGENTS.md` — agent and project working rules.
- `docs/architecture.md` — architectural contracts and dependency direction.
- `docs/design/README.md` — design-document authority and precedence.
- `docs/design/current/` — current mechanical authority.
- `docs/design/direction/` — committed long-term direction.
- `content/profiles/starter.ron` — active mutable tuning values.
- `.github/workflows/ci.yml` — automated quality gate.
