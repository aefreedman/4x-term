# Architecture Plan

## Goals

- Build a data-driven terminal game in Rust.
- Use an entity-component-system (ECS) as the simulation foundation.
- Keep the simulation headless and independent of terminal concerns.
- Treat the TUI strictly as an input and rendering adapter.
- Preserve the option to add other frontends or move the simulation into a separate process later.

## Technology choices

| Concern | Choice |
| --- | --- |
| Language | Rust |
| ECS | `bevy_ecs` 0.19 without the full Bevy engine |
| TUI | `ratatui` 0.30 |
| Terminal backend | `crossterm` 0.29 |
| Async runtime | `tokio` 1.52 |
| Serialization | `serde` 1.0 |
| Initial content format | RON through a format-specific adapter |
| Error handling | `thiserror` in libraries; `anyhow` at executable boundaries |
| Logging and diagnostics | `tracing` and `tracing-subscriber` |

RON is the initial human-authored content format. Runtime code must not depend on that choice; deserialized source definitions are validated and compiled into format-independent typed definitions.

The workspace MSRV is Rust 1.97. Dependency metadata was verified against the selected releases on 2026-07-10: [bevy_ecs 0.19](https://docs.rs/bevy_ecs/0.19.0), [Tokio 1.52](https://docs.rs/tokio/1.52.3), [Ratatui 0.30](https://docs.rs/ratatui/0.30.2), [Crossterm 0.29](https://docs.rs/crossterm/0.29.0), [Serde 1.0](https://docs.rs/serde/1.0.228), and [RON 0.12](https://docs.rs/ron/0.12.2). The highest dependency MSRV is `bevy_ecs` at Rust 1.95, below the workspace MSRV.

## High-level structure

```text
┌────────────────────────────────────┐
│ game-cli                           │
│ process startup and composition    │
└─────────────────┬──────────────────┘
                  │
┌─────────────────▼──────────────────┐
│ game-tui                           │
│ terminal input, focus, rendering   │
└─────────────────┬──────────────────┘
                  │ commands / views
┌─────────────────▼──────────────────┐
│ game-app                           │
│ simulation facade and orchestration│
└───────────┬───────────────┬────────┘
            │               │
┌───────────▼────────┐  ┌───▼──────────────┐
│ game-core          │  │ game-persistence │
│ ECS simulation     │  │ saves and loading│
└───────────▲────────┘  └──────────────────┘
            │
┌───────────┴────────┐
│ game-content       │
│ validation/compile │
└────────────────────┘
```

## Cargo workspace

```text
Cargo.toml
crates/
  game-core/
    src/
      components/
      systems/
      resources/
      commands/
      events/
      queries/
      schedule.rs
      lib.rs
  game-content/
    src/
      definitions/
      loaders/
      validation/
      compile/
      lib.rs
  game-persistence/
    src/
      snapshots/
      migrations/
      lib.rs
  game-app/
    src/
      facade.rs
      views/
      lib.rs
  game-tui/
    src/
      input/
      screens/
      widgets/
      focus.rs
      render.rs
      lib.rs
  game-cli/
    src/main.rs
tests/
content/
```

The initial prototype includes `game-core`, `game-content`, `game-app`, `game-tui`, and `game-cli`. `game-persistence` remains deferred until save APIs become concrete.

## Dependency rules

Allowed dependency direction:

```text
game-cli ──► game-tui ──► game-app ──► game-core
game-cli ────────────────────────────► game-content
game-cli ────────────────────────────► game-persistence
game-content ────────────────────────► game-core
game-persistence ────────────────────► game-core
```

Rules:

1. `game-core` must not depend on `ratatui`, `crossterm`, filesystem APIs, or a specific content format.
2. `game-core` must be runnable in tests without a terminal.
3. TUI widgets must not mutate the ECS `World` directly.
4. The TUI must not receive unrestricted access to ECS storage.
5. File parsing and save formats must not leak into components or systems.
6. Executable crates perform dependency wiring; library crates expose capabilities.

## ECS simulation core

`game-core` owns:

- Components: entity-local state with little or no behavior.
- Resources: singleton world state and shared services.
- Systems: behavior operating on components and resources.
- Schedules: explicit ordering and simulation phases.
- Commands: requested changes entering the simulation.
- Events: facts produced by the simulation.
- Queries: read-only projections needed by the application layer.

### Scheduling

Use explicit schedules rather than allowing the TUI to invoke arbitrary systems. A simulation step should have named phases such as:

```text
receive commands
→ validate/resolve commands
→ run simulation systems
→ apply deferred ECS commands
→ publish events
→ produce views
```

System ordering should be declared centrally in `schedule.rs`. Systems should not depend on incidental registration order.

### Commands

Frontend input is translated into typed commands. The application layer submits those commands to the simulation.

```rust
pub trait Simulation {
    fn submit(&mut self, command: GameCommand);
    fn step(&mut self) -> StepResult;
}
```

Commands express intent and use stable domain identifiers where necessary. They must not contain terminal key codes, widget IDs, or layout state.

### Events

Systems emit typed events describing completed or rejected outcomes. Events support UI notifications, logs, diagnostics, and future integrations without coupling those consumers to systems.

Events are transient unless explicitly included in persistence or an event journal.

### Entity identity

Runtime ECS entity IDs are internal and ephemeral. They must not be used as persistent content identifiers.

Use separate stable identifiers for:

- Content definitions
- Save-file references
- External commands and integrations

Stable content IDs are validated string newtypes using a namespace-qualified form such as `core:system_01`. Mappings between stable IDs and ECS entities belong in core resources or persistence reconstruction code.

The economy uses a checked integer `Energy` newtype for physical energy stock, tank amounts, prices, costs, reservations, and settlements; floating-point values are not used for economic arithmetic. A market's `core:energy` inventory line is its only purchasing balance, while trader tanks and cargo-bay energy remain distinct physical stores. Three-dimensional map coordinates and derived Euclidean distances use finite `f64` values in prototype distance units. The enduring economy contract, deterministic phase order, and reconciliation rules are documented in [Energy-Denominated Economy](energy-economy.md).

## Application boundary

`game-app` is the public facade used by frontends. It owns orchestration but not game rules.

A minimal API should resemble:

```rust
pub trait GameSession {
    fn submit(&mut self, command: GameCommand) -> Result<(), CommandError>;
    fn step(&mut self) -> Result<StepReport, SimulationError>;
    fn view(&self, query: ViewQuery) -> ViewResult;
    fn drain_events(&mut self) -> Vec<GameEvent>;
}
```

The TUI consumes complete immutable view-model snapshots from `game-app`. View models are designed for presentation but contain no `ratatui` types. They retain stable IDs for commands while also resolving all player-facing names—including locations, cargo goods, route legs, and event-log labels—so frontends never need to display or look up internal content IDs. This keeps the same interface usable by tests, a graphical client, or a remote protocol adapter. Incremental view diffs are deferred until profiling demonstrates a need.

Structurally invalid requests fail immediately at the application boundary. Commands that are structurally valid but rejected by simulation rules produce typed rejection events.

## TUI adapter

`game-tui` owns only terminal concerns:

- Raw keyboard and terminal events
- Focus and navigation state
- Screen and modal state
- Layout
- Rendering
- Mapping input into application commands
- Mapping application views and events into visible output

The TUI has its own local UI state. Selection cursors, active tabs, scroll positions, and focus are not ECS components unless they have simulation meaning.

The TUI runs as an async adapter and concurrently awaits terminal input, application updates, and shutdown signals:

```text
tokio::select!
├── terminal input → update local UI state or submit a game command
├── application update → replace cached view model and render
└── shutdown signal → exit cleanly
```

The TUI renders immutable view-model snapshots received from the application layer. It must not hold locks on the ECS world or query it during rendering.

Terminal setup and restoration should use an RAII guard so raw mode and alternate-screen state are restored after errors or panics where possible.

## Data-driven content

Content files describe definitions, not live ECS state. Loading follows a staged pipeline:

```text
source files
→ deserialize into source definitions
→ schema and semantic validation
→ resolve cross-references
→ compile into typed runtime definitions
→ instantiate entities/components
```

Requirements:

- Human-readable content IDs are stable and globally namespaced.
- Duplicate IDs and unresolved references fail with source-aware diagnostics.
- Raw serialized values are converted into typed Rust values before reaching systems.
- Content definitions are immutable after compilation unless explicit hot-reload support is added.
- Format-specific parsing is isolated behind loader adapters.

A registry resource in the ECS world can hold compiled definitions and map stable content IDs to runtime templates. Population-level parameters that designers tune together—such as NPC trader count, common speed, starting tank energy/capacity, cargo capacity, travel burn, naming, and distribution—belong in dedicated validated configuration files such as `content/traders.ron`, rather than being duplicated across individual entity definitions.

## Persistence

Persistence is an adapter around the simulation rather than a concern embedded in each system.

Initial strategy:

1. Save versioned snapshots of persistent components and resources.
2. Store stable IDs instead of raw ECS entity IDs where references cross serialization boundaries.
3. Reconstruct the ECS world through a controlled load process.
4. Add migrations whenever the snapshot schema changes.

Save metadata should include at least:

- Save format version
- Application version
- Content version or fingerprint
- Random seed/state when required for reproducibility

Do not serialize the entire `bevy_ecs::World` blindly. Explicit persistence boundaries make migrations and transient-state handling manageable.

## Determinism and side effects

To keep the core testable and suitable for headless execution:

- Inject or store seeded random-number generators as ECS resources.
- Keep wall-clock access outside systems unless provided through an explicit resource.
- Keep filesystem, terminal, and network access outside `game-core`.
- Pass external results into the simulation through commands or resources.
- Define stable system ordering where order affects outcomes.

Perfect cross-platform determinism is not an initial requirement, but replayable tests on a supported target should be possible.

## Error handling and diagnostics

- Library crates expose typed errors using `thiserror`.
- `game-cli` uses `anyhow` to attach startup and operational context.
- Content errors should report file, definition ID, and field path when possible.
- Use `tracing` instead of writing diagnostics into the TUI directly.
- The executable chooses whether traces go to a file, stderr, or another subscriber.

## Testing strategy

### Core tests

- Construct a `World` with minimal fixtures.
- Submit commands and execute schedules without a TUI.
- Assert component/resource state and emitted events.
- Use fixed seeds for repeatable simulation tests.

### Content tests

- Validate all repository content in CI.
- Test duplicate IDs, unresolved references, and invalid values.
- Snapshot compiled definitions where useful.

### Application tests

- Exercise the facade using commands and view queries.
- Confirm that view models do not expose ECS or TUI implementation details.

### TUI tests

- Test key-to-intent mapping independently.
- Render stable views using `ratatui::backend::TestBackend`.
- Avoid depending on a real terminal in automated tests.

### Persistence tests

- Round-trip supported snapshots.
- Load older fixtures through migrations.
- Verify stable-reference reconstruction.

## Async execution model

The application is asynchronous from the first prototype, but individual ECS schedules remain synchronous and deterministic.

Use an actor-style owner for the simulation:

```text
TUI task
  ├── sends typed requests over a bounded Tokio channel
  └── receives the latest bounded-history view snapshot over a watch channel

simulation task
  ├── exclusively owns GameSession and the ECS World
  ├── processes one request at a time
  ├── runs synchronous ECS schedules
  └── publishes resulting events and immutable view snapshots
```

Rules:

- Exactly one task owns and mutates the ECS world.
- Never place the world behind a broadly shared `Arc<Mutex<_>>`.
- Use bounded channels to make backpressure explicit.
- Use Tokio `mpsc` for ordered requests and `watch` for the latest replaceable view snapshot, which includes bounded recent event history.
- Add a separate ordered event stream only when a concrete consumer requires lossless independent delivery.
- Commands that need acknowledgement use a request envelope with a Tokio `oneshot` sender.
- Blocking filesystem operations belong in `spawn_blocking` or dedicated adapter tasks.
- Systems do not spawn tasks or await futures. Async work completes outside the core and returns through typed commands.
- Cancellation and shutdown are explicit messages/signals, not dropped-task side effects.

This preserves a deterministic core while allowing terminal input, timers, persistence, networking, and future integrations to operate concurrently.

## Separate-process option

The initial application runs the TUI and simulation in one process with separate Tokio tasks. The command/query boundary must remain serialization-friendly enough that a future adapter could replace in-process channels with IPC or a network protocol.

A separate server process should only be introduced for a concrete requirement such as multiplayer, remote clients, process isolation, or a continuously running world.

## Implementation sequence

1. Create the Cargo workspace and initial four crates.
2. Establish a headless `game-core` world and explicit synchronous schedule.
3. Implement the async `game-app` actor, bounded request channels, and view publication.
4. Add a minimal async Ratatui event loop using `tokio::select!` and immutable view models.
5. Add typed RON content definitions, namespace-qualified stable IDs, and validation.
6. Add snapshot persistence after the first persistent components exist.
7. Add CI checks for formatting, Clippy, tests, and content validation.
8. Revisit crate boundaries after the first end-to-end vertical slice.

## Architectural acceptance criteria

The architecture is functioning as intended when:

- The simulation test suite runs without initializing a terminal.
- A command-line headless runner can drive the same simulation as the TUI.
- No core crate imports `ratatui` or `crossterm`.
- The TUI cannot directly mutate ECS components.
- Repository content can be validated without starting the game.
- Save loading reconstructs references without relying on prior runtime entity IDs.
- Replacing the TUI would not require rewriting simulation systems.
