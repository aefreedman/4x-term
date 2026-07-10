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
| ECS | `bevy_ecs` without the full Bevy engine |
| TUI | `ratatui` |
| Terminal backend | `crossterm` |
| Serialization | `serde` |
| Initial content format | RON, YAML, or JSON through format-specific adapters |
| Error handling | `thiserror` in libraries; `anyhow` at executable boundaries |
| Logging and diagnostics | `tracing` and `tracing-subscriber` |

The exact human-authored content format can be selected after a small authoring experiment. Runtime code must not depend on that choice.

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

This is the intended separation, not a requirement to create every crate immediately. We can begin with `game-core`, `game-app`, `game-tui`, and `game-cli`, then extract content and persistence when their APIs become concrete.

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

Mappings between stable IDs and ECS entities belong in core resources or persistence reconstruction code.

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

The TUI consumes immutable view models from `game-app`. View models are designed for presentation but contain no `ratatui` types. This keeps the same interface usable by tests, a graphical client, or a remote protocol adapter.

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

The main loop is conceptually:

```text
poll terminal input
→ update local UI state or submit a game command
→ step the application when required
→ request immutable view models
→ render
```

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

A registry resource in the ECS world can hold compiled definitions and map stable content IDs to runtime templates.

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

## Separate-process option

The initial application runs the TUI and simulation in one process. The command/query boundary must remain serialization-friendly enough that a future adapter could replace direct calls with IPC or a network protocol.

A separate server process should only be introduced for a concrete requirement such as multiplayer, remote clients, process isolation, or a continuously running world.

## Implementation sequence

1. Create the Cargo workspace and initial four crates.
2. Establish a headless `game-core` world and explicit schedule.
3. Implement the `game-app` command/query facade.
4. Add a minimal Ratatui event loop using immutable view models.
5. Add typed content definitions and validation.
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
