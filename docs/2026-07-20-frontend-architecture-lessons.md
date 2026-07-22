---
title: Frontend Architecture Lessons from the Retired Prototype
type: reference
date: 2026-07-20
status: retained-knowledge
applies_to: user-facing application and terminal surfaces
---
# Frontend Architecture Lessons from the Retired Prototype

## Purpose

This document preserves implementation and interaction lessons considered when
rebuilding the playable application and terminal UI around the
origin-and-frontier game, and for future user-facing adapters.

It is **not a compatibility contract**. The retired crate APIs, activity model,
keybindings, view DTOs, and trader/market flows must not constrain the new
product. Git history remains the source for old implementation details; this
document retains only patterns worth reconsidering when their new use cases
exist.

Current architecture and migration authority remain
[Architecture](architecture.md), the
[Testing Stance](plans/2026-07-20-testing-stance-correction.md), and the
[Governance Sandbox](design/direction/foundations.md).

## Architectural lessons to retain

### Keep one owner of mutable simulation state

The former `game-app` used an actor-style owner task. It exclusively owned the
headless session, accepted typed requests through a bounded channel, advanced
simulation time, and published immutable views. This avoided exposing the ECS
world through a shared lock or allowing a frontend to invoke systems directly.

For a future application boundary:

- keep exactly one owner of mutable simulation state;
- use typed domain requests rather than terminal events or widget identifiers;
- keep request transport bounded so overload and backpressure are explicit;
- acknowledge requests that need a definitive accepted/rejected result;
- make shutdown an explicit request with a joined owner task;
- keep time advancement in the owner, not in the renderer; and
- test timing with a controllable clock rather than wall-clock sleeps.

The former implementation used Tokio `mpsc` for ordered bounded requests,
`oneshot` for acknowledgements, and `watch` for the latest replaceable view. The
pattern was useful; the exact request enum and channel API were not.

### Publish immutable, presentation-ready views

The TUI received complete immutable `ApplicationView` snapshots. It never
queried ECS storage or held a lock while rendering. This kept rendering
repeatable and made a future non-terminal frontend plausible.

A future view boundary should:

- expose stable domain IDs only where the frontend must send them back;
- resolve all player-facing names and labels before presentation;
- include typed availability, warning, rejection, and limiting-reason state;
- bound event/history collections deliberately;
- avoid leaking ECS entities, components, Ratatui types, or mutable handles; and
- prefer complete snapshots until profiling proves incremental diffs are needed.

Structural request errors may fail at the application boundary. Valid domain
intent rejected by simulation rules should remain a typed, observable outcome
rather than becoming a renderer-side rule.

### Separate domain intent from input routing

The former TUI translated `crossterm` key events into an intermediate
`InputAction` before dispatching application requests. Input precedence was
explicit:

1. terminal-size or unsupported-layout safety behavior;
2. active amount/confirmation layers;
3. help and other overlays;
4. global controls; and
5. context-specific activity controls.

This prevented a key intended for a modal from also triggering a global or
screen action. User-facing implementations should retain the separation and
precedence tests while choosing actions and bindings from current gameplay.

Selection cursors, focused sections, scroll positions, open layers, and similar
presentation state belong in frontend-local state. They should enter the core
only when they represent actual game state.

### Preserve player intent across refreshes and rejection

The prototype exposed a recurring UI-state lesson: background view updates and
unrelated inspection changes must not silently replace a pending player choice.
Route proposals and selections remained stable until explicitly cleared or
committed, and rejected commands left the proposal available for correction.

Apply that principle to future expedition, reclamation, construction, staffing,
or allocation flows:

- identify draft/proposal state explicitly;
- keep it stable across unrelated snapshots;
- clear it only on an explicit cancel or a successful transition that consumes
  it;
- do not replace it while an incompatible action is active; and
- show why a commit is unavailable or rejected without discarding the draft.

This is a UX principle, not a requirement to restore the old route API.

### Treat terminal setup and restoration as a resource lifecycle

The former TUI used a guard that tracked which terminal transitions had
succeeded and unwound them in `Drop`. Raw mode, alternate-screen entry, cursor
visibility, panic restoration, and ordinary shutdown were handled as one
lifecycle.

A future terminal adapter should preserve that discipline:

- restore only states that were successfully entered;
- restore on normal exit and recoverable errors;
- install panic restoration where practical;
- keep terminal operations injectable enough to test partial setup failures;
- avoid leaving raw mode or the alternate screen active after owner-task errors;
  and
- redraw on relevant input, view updates, and resize events without coupling
  rendering to simulation mutation.

### Make asynchronous responsibilities explicit

The former adapter concurrently awaited terminal events and updated views, while
the application owner concurrently handled requests and simulation ticks.
Useful rules were:

- systems remain synchronous and deterministic;
- async work completes outside `game-core`;
- missed real-time ticks use an explicit policy rather than an accidental burst;
- bounded request capacity and closed-channel behavior are tested;
- latest-view delivery may coalesce intermediate states, while any future
  lossless event consumer needs a separate ordered channel; and
- cancellation and shutdown are messages/signals, not dropped-task side
  effects.

The current playable adapter is synchronous and does not need an async owner.
Do not add Tokio merely to recreate the old shape; re-evaluate it only for a
concrete future responsibility.

## Terminal interaction and rendering lessons

### Support explicit layout classes

The old renderer had hand-tested compact (`80x30`) and regular (`160x45`)
layouts plus unsupported-size handling. The exact dimensions are historical,
but explicit layout classes were easier to reason about than allowing every
widget to improvise responsive behavior.

For terminal UI work:

- define the minimum supported terminal size deliberately;
- test at the minimum, immediately below it, and at a representative larger
  size;
- bound tables and lists with stable selected-row viewports;
- render position/overflow indicators when content is clipped;
- prevent large quantities and labels from overflowing fields; and
- keep unsupported layouts safe and non-destructive.

### Do not rely on color alone

Warnings, disabled actions, selections, read-only state, severity, and keyboard
shortcuts had textual or structural cues in addition to styles. Preserve this
accessibility baseline. Color and accent styling may reinforce meaning but must
not be its only carrier.

### Keep help and available actions contextual

The footer/help surfaces were generated from the active UI context rather than
showing every command everywhere. This reduced ambiguity and made precedence
visible. Future screens should disclose currently available actions and why an
action is unavailable, but must derive them from new domain capabilities rather
than copy old activity shortcuts.

### Use exact confirmation layers for consequential quantities

The former exact-amount layer displayed the requested quantity, maximum,
calculated consequence, and limiting reason before a transaction. The trading
semantics are obsolete, but the interaction pattern can serve construction,
resource allocation, expeditions, reclamation, or transfers:

- do not reuse an invisible previous quantity;
- show the current exact amount and relevant consequence;
- expose a computed maximum and its limiting reason;
- validate again at commit time in the simulation; and
- retain the layer and entered value after rejection when correction is
  possible.

## Testing lessons to retain

The old frontend had a large obsolete gameplay suite, but several testing
techniques remain useful:

- test key-to-intent routing independently from rendering and simulation;
- test modal/overlay precedence with keys that would otherwise trigger global
  or screen actions;
- use `ratatui::backend::TestBackend` or the contemporary equivalent for stable
  rendering assertions;
- render compact, regular, and below-minimum dimensions explicitly;
- assert textual fallbacks for semantic color cues;
- test long labels and extreme quantities at field boundaries;
- use paused async time for tick-rate and auto-pause behavior;
- test request acknowledgement, bounded-channel pressure, owner shutdown, and
  closed-owner errors;
- compare immutable views rather than inspecting ECS state from frontend tests;
  and
- keep end-to-end acceptance focused on the new origin-first player flow rather
  than recreating trader/market journeys.

Prefer semantic assertions over full-screen golden snapshots when a small
structural or textual oracle is sufficient. Broad render snapshots become
expensive when content and layout are still evolving.

## Knowledge not to retain as product requirements

Do not carry forward:

- the six activities named Systems, Trade, Logistics, Governance, Intelligence,
  and Encyclopedia;
- their function-key map or context shortcuts;
- market, trader, wallet, pricing, commercial-contract, or NPC-fleet DTOs;
- the assumption that every location has a populated inspection screen;
- trader travel and transaction dialogs as compatibility workflows;
- exact old view field names, channel capacities, event-history limits, or tick
  rates;
- `game-app`, `game-tui`, or `game-cli` public signatures; or
- old compact/regular dimensions without a fresh usability decision.

Patterns should be justified by the new frontend slice. Historical code is an
example to consult, not an API to restore.

## Removed dependency ledger

The following dependencies were direct dependencies of the retired user-facing
crates. Versions and features record the last prototype configuration, not
current pins. The rebuilt synchronous terminal uses Ratatui and Crossterm again;
Tokio, `anyhow`, and tracing remain absent.

| Dependency | Historical version/features | Former responsibility | Current disposition |
| --- | --- | --- | --- |
| `anyhow` | `1.0.103` | Startup and terminal-adapter error context in `game-cli` and `game-tui`. | Absent; libraries and adapters use typed errors. |
| `crossterm` | `0.29.0`, feature `event-stream` | Terminal backend, keyboard/resize events, raw mode, alternate screen, cursor and terminal control. | Reintroduced synchronously without `event-stream`, confined to `game-tui`. |
| `futures-util` | `0.3.31` | Stream helpers for asynchronous Crossterm event consumption. | Not used directly; add only for a concrete asynchronous responsibility. |
| `ratatui` | `0.30.2` | Terminal layout, widgets, styling, frame rendering, and `TestBackend`. | Reintroduced with minimal features, confined to `game-tui`. |
| `tokio` | `1.52.3`; features `macros`, `rt-multi-thread`, `sync`, `time`, `signal`, `test-util` | Application owner task, bounded requests, acknowledgements, latest-view publication, timers, signal/shutdown handling, and paused-time tests. | Absent; add only for an accepted async composition and never to `game-core`. |
| `tracing` | `0.1.44` | Structured startup/runtime diagnostics at the executable boundary. | Absent; reconsider only with a concrete diagnostic consumer. |
| `tracing-subscriber` | `0.3.23`; features `env-filter`, `fmt` | CLI installation of filtered formatted trace output. | Absent; any future subscriber selection belongs at the executable boundary. |

The removed internal dependency chain was:

```text
game-cli ──► game-tui ──► game-app ──► game-core
game-cli ────────────────────────────► game-content
```

This records separation of responsibilities, not required crate boundaries.
The current product recreated focused `game-app`, `game-tui`, and `game-play`
crates from concrete needs. No new crate or external dependency should be added
solely because it appeared in the prototype.

## Historical implementation pointers

The final pre-removal implementation is available immediately before commit
`c1e8c80` (`refactor(workspace): remove legacy product crates`). Useful paths
can be inspected without restoring them:

```bash
git show c1e8c80^:crates/game-app/src/lib.rs
git show c1e8c80^:crates/game-tui/src/input.rs
git show c1e8c80^:crates/game-tui/src/state.rs
git show c1e8c80^:crates/game-tui/src/lib.rs
git show c1e8c80^:crates/game-cli/src/main.rs
git show c1e8c80^:crates/game-app/Cargo.toml
git show c1e8c80^:crates/game-tui/Cargo.toml
git show c1e8c80^:crates/game-cli/Cargo.toml
```

Consult those files for behavior archaeology only. Do not copy them into the
working tree or create an archive. Any retained lesson promoted into a future
contract must receive a focused specification and tests against the
origin-and-frontier model.
