---
title: "Stage 5: Origin-and-Frontier Playable Startup"
type: feature
status: completed
date: 2026-07-21
---
# Stage 5: Origin-and-Frontier Playable Startup

## Objective

Restore a truthful human-playable executable around the completed Stage 4b
origin-and-frontier simulation. Human play uses a terminal UI. The simulation
remains headless and data driven; the TUI is an input and presentation adapter,
not an owner of gameplay rules or hidden world state.

Deliver Stage 5 as focused sequential slices. Slice 5a settles the reusable UI
design foundation. Slice 5b then proves world generation, preview, and the
minimum bank/develop/bootstrap gameplay loop with manual tick advancement.
Later slices expose the already-implemented scouting and founding mechanics
without bundling every Stage 4b command into the first playable pass.
Agent-facing CLI interaction is deferred to a separate future stage so this
plan can focus exclusively on human interaction through the TUI.

## Implementation audit — 2026-07-21

Stage 5 is implemented, playable, and acceptance-complete. Checked boxes below
have current implementation, playtesting, deterministic test evidence, or an
explicitly approved scope decision. Unchecked boxes are historical orchestration
steps that were not performed and cannot be completed retroactively.

Several reviewed details were superseded by later follow-up work:

- dashboard and local navigation now use one visible selectable list rather than
  independent map/panel focus;
- anonymous position-derived frontier visuals replace the original strict
  no-position presentation while continuing to expose no neutral identity or
  facts;
- probe planning is target/route first, defaults to maximum capability, and
  applies an optional typed jump-limit override explicitly;
- the focused Start action is itself the explicit confirmation, replacing the
  separate confirmation overlay; and
- generic development enable/disable and active ship-position rendering extend
  the originally planned minimum surface.

Additional semantic renderer cases and cross-layer pre-receipt leak assertions
were explicitly waived: extensive playtesting covers the presentation flows and
deterministic core tests cover the hidden-information mechanics. The historical
per-slice branch/owner/commit sequence also was not followed; the feature landed
as a whole-stage delivery.

Validation rerun during this audit passed formatting, workspace all-target and
all-feature checking, Clippy with warnings denied, all-feature tests (110
passed, none ignored), metadata/dependency inspection, and `git diff --check`.
No Tokio, `anyhow`, or tracing dependency is present.

## Authoritative context

| Concern | Source |
| --- | --- |
| Stage 5 direction and testing stance | [Testing Stance Correction](2026-07-20-testing-stance-correction.md) |
| Current crate and player-view boundary | [Architecture](../architecture.md) |
| Governance-game identity and core loop | [Governance Sandbox](../2026-07-20-design-direction-governance-sandbox.md) |
| Retained frontend lessons | [Frontend Architecture Lessons](../2026-07-20-frontend-architecture-lessons.md) |
| Generated-world contracts | [World Generation](../design/world-generation.md) |
| Profile and generator identity | [Generator Identity](../design/generator-identity.md) |
| Origin infrastructure | [Systems and Resources](../design/systems-and-resources.md) |
| Population bootstrap | [Population and Habitats](../design/population-and-habitats.md) |
| Player-safe knowledge | [Scouting and Knowledge](../design/scouting-and-knowledge.md) |
| Probes, expeditions, and founding | [Ships and Expansion](../design/ships-and-expansion.md) |
| Active invariant evidence | [Engine Invariant Registry](../2026-07-20-engine-invariant-registry.md) |
| Implemented dependency | [Stage 4b Plan](2026-07-20-feature-constructive-world-generation-stage-4b-plan.md) |

## Starting boundary

The workspace contains only `game-core` and `game-content`.

- `game-core` owns deterministic state, validated commands, the global atomic
  tick, and the knowledge-filtered `PlayerWorldView`.
- `game-content` owns strict profile loading, canonical fingerprints,
  generation identity, and `GeneratedWorldArtifact`.
- `content/profiles/starter.ron` is an editable baseline, not a canonical
  universe or an implicit startup contract.
- There is no application/session crate, executable, CLI, TUI, persistence, or
  replay format.
- Stage 4b already implements construction, Habitat controls, Shipyards,
  probes, expeditions, delayed knowledge, founding, and loss. Stage 5 adapts
  selected existing mechanics for play; it does not reimplement them in the
  frontend.

## Settled product decisions

1. The TUI is the human play surface.
2. The game remains data driven. Widgets and key handlers do not contain
   recipes, resource identities, simulation rules, or hidden world facts.
3. The first implementation slice after UI preparation includes generated-world
   selection and preview plus a basic playable origin loop. It need not expose
   every Stage 4b feature.
4. Ticks can be advanced manually. Real-time ticking is not required in this
   stage.
5. Agent-facing CLI interaction is outside Stage 5 and will receive a separate
   plan. Stage 5 does not need to define a stable non-interactive command or
   output protocol.
6. Persistence and full event-log replay remain Stage 6 work.
7. There is no compatibility path from the retired trader game.
8. The Stage 5 TUI targets a `160x45` terminal-cell canvas. This is sized to
   fit a fullscreen 1920x1080 display with cells up to 12x24 pixels. Stage 5
   does not require a compact gameplay layout below this size.
9. Directional navigation always supports arrow keys and also supports a
   player-selected keyboard layout. The initial layouts are QWERTY (`hjkl`)
   and Colemak-DH `unei` as a WASD-style directional cluster: `u` up, `n`
   left, `e` down, and `i` right. Help and contextual hints reflect the active
   layout. Keyboard mode is a global TUI user setting, independent of profile,
   seed, generated identity, and session. It may be changed from global
   settings on any surface; machine-local persistence is not required in this
   stage.
10. The frontier overview presents a synchronized map and system list derived
    from player-safe state. Charted systems expose admitted identity and exact
    position. Anonymous frontier fog may use position-derived visual points, but
    must not associate them with hidden system identities or facts.
11. Multi-tick advancement presents intermediate tick changes at a selectable
    pace of 1, 5, or 10 ticks per second, defaulting to 5. Space pauses or
    resumes between ticks, Enter advances one tick while paused, and Esc stops
    between ticks. This is presentation pacing for a manual command, not
    autonomous or real-time simulation.
12. Construction begins from a selected empty body slot.
13. Energy receives a dedicated information render rather than being shown
    only as one row in the general stock list.
14. The overall TUI design sensibility draws from *Caves of Qud* first and
    *Dwarf Fortress* second. Use them as references for atmosphere,
    information density, keyboard-first inspectability, map-and-panel
    composition, and systemic legibility—not as layouts, terminology,
    keybindings, color palettes, or widgets to copy literally.
15. A player may assign or clear a personal alias for a charted system. The
    stable survey-catalogue label remains available for disambiguation. Alias
    state belongs to the application session, not TUI-local selection state or
    the generated world definition.
16. A rejected construction command explicitly returns `Retain` or
    `InvalidateRoot`. Retainable rejection preserves the draft and returns to
    it from the concise overlay; invalidated roots close the draft and return
    to the refreshed slot list. Accepted construction closes the draft.

## Target architecture

The concrete need to keep human interaction separate from simulation ownership
justifies a small application/session boundary:

```text
human-play executable ──► game-tui ──► game-app ──► game-core
          └───────────────────────────────────────► game-content
```

The implementation uses these concrete workspace packages and dependency
edges:

```text
crates/game-play (package game-play, binary 4x-term)
├──► game-tui
└──► game-app
      ├──► game-content
      └──► game-core

game-tui ──► game-app
game-content ──► game-core
```

`game-tui` does not depend directly on `game-core`; `game-app` exposes the
stable IDs required by frontend intents. The responsibility boundaries are
contractual:

### `game-app`

- exclusively owns the mutable `WorldState` for a running session;
- composes generated artifact metadata with player-safe runtime views;
- accepts typed domain intents and returns typed accepted/rejected outcomes;
- creates immutable, presentation-ready views;
- composes player-facing labels from loaded content with core-owned costs,
  availability assessments, and limiting reasons;
- never exposes unrestricted mutable core state; and
- remains independent of terminal libraries and terminal events.

The first implementation is synchronous. Manual tick advancement provides no
concrete need for Tokio, channels, background tasks, or a simulation clock.
`game-tui` uses only an injectable monotonic presentation clock to pace an
explicit multi-tick command.

### `game-tui`

- translates terminal input into typed application intents;
- owns selection, focus, scrolling, overlays, drafts, and other presentation
  state;
- renders immutable application views;
- does not depend directly on hidden world definitions or privileged
  `test-support` snapshots;
- does not infer command legality by copying core rules; and
- preserves a pending player choice after a rejected command when correction
  remains possible.

### Human-play executable

- load an explicit profile and seed through the TUI startup flow;
- compose generation, application session, and terminal adapter;
- report startup/content/generation failures clearly; and
- contain only process startup and shutdown concerns, leaving gameplay intent
  and presentation behavior in their owning layers.

### Dependency choices

Use synchronous terminal integration only:

- `ratatui = 0.30.2` in `game-tui`, with default features disabled and the
  `crossterm_0_29` backend feature enabled;
- `crossterm = 0.29.0` in `game-tui` for synchronous input, resize events, raw
  mode, alternate screen, and cursor lifecycle;
- `unicode-width = 0.2.2` where alias validation and terminal-cell truncation
  require the approved display-cell contract; and
- existing workspace `thiserror = 2.0.18` for typed library/adapter errors.

Do not add Tokio, channels, `anyhow`, tracing, or asynchronous event streams in
Stage 5. Crates.io metadata confirms these versions support the workspace's
Rust 1.97 toolchain (`ratatui` requires 1.88, `crossterm` 1.63, and
`unicode-width` 1.66). The first manifest task must still compile the selected
minimal feature set before implementation fans out.

## Cross-slice constraints

- All runtime presentation uses `PlayerWorldView` or an application view
  derived from it. Production code must not enable `test-support`.
- Generation preview may use public `GeneratedWorldArtifact` identity,
  provenance, and definition data before a session starts. It must label this
  as pre-play generation information and must not turn complete generated
  definitions into an in-session hidden-information bypass.
- Starting a session instantiates exactly the generated origin community and
  neutral frontier supplied by Stage 4b. No market or NPC population is added.
- Core commands remain validate-before-mutate. Application and TUI rejection
  handling must not simulate speculative mutations or silently discard intent.
- Resource and development behavior comes from the selected profile. The TUI
  must not special-case `core:energy`, `core:ore`, `core:alloy`, recipe costs,
  or generator quantities as presentation logic.
- Tests use deterministic authored fixtures or explicit generated requests.
  They do not judge seed quality, frontier connectivity, solvency, or survival.
- Add external dependencies only for a concrete adapter responsibility and
  keep them out of `game-core` and `game-content`.

## Implementation orchestration

Stage 5 remains sequential across slices: 5b must pass its gate before 5c
begins, and 5c must pass before 5d begins. Within a slice, delegate only the
explicit parallel lanes below.

- [ ] Create one integration branch/worktree for the active slice.
- [ ] Assign one integration owner for root manifests, `Cargo.lock`, shared
      crate exports, architecture docs, README, and changelog.
- [ ] Land the slice's scaffold and public contract before creating parallel
      implementation branches.
- [ ] Give each delegated agent exclusive module directories and separate test
      files; test-only agents report implementation defects rather than editing
      another lane's source.
- [ ] Merge and validate each lane through the integration owner; reassign fixes
      serially after merge instead of allowing opportunistic cross-lane edits.
- [ ] Close each slice with the listed gate before delegating work from the next
      slice.

These orchestration boxes remain unchecked intentionally: implementation landed
on one whole-stage branch and the per-slice ownership and gate sequence cannot
be reconstructed after the fact.

Recommended agent roles are **integration**, **core boundary**, **application**,
**TUI state/input**, **renderer**, **terminal adapter**, **acceptance tests**, and
**documentation**. A role describes ownership, not a permanent agent identity.
Shared model/export files always have one owner at a time.

## Slice 5a — UI design foundation

Complete and review the minimum human-interaction foundation before
implementation agents choose crate APIs, Ratatui widgets, or input handlers.
The goal is not to pre-design every screen. Approve critical reference screens,
a reusable UI design system, and an element classification that lets later
screens be composed consistently without requiring agents to invent a new
visual language.

Markdown and exact-cell ASCII wireframes are sufficient; this phase should not
build a throwaway TUI or make a generated seed into a gameplay acceptance
target. Keep draft review artifacts as planning supplements under `docs/plans/`
and link them from this plan. They move into durable `docs/design/`
documentation only if they are approved and expected to persist beyond this
implementation plan. The supplements may be separate documents or one reviewed
Slice 5a TUI specification, but together they must cover the following
contracts.

Slice 5a planning supplements:

- [Stage 5a TUI Design Foundation Supplement](2026-07-21-stage-5a-tui-design-foundation-supplement.md)
- [Stage 5a TUI Reference Wireframes Supplement](2026-07-21-stage-5a-tui-reference-wireframes-supplement.md)

### Surface inventory and navigation map

Inventory the required Slice 5b surfaces and their transitions without fully
designing each one:

- profile/seed selection;
- content or generation failure;
- generated-world preview and start confirmation;
- frontier overview;
- origin/system detail;
- bodies, slots, stocks, and construction queue inspection;
- construction role and Extractor-target selection;
- Habitat controls;
- accepted/rejected command feedback;
- charted-system rename/clear-alias interaction;
- contextual help;
- global TUI user settings, including keyboard mode; and
- below-minimum terminal-size behavior.

For each surface, record its entry/exit path, default focus, available actions,
and classification as a full-screen shell, standard panel composition, overlay,
confirmation, message, or safety state. Detailed dimensions and widget choices
are required only for critical reference screens or where the design system
cannot determine the answer.

### UI design system and element classification

Begin with a short reviewed reference-sensibility note. *Caves of Qud* is the
primary reference: favor a strong map-centered composition, compact but
readable side information, evocative terminal texture, clear focus, and
keyboard-first contextual interaction. *Dwarf Fortress* is the secondary
reference for dense systemic information, inspectability, and the ability to
drill from an overview into exact state. Preserve this project's accessibility
and architecture constraints: meaning cannot depend on color, dense data must
retain hierarchy, and presentation must not leak hidden simulation state.
Record which principles are being adopted and which tempting reference patterns
are intentionally rejected. Do not require implementation agents to infer the
reference games' design from memory.

Define a small reusable terminal design system for the `160x45` canvas. It must
cover:

- screen shell, title/global-action regions, panel-local action bars, panel
  grid, spacing, borders, and focus;
- list, table, detail, metric, resource, progress, form-field, action,
  key-hint, message, and map elements;
- overlays, confirmations, errors, warnings, disabled states, empty states, and
  below-minimum safety presentation;
- default, focused, selected, pending, accepted, rejected, and unavailable
  variants where applicable;
- truncation, scrolling, selected-row viewport, and overflow indicators; and
- semantic text markers so color is never the only carrier of meaning.

Create a classification matrix mapping every inventoried Slice 5b element to a
design-system component and variant. An implementation agent may compose an
unwireframed surface from approved components, but must not create a new
component, state language, or navigation pattern without recording the gap for
review.

### Critical `160x45` reference wireframes

Create reviewed exact-cell wireframes only for the screens that establish the
system's composition and highest-risk interactions:

1. startup/profile/seed entry and generated-world preview;
2. the main-play dashboard with synchronized knowledge-filtered frontier map
   and system list, selected-system detail, and dedicated Energy render;
3. slot-initiated construction through role/Extractor-target confirmation,
   including a correctable rejection; and
4. manual multi-tick advancement with intermediate changes and interruption.

Use these reference screens to prove the design system against representative
edge states such as long labels, large quantities, empty and overflowing lists,
no available action, and source-aware errors. Other Slice 5b surfaces may use
component-level examples and classification rather than bespoke full-screen
wireframes.

### Interaction grammar

Approve a keybinding table and explicit input precedence for:

1. the unsupported-size safety view;
2. active confirmation/value-entry layers;
3. help and other overlays;
4. global controls; and
5. focused-panel actions.

Record which actions execute immediately, which open a draft or confirmation,
and how focus, selection, back, cancel, help, manual tick advancement, and quit
behave. Arrow keys remain universal. Define QWERTY `hjkl` and Colemak-DH
`unei` (`u` up, `n` left, `e` down, `i` right) layout mappings through one
semantic navigation layer so widgets never match
layout-specific keys directly. Global user settings own layout selection, and
all contextual hints must use the active mapping.

### Information hierarchy and formatting

Classify player information as always visible, visible on inspection,
contextual help, or intentionally omitted. Define formatting rules for ticks,
seasons, quantities, progress, player-facing seed/profile identity, labels
versus stable IDs, truncation, disabled actions, warnings, errors, selection,
and scroll position.

Design the dedicated Energy render around player-safe evidence: current Energy,
capacity, headroom, current seasonal position, last-tick life-support demand and
payment, shortage, supported/underserved population, and retention overflow.
Any production forecast or derived runway shown later must be supplied as an
application/core value; the TUI must not copy seasonal or resource arithmetic.

### Command-flow state machines

Specify the UI states and transitions for:

- generation, regeneration, preview, and explicit session start;
- slot-initiated construction, including Extractor resource targeting and
  confirmation;
- Habitat generation controls;
- one-tick and bounded multi-tick advancement, including intermediate
  per-tick presentation and interruption behavior; and
- quit/cancel behavior.

Each command flow must define accepted and rejected outcomes. Correctable
rejection retains the player's draft and selection.

### Application-view and intent contract

List the exact presentation data and typed intents required by the approved
critical screens and classified elements. The contract must identify which
layer resolves labels, costs, availability, and limiting reasons, and must prove
that the TUI neither reads hidden runtime state nor duplicates simulation rules.
Add only fields used by an approved Slice 5b screen, component, or action.

### UX acceptance walkthroughs

Walk through deterministic authored scenarios for generation/startup,
inspection, valid construction, correctable rejection, manual advancement,
Habitat bootstrap, below-minimum resize/recovery, and clean quit. Exact UI tests
should use small hand-computable fixtures; generated previews may use explicit
seeds only to verify identity and deterministic presentation, not world quality.

### Design-lock checklist

- [x] Approve the surface inventory and navigation map.
- [x] Approve the *Caves of Qud*/*Dwarf Fortress* reference-sensibility note.
- [x] Approve the UI design system and element classification matrix.
- [x] Approve the critical `160x45` reference wireframes and component edge
      states.
- [x] Approve keybindings and input precedence.
- [x] Approve information hierarchy and formatting rules.
- [x] Approve command-flow state machines.
- [x] Approve typed intents and application-view fields.
- [x] Walk through all Slice 5b UX acceptance scenarios. The reviewed contracts
      include `Retain` and `InvalidateRoot` branches, deterministic paced-batch
      controls with a controllable clock, stopped resize recovery, and exact
      Habitat/population presentation fields.
- [x] Record deferred UI questions explicitly.
- [x] Link the approved design artifacts from this plan.

Slice 5b implementation should not begin until this checklist is complete or a
specific unchecked item is explicitly accepted as implementation-owned rather
than product-design-owned. Implementation agents may make local composition
choices for classified elements within the approved design system; unresolved
product behavior, new component types, and new interaction patterns require
review.

## Slice 5b — Generated-world preview and minimal playable TUI

### Goal

Prove the complete human-facing startup and minimum gameplay path:

```text
profile + seed
→ generate world
→ preview generation
→ start as the origin governor
→ inspect player-visible state
→ issue core commands
→ manually advance ticks
→ observe results and typed rejections
```

This slice tests whether the bank/develop/bootstrap loop is understandable and
operable. It deliberately does not require every Stage 4b command.

### Included

#### Startup and preview

- Select or enter an unsigned 64-bit seed.
- Select an explicit profile path, with `content/profiles/starter.ron` offered
  as a convenience default by the executable rather than embedded in the core.
- Load, validate, canonicalize, and fingerprint the profile through
  `game-content`.
- Generate `core:frontier_world@1` through the public generation API.
- Preview at least:
  - seed;
  - player-facing profile identity;
  - origin identity and the constructed origin scaffold; and
  - clear generation/content failures.
  Generator revision, fingerprint, provenance, aggregate count, and other
  reproduction/debug metadata remain internal rather than player-facing.
- Permit regenerate/repreview with a different seed before starting.
- Start a fresh `WorldState` only after explicit player confirmation.

#### Minimum gameplay surface

- Inspect world time through a knowledge-filtered frontier overview containing
  a synchronized map and system list.
- Assign or clear a player-facing alias for a charted system while retaining
  its stable survey-catalogue label.
- Inspect the origin's stocks, a dedicated Energy render, bodies, slots,
  developments, body resources, construction queue, population-facing state,
  and Habitat controls.
- Initiate construction from a selected empty slot and enqueue the core
  development roles supported by current data, including an explicit Extractor
  resource target where required.
- Enable or disable automatic population generation on a functional empty
  Habitat.
- Advance exactly one tick manually.
- Advance a small explicit tick count as a convenience while preserving and
  presenting intermediate changes from each atomic core tick and
  stopping/reporting on rejection or player interruption.
- Display accepted outcomes and typed rejection reasons.
- Keep selection or draft state after a correctable rejection.
- Provide contextual help and a safe quit path.

#### Presentation baseline

- Use `160x45` terminal cells as both the minimum supported gameplay size and
  the primary design/test canvas for a fullscreen 1920x1080 display.
- Permit larger terminals by allocating or centering additional space without
  changing gameplay semantics.
- Below `160x45`, show a safe size requirement view and do not dispatch
  gameplay commands. A compact gameplay layout is outside this stage.
- Do not rely on color alone for selection, warnings, or disabled actions.
- Restore terminal state after normal exit, recoverable startup/runtime errors,
  and partial terminal setup failure where testable.
- Prefer semantic render assertions over full-screen golden snapshots.

### Excluded

- probe launch, route visualization, and delayed scouting reports unless they
  remain a clearly bounded addition after the minimum loop is complete;
- expedition construction, founding, or founding loss;
- save/load, autosave, lineage, or resume;
- real-time or background tick advancement;
- async application ownership;
- generated-world quality scores or reroll recommendations;
- unrestricted map truth after play starts; and
- any agent-facing CLI, scripting protocol, or structured automation output.

### Application-view contract work

`PlayerWorldView` contains stable IDs, knowledge, missions, redacted routes, and
commandable local snapshots, but it does not by itself provide every label,
action availability, cost, or startup identity needed by a player interface.
Slice 5b must define a focused application view rather than letting the TUI
inspect private runtime state or duplicate profile rules.

The view should expose only presentation-relevant data, including:

- generation/session identity needed in the UI;
- resolved resource and location labels;
- synchronized knowledge-filtered map and system-list entries;
- origin and selected-system summaries;
- dedicated player-safe Energy information;
- body/slot/development rows;
- queues and progress;
- population/Habitat status;
- typed available actions with costs or limiting reasons; and
- the latest accepted/rejected application outcome.

Do not promise a permanent all-features DTO in this slice. Add fields only for
an implemented screen or command.

### Acceptance journey

A deterministic TUI-focused journey must demonstrate:

1. Load `content/profiles/starter.ron` with an explicit seed.
2. Show generation identity and an origin-scaffold preview.
3. Change the seed and regenerate without entering play.
4. Start at the sole living origin community.
5. Show no hidden neutral-system runtime state in the play view.
6. Navigate the synchronized frontier map and list without associating
   anonymous frontier visuals with hidden system identity or facts.
7. Inspect the dedicated Energy render plus origin stocks, bodies,
   developments, and available slots.
8. Start from an empty slot and queue one valid development through a typed
   application intent.
9. Attempt one invalid action and show its reason without mutating session
   state or discarding the current correctable selection.
10. Advance ticks manually and observe intermediate multi-tick changes.
11. Observe updated time, Energy evidence, stocks, queue progress, completed
    infrastructure, and population/Habitat-facing state through immutable
    views.
12. Switch between QWERTY and Colemak-DH navigation while arrow keys continue
    to work.
13. Quit with terminal state restored.

### Focused tests

- deterministic generation preview from explicit profile bytes and seed;
- profile/read/generation errors remain source aware;
- starting play instantiates the previewed definition and identity;
- application intents map to the intended core commands;
- rejected intents leave authoritative state unchanged;
- application views do not expose neutral local runtime state;
- one-tick and bounded multi-tick manual advancement with intermediate views
  and interruption;
- synchronized map/list selection without hidden-knowledge leakage;
- charted-system alias assignment, clearing, validation, and catalogue-label
  fallback;
- dedicated Energy rendering from application-provided values;
- arrow, QWERTY `hjkl`, and Colemak-DH `unei` navigation through one semantic
  input mapping, with the directional meaning of every key asserted;
- input routing and modal/overlay precedence;
- the `160x45` design canvas, representative larger layouts, and the safe
  below-minimum view at `159x45` and `160x44`;
- textual cues for warning/disabled/selection states; and
- terminal lifecycle restoration with an injectable boundary where practical.

### Implementation task graph

#### 5B-0 — Integration scaffold (serial)

- [x] Add `crates/game-app`, `crates/game-tui`, and `crates/game-play`; configure
      package `game-play` to produce binary `4x-term`.
- [x] Add the approved workspace dependencies and minimal Ratatui/Crossterm
      features; update `Cargo.lock` under the integration owner only.
- [x] Verify the graph with Rust 1.97 using workspace metadata,
      all-target/all-feature check, and dependency inspection. Confirm no Tokio
      or terminal dependency enters `game-core`, `game-content`, or `game-app`.
- [x] Freeze module ownership and app re-exports so `game-tui` can use stable
      intent IDs without a direct `game-core` dependency.
- [x] Define `ProfileDescriptor { machine_path, logical_source_id,
      display_name }`. Derive Stage 5's player-facing name from the selected
      filename stem, keep the machine path startup-only, and never render
      provenance or path in play.

#### 5B-1 — Parallel foundation lanes

Core-boundary lane; owns only `game-core` projection code and its tests:

- [x] Add a player-safe local population projection containing derived
      population count and occupied Habitat slot coordinates only where local
      state is already commandable, plus the current core-derived seasonal
      phase required by the Energy view.
- [x] Factor construction and Habitat-toggle validation into private plans
      shared by read-only assessment and atomic commit. Assessments expose only
      player-safe role/target eligibility, exact costs, availability, and typed
      limiting reasons; labels and copy remain application-owned.
- [x] Test population projection at origin tick zero, Habitat generation, and
      expedition transitions available to existing fixtures, while excluding
      token identities, transit state, and neutral local runtime.
- [x] Test assessment non-mutation and commit agreement for every construction
      role, occupied/reserved slots, Extractor targets, insufficient resources,
      Habitat occupancy/state, and stale-state revalidation.

Terminal-scaffold lane; owns only `game-tui` terminal abstractions and tests:

- [x] Prove the selected Ratatui/Crossterm feature set with a minimal
      `TestBackend` render and synchronous event abstraction.
- [x] Define an injectable terminal-operations boundary and staged guard before
      acquiring raw mode, alternate screen, or hidden cursor state.
- [x] Define an injectable monotonic clock suitable for deterministic
      1/5/10-ticks-per-second tests without wall-clock sleeps.

#### 5B-2 — Application contract gate (serial)

- [x] Define startup coordinator state, preview staleness, explicit
      confirmation, and exact artifact-to-session consumption. The later TUI
      flow treats activation of the focused Start action as that confirmation
      rather than opening a second overlay.
- [x] Define the approved startup/session intents, `DraftDisposition`, typed
      outcomes, action availability/reasons, `PlayingView`, `EnergyView`,
      construction/Habitat views, and `TickStepView`.
- [x] Define FSC catalogue labels, session-owned aliases, 32-display-cell alias
      validation, and resolved collection/detail labels.
- [x] Review exported DTO fields against the approved allowlist; add fixture
      tests proving hidden IDs/values do not enter projections and dependency
      inspection proving terminal types remain outside `game-app`.
- [ ] Commit the compiling contract before delegating application and TUI-state
      implementation. **Historical deviation:** contracts and implementation
      landed together in the whole-stage delivery.

#### 5B-3 — Parallel application and TUI-state lanes

Application lane; owns `game-app` startup/session/projection modules:

- [x] Implement profile loading, source-aware errors, allowlisted preview,
      stale-preview behavior, explicit start, and sole mutable `WorldState`
      ownership.
- [x] Map core construction/Habitat assessments and commit commands to
      player-facing labels/outcomes, returning `Retain` or `InvalidateRoot`
      without reconstructing rules or parsing error text.
- [x] Implement immutable player-safe projections for map/list synchronization,
      Energy, stocks, bodies/slots, queues, local population/Habitats, actions,
      aliases, and limiting reasons.
- [x] Implement one-tick outcomes and ordered player-visible deltas; keep
      multi-tick as repeated application `AdvanceOneTick` calls. Each retained
      step includes the resulting immutable view; richer keyed knowledge/report
      summaries remain follow-up work.

TUI-state/input lane; owns semantic input, local state, drafts, and clock logic:

- [x] Implement arrows plus QWERTY `hjkl` and Colemak-DH `unei`, global settings,
      focused-editor precedence, contextual actions, and help.
- [x] Implement map/list shared selection, focus/scroll retention, alias editor,
      slot-first construction draft, Habitat confirmation, and typed rejection
      recovery. View refresh now retains the stable selected system ID even when
      newly admitted knowledge changes sorted rows.
- [x] Implement startup/preview/start confirmation, quit confirmation, and the
      undersized safety state without dispatching gameplay intents. **Superseded
      interaction:** focused Start activation directly confirms the current
      preview; there is no separate Start overlay.
- [x] Implement paced multi-tick state for count `1..100`, rates 1/5/10 with
      default 5, pause/resume, paused single-step, stop, rejection, resize-stop,
      and retained intermediate history using the injectable clock.
- [x] Test paced controls and resize recovery with deterministic durations;
      keep core/app atomic tick tests independent of presentation timing.

#### 5B-4 — Parallel presentation, lifecycle, and adapter-test lanes

Renderer lane; owns `game-tui/src/render/**` and semantic render tests:

- [x] Implement the approved component language and `160x45` compositions,
      deterministic larger viewports, clipping/scrolling, and textual semantic
      cues. **Superseded composition:** later one-focus navigation, uncertainty
      visuals, and active ship markers replace portions of the reference
      wireframes; durable behavior is recorded in `docs/tui-ux-guidelines.md`.
- [x] Accept the existing semantic `TestBackend` coverage for startup,
      dashboard, uncertainty/ship visuals, and size boundaries. Additional
      rejection, batch, alias, and extreme-content render cases are explicitly
      waived based on extensive playtesting; broad goldens remain excluded.

Terminal lane; owns terminal lifecycle/event-loop code and dedicated tests:

- [x] Implement staged RAII cleanup in reverse acquisition order, normal/error
      shutdown, synchronous resize/input polling, and unwind restoration through
      owned guards.
- [x] Force failure after each setup stage and assert cleanup without a real
      TTY.

Application-test lane; owns separate black-box test files:

- [x] Cover preview identity/staleness, exact preview consumption, neutral-state
      exclusion, aliases, Energy evidence, construction `Retain` and
      `InvalidateRoot`, Habitat bootstrap, and rejected-command atomicity. The
      application bootstrap test banks Ore, refines Alloy, expands Energy
      capacity, constructs and enables a Habitat, and observes its first resident.
- [x] Cover sequential one-tick outcomes and a later rejected tick at the
      application boundary; assert prior steps commit and the rejected step does
      not, without testing presentation cadence here.

#### 5B-5 — Integration and gate

- [x] Compose `4x-term` with the starter-profile convenience path, synchronous
      app/TUI loop, startup/runtime diagnostics, and safe shutdown.
- [x] Complete the 13-scenario Slice 5b acceptance journey through extensive
      manual playtesting, supplemented by the focused deterministic coverage.
- [x] Update `docs/architecture.md`, README startup instructions, relevant
      invariant references, and `CHANGELOG.md` under `Unreleased`.
- [x] Run formatting, workspace all-target/all-feature check, Clippy with
      warnings denied, all-feature tests with no ignored tests, and dependency
      inspection. Revalidated during the implementation audit.
- [ ] Record the 5b gate as passed before starting any Slice 5c branch.
      **Historical deviation:** the slices landed together, so this sequencing
      gate was not observed.

## Slice 5c — Scouting loop

### Goal

Expose the implemented probe loop while preserving the player knowledge
boundary.

### Included

- inspect functional Shipyards, their queues, and completed probe assets;
- enqueue and cancel unstarted probe projects;
- choose only knowledge-valid targets;
- plan target and route using maximum probe capability by default, with an
  optional explicitly applied jump-limit override within profile-authored bounds;
- preview only the redacted route available to the player;
- launch a completed probe;
- display active redacted routes and mission state;
- manually advance ticks through travel, observation, and delayed report
  receipt; and
- update the frontier view as knowledge changes.

### Acceptance focus

A short deterministic fixture or explicit generated request demonstrates that
an uncharted/identified frontier changes only through approved observations and
transmissions. The TUI must not name hidden intermediate systems before the
player boundary reveals them.

### Deferred

- expeditions and founding;
- deep survey layers beyond Stage 4b;
- route optimization advice; and
- real-time ticking.

### Implementation task graph

#### 5C-0 — Core scouting-query gate (serial)

- [x] Refactor probe launch validation into one private plan shared by assessment
      and commit; do not duplicate routing, knowledge, jump-limit, asset, or
      Energy rules.
- [x] Add a read-only probe-launch assessment returning only approved target
      eligibility, authored jump bounds, exact cost/readiness, typed limiting
      reason, and `RedactedRoute`. Commit must revalidate atomically.
- [x] Add the minimum player-safe pending-report status needed after a probe is
      consumed and before delayed knowledge arrives. Expose mission identity and
      `AwaitingReport` only, never pending report contents, hidden stops, or
      authoritative receipt internals.
- [x] Test that assessment never mutates state, assessment and successful launch
      agree, failures match commit-time validation, and hidden route stops remain
      `None`.

#### 5C-1 — Scouting contract gate (serial)

- [x] Extend app contracts with Shipyard queue/probe-asset rows, enqueue and
      unstarted-cancel intents, knowledge-valid targets, effective jump limit,
      probe assessment, launch outcome, active redacted routes, missions, and
      awaiting-report state. **Superseded interaction:** planning defaults to
      maximum capability and exposes an optional explicitly applied override
      instead of requiring an up-front jump-limit choice.
- [x] Compose all 5c surfaces from the established Stage 5 component language;
      later target/route-first interaction is recorded in
      `docs/tui-ux-guidelines.md`.
- [ ] Commit the compiling contract and module exports before parallel lanes.
      **Historical deviation:** this separate gate commit did not occur.

#### 5C-2 — Parallel application and TUI lanes

Application scouting lane; owns a dedicated `game-app` scouting module:

- [x] Map existing enqueue/cancel/launch commands and the new assessment into
      typed outcomes and immutable views without retaining hidden route nodes.
- [x] Derive frontier state from admitted knowledge changes and represent the
      post-arrival/pre-receipt gap as awaiting report rather than fabricating a
      report history. Each tick step retains its immutable resulting view;
      richer keyed knowledge/report delta summaries remain follow-up work.

TUI scouting lane; owns dedicated scouting state/input/render modules:

- [x] Implement Shipyard queue and completed-probe inspection, enqueue and
      unstarted cancellation, target-first planning with optional jump override,
      redacted-route review, launch confirmation, active-route inspection, and
      mission/report status.
- [x] Keep map/list selection synchronized by stable system identity as admitted
      knowledge changes; never introduce a name or chart point from a hidden
      route stop.

Test lane; owns separate scouting fixture/integration tests:

- [x] Reuse the small hand-computable Tier 1 scouting fixtures in
      `crates/game-core/tests/ships_expansion.rs` and
      `crates/game-core/src/stage5_boundary_tests.rs` without accepting a
      generated seed as the gameplay oracle.
- [x] Cover queue/cancel, invalid target/jump limit, assessment/commit agreement,
      multileg redaction, reveal on arrival, delayed report receipt, and
      player-view frontier refresh at the core boundary.

#### 5C-3 — Integration and gate

- [x] Complete the scouting journey through the real app/TUI composition,
      including travel, observations, awaiting report, and delayed receipt,
      through extensive manual playtesting. Deterministic core tests separately
      verify exact receipt scheduling and player-view updates.
- [x] Accept deterministic core redaction coverage plus extensive playtesting
      for hidden intermediate IDs and pending report contents. Additional
      cross-layer render/DTO negative assertions are explicitly waived.
- [x] Update architecture/README/CHANGELOG where the playable loop changes.
- [x] Run the full workspace formatting/check/Clippy/test suite and explicit
      non-mutation/redaction tests. Revalidated during the implementation audit.
- [ ] Record the 5c gate as passed before starting any Slice 5d branch.
      **Historical deviation:** the slices landed together, so this sequencing
      gate was not observed.

## Slice 5d — Founding loop

### Goal

Expose the bounded Stage 4b expedition and founding loop as governor play.

### Included

- inspect and enqueue expedition projects;
- show complete commitments and population requirements;
- select knowledge-valid targets;
- provide named reservations only when complete target knowledge permits them;
- launch a completed expedition;
- show population departure, awaiting outcome, redacted travel, success, and
  explicit loss through player-safe views;
- delay remote commandability until the successful founding report arrives;
  and
- inspect a founded daughter system only after it becomes commandable.

### Acceptance focus

Use short deterministic Tier 1 scenarios for both successful founding and
founding loss. Physical arrival must not leak an outcome before the approved
transmission reaches the player.

### Deferred

- reclamation;
- automated freight or delegated logistics;
- daughter-system policy AI;
- succession after origin loss; and
- richer expedition composition.

### Implementation task graph

#### 5D-0 — Core founding-query gate (serial)

- [x] Refactor expedition launch validation into one private plan shared by
      read-only assessment and atomic commit.
- [x] Add a player-safe expedition assessment containing complete commitments,
      resident-population requirement/readiness, travel cost, knowledge-valid
      target state, typed limiting reason, and redacted route.
- [x] For complete knowledge, assess only explicitly observed reservation
      coordinates supplied by the player; for summary knowledge, expose no
      named reservation choices. Commit revalidates current authoritative slot
      availability.
- [x] Test assessment non-mutation and agreement with launch for valid,
      insufficient-Energy, no-population, reservation-collision, and hidden-stop
      cases.

#### 5D-1 — Founding contract gate (serial)

- [x] Extend app contracts with expedition queue/assets, complete commitments,
      population readiness, target and optional reservation drafts, assessment,
      launch, redacted travel, `AwaitingOutcome`, founded/lost outcomes, and
      daughter commandability.
- [x] Specify concise correction behavior for stale target/reservation drafts
      using typed dispositions; do not infer recovery from error strings.
- [x] Compose expedition/founding surfaces from the established component
      language and mission modal.
- [ ] Commit the compiling contract and module exports before parallel lanes.
      **Historical deviation:** this separate gate commit did not occur.

#### 5D-2 — Parallel application and TUI lanes

Application founding lane; owns a dedicated `game-app` founding module:

- [x] Map expedition enqueue/cancel/assessment/launch to existing core commands,
      preserving complete commitments and resident-population requirements in
      player-facing views.
- [x] Project departure, redacted travel, awaiting outcome, received success or
      loss, and daughter commandability only from admitted player-safe state.
- [x] Keep physical founding/loss and pending transmission internals absent until
      the approved report arrives.

TUI founding lane; owns dedicated founding state/input/render modules:

- [x] Implement expedition project/asset inspection, enqueue/cancel, target and
      optional reservation drafts, assessment, launch confirmation, and retained
      correction state.
- [x] Implement population departure, redacted route, awaiting outcome,
      success/loss, and founded-daughter inspection without premature unlock.

Test lane; owns separate founding fixture/integration tests:

- [x] Reuse small hand-computable success and insufficient-slot loss fixtures
      from `ships_expansion.rs`; do not use generated-world quality as an oracle.
- [x] Cover complete reservations, summary auto-selection, commitment/refund,
      population departure, physical arrival before receipt, explicit loss,
      overflow evidence, and command unlock only after successful receipt at the
      core boundary.

#### 5D-3 — Integration and Stage 5 gate

- [x] Complete the founding acceptance evidence: extensive manual playtesting
      covers the successful app/TUI journey, while the deterministic Tier 1
      simultaneous-arrival test covers delayed explicit founding loss.
- [x] Accept deterministic core player-view timing coverage plus extensive
      playtesting for pre-receipt outcome, daughter-state, loss-accounting, and
      commandability redaction. Additional cross-layer DTO/render assertions are
      explicitly waived.
- [x] Update architecture, README/play instructions, design references where
      implementation fixed a durable contract, and `CHANGELOG.md`.
- [x] Run formatting, workspace all-target/all-feature check, Clippy with
      warnings denied, all-feature tests with no ignored tests, and dependency
      inspection. Revalidated during the implementation audit.
- [x] Verify every Stage 5 completion criterion and mark Stage 5 complete.

## Stage-wide exclusions

- persistence, migration, save compatibility, or autosave;
- full event-log replay and generated-world invariant soaks;
- real-time simulation or pause-speed controls;
- markets, pricing, traders, wallets, or independent NPC communities;
- generated-world desirability scoring, seed screening, or statistical gates;
- reclamation, logistics automation, specialists, cultural influence, or
  delegation;
- agent-facing CLI interaction or structured automation protocols; and
- compatibility with retired application, TUI, CLI, DTO, or keybinding APIs.

## Stage 5 completion

Stage 5 is complete when:

- [x] Normal human startup generates and previews an explicit
      origin-and-frontier world before entering a TUI session.
- [x] The player begins as the origin governor and can operate the retained
      bank/develop/bootstrap, scouting, and bounded founding loops.
- [x] All time advancement required for play can be performed manually.
- [x] The TUI remains a presentation/input layer over immutable application
      views and typed intents. Slot-level probe and expedition actions are now
      typed application projections rather than TUI-derived Shipyard logic.
- [x] In-session views preserve Stage 4b knowledge and commandability
      redaction.
- [x] Neutral frontier systems do not instantiate living markets or NPC
      communities.
- [x] The workspace passes formatting, compilation, Clippy with warnings
      denied, and focused deterministic tests with no generated-world quality
      gates.
