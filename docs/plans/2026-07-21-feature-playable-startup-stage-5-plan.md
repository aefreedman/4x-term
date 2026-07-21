---
title: "Stage 5: Origin-and-Frontier Playable Startup"
type: feature
status: planned
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

## Authoritative context

| Concern | Source |
| --- | --- |
| Stage 5 direction and testing stance | [Testing Stance Correction](../2026-07-20-testing-stance-correction.md) |
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
   layout. Layout selection is
   available during startup and may be changed during play; machine-local
   persistence of that preference is not required in this stage.
10. The frontier overview presents a synchronized map and system list. Both are
    derived from player knowledge and must not reveal anonymous or hidden
    system identity or position.
11. Multi-tick advancement presents intermediate tick changes rather than only
    the final state.
12. Construction begins from a selected empty body slot.
13. Energy receives a dedicated information render rather than being shown
    only as one row in the general stock list.
14. The overall TUI design sensibility draws from *Caves of Qud* first and
    *Dwarf Fortress* second. Use them as references for atmosphere,
    information density, keyboard-first inspectability, map-and-panel
    composition, and systemic legibility—not as layouts, terminology,
    keybindings, color palettes, or widgets to copy literally.

## Target architecture

The concrete need to keep human interaction separate from simulation ownership
justifies a small application/session boundary:

```text
human-play executable ──► game-tui ──► game-app ──► game-core
          └───────────────────────────────────────► game-content
```

Exact crate names and executable packaging may be adjusted during Slice 5b
setup, but the responsibility boundaries are contractual:

### `game-app`

- exclusively owns the mutable `WorldState` for a running session;
- composes generated artifact metadata with player-safe runtime views;
- accepts typed domain intents and returns typed accepted/rejected outcomes;
- creates immutable, presentation-ready views;
- resolves player-facing labels, costs, availability, and limiting reasons
  from loaded data;
- never exposes unrestricted mutable core state; and
- remains independent of terminal libraries and terminal events.

The first implementation is synchronous. Manual tick advancement provides no
concrete need for Tokio, channels, background tasks, or a wall clock.

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

## Slice 5a — UI design foundation

Complete and review the minimum human-interaction foundation before
implementation agents choose crate APIs, Ratatui widgets, or input handlers.
The goal is not to pre-design every screen. Approve critical reference screens,
a reusable UI design system, and an element classification that lets later
screens be composed consistently without requiring agents to invent a new
visual language.

Markdown and exact-cell ASCII wireframes are sufficient; this phase should not
build a throwaway TUI or make a generated seed into a gameplay acceptance
target. Store the approved design artifacts under `docs/design/` and link them
from this plan. The artifacts may be separate documents or one reviewed Slice
5a TUI specification, but together they must cover the following contracts.

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
- contextual help; and
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

- screen shell, title/status regions, panel grid, spacing, borders, and focus;
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
layout-specific keys directly. Startup and in-play layout selection and all
contextual hints must use the active mapping.

### Information hierarchy and formatting

Classify player information as always visible, visible on inspection,
contextual help, or intentionally omitted. Define formatting rules for ticks,
seasons, quantities, progress, generation identity/fingerprints, labels versus
stable IDs, truncation, disabled actions, warnings, errors, selection, and
scroll position.

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

- [ ] Approve the surface inventory and navigation map.
- [ ] Approve the *Caves of Qud*/*Dwarf Fortress* reference-sensibility note.
- [ ] Approve the UI design system and element classification matrix.
- [ ] Approve the critical `160x45` reference wireframes and component edge
      states.
- [ ] Approve keybindings and input precedence.
- [ ] Approve information hierarchy and formatting rules.
- [ ] Approve command-flow state machines.
- [ ] Approve typed intents and application-view fields.
- [ ] Walk through all Slice 5b UX acceptance scenarios.
- [ ] Record deferred UI questions explicitly.
- [ ] Link the approved design artifacts from this plan.

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
  - generator family and revision;
  - logical profile identity and normalized fingerprint;
  - generated system count;
  - origin identity and the constructed origin scaffold; and
  - clear generation/content failures.
- Permit regenerate/repreview with a different seed before starting.
- Start a fresh `WorldState` only after explicit player confirmation.

#### Minimum gameplay surface

- Inspect world time through a knowledge-filtered frontier overview containing
  a synchronized map and system list.
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
6. Navigate the synchronized frontier map and list without revealing hidden
   system identity or position.
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
- dedicated Energy rendering from application-provided values;
- arrow, QWERTY `hjkl`, and Colemak-DH `unei` navigation through one semantic
  input mapping, with the directional meaning of every key asserted;
- input routing and modal/overlay precedence;
- the `160x45` design canvas, representative larger layouts, and the safe
  below-minimum view at `159x45` and `160x44`;
- textual cues for warning/disabled/selection states; and
- terminal lifecycle restoration with an injectable boundary where practical.

### Completion checklist

- [ ] Record exact crate/executable names and dependency choices.
- [ ] Add the synchronous application/session owner.
- [ ] Add generated preview and explicit start-session composition.
- [ ] Define typed Slice 5b intents, outcomes, and immutable views.
- [ ] Implement minimum TUI input, state, rendering, and terminal lifecycle.
- [ ] Cover the deterministic acceptance journey and focused adapter tests.
- [ ] Update architecture, README/startup instructions, invariant references if
      applicable, and `CHANGELOG.md` under `Unreleased`.
- [ ] Pass formatting, all-target/all-feature check, Clippy with warnings
      denied, and all-feature workspace tests with no ignored tests.

## Slice 5c — Scouting loop

### Goal

Expose the implemented probe loop while preserving the player knowledge
boundary.

### Included

- inspect functional Shipyards, their queues, and completed probe assets;
- enqueue and cancel unstarted probe projects;
- choose only knowledge-valid targets;
- choose an explicit jump limit within profile-authored bounds;
- preview only the redacted route available to the player;
- launch a completed probe;
- display active redacted routes and mission state;
- manually advance ticks through travel, observation, and delayed report
  receipt; and
- update the frontier view as knowledge changes.

### Acceptance focus

A short deterministic fixture or explicit generated request demonstrates that
an anonymous/identified frontier changes only through approved observations and
transmissions. The TUI must not name hidden intermediate systems before the
player boundary reveals them.

### Deferred

- expeditions and founding;
- deep survey layers beyond Stage 4b;
- route optimization advice; and
- real-time ticking.

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

## Stage-wide exclusions

- persistence, migration, save compatibility, or autosave;
- full event-log replay and generated-world invariant soaks;
- real-time simulation or pause-speed controls;
- markets, pricing, traders, wallets, or independent NPC communities;
- generated-world desirability scoring, seed screening, or statistical gates;
- reclamation, logistics automation, specialists, cultural influence, or
  delegation; and
- agent-facing CLI interaction or structured automation protocols;
- compatibility with retired application, TUI, CLI, DTO, or keybinding APIs.

## Stage 5 completion

Stage 5 is complete when:

- normal human startup generates and previews an explicit origin-and-frontier
  world before entering a TUI session;
- the player begins as the origin governor and can operate the retained
  bank/develop/bootstrap, scouting, and bounded founding loops;
- all time advancement required for play can be performed manually;
- the TUI is only a presentation/input layer over immutable application views
  and typed intents;
- in-session views preserve Stage 4b knowledge and commandability redaction;
- neutral frontier systems do not instantiate living markets or NPC
  communities;
- the workspace passes formatting, compilation, Clippy with warnings denied,
  and focused deterministic tests with no generated-world quality gates.
