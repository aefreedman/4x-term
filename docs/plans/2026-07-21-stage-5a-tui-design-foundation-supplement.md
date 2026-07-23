---
title: "Stage 5a TUI Planning Supplement — Design Foundation"
type: plan-supplement
status: approved
date: 2026-07-21
approved: 2026-07-21
source: "../plans/2026-07-21-feature-playable-startup-stage-5-plan.md"
---
# Stage 5a TUI Planning Supplement — Design Foundation

## Review purpose

This approved supplement establishes a reusable terminal design system, classifies the required Slice 5b surfaces, fixes common interaction behavior, and records the application-view boundary. It remains an implementation-plan supplement rather than durable game-design documentation. It does not select Rust dependencies or implement widgets.

The overall direction and every decision below are approved. Local panel composition may evolve inside this system; new interaction patterns or component types require review.

See the exact-cell [reference wireframes](2026-07-21-stage-5a-tui-reference-wireframes-supplement.md).

## Decision review status

| Decision | Review | Answer under review or approved |
| --- | --- | --- |
| Map when position is not known | Approved | Plot only systems with player-known positions. Keep identified but unpositioned systems selectable in the synchronized list with `--` for position; do not add explanatory sections inside the map. Never plot uncharted indications. Center the known origin without requiring a global coordinate. If a richer tick-zero spatial map is desired, change the scouting contract explicitly rather than reading hidden positions. |
| Preview boundary | Approved | Preview is an allowlisted player summary: seed, profile name, and origin scaffold. Generator family/revision, fingerprint, provenance, aggregate count, and neutral-world facts are not player-facing. Starting play discards preview-detail state except seed/profile session identity. |
| Multi-tick behavior | Approved | Execute and render one atomic tick at a time at a player-selected pace of 1, 5, or 10 ticks per second, defaulting to 5. Space pauses/resumes between ticks; Enter advances one tick while paused; Esc stops between ticks. Already completed ticks remain committed; a rejected tick remains uncommitted. Report requested, completed, stopped/rejected state, and visible per-tick changes. |
| Correctable rejection | Approved | A rejected construction outcome explicitly supplies `Retain` or `InvalidateRoot`. `Retain` preserves slot, role, and optional target and returns from the concise rejection overlay to that draft. `InvalidateRoot` closes the draft and returns to the refreshed slot list. Accepted construction closes its draft. The TUI never infers recovery from error text. |
| Quit without saves | Approved | Quit from startup is immediate. Quit from an active session requires confirmation stating that the session cannot be resumed and defaults to Cancel. |
| Failed regeneration | Approved | Editing profile or seed immediately makes an existing preview stale and disables Start. A failed generation preserves edited fields and the visibly stale prior preview for comparison, but the stale preview cannot start a session. |
| Undersized recovery | Approved | Below `160x45`, stop multi-tick advancement between ticks, preserve focus/drafts/overlays and completed history, dispatch no gameplay intent, and allow only resize recovery or confirmed quit. Restore the prior composition and local state after recovery, except that a previously running multi-tick batch remains explicitly stopped and never resumes automatically. |
| Tick-zero Energy evidence | Approved | Render last-tick fields as `-- no completed tick --`, not as zero evidence. The application view represents this distinction explicitly. |
| Habitat toggle | Approved | Use a lightweight confirmation showing current state and preserved progress. Unavailable controls remain visible with an application-provided reason. |
| Larger terminals | Approved | Keep the approved 160x45 composition and deterministically expand list/map viewports. Do not invent a second responsive layout in Stage 5. |
| Keyboard mode | Approved | Keyboard mode is a global TUI user setting, independent of profile, seed, generation identity, and session. It is changed through global settings, never through the generation form. Machine-local persistence may be added later without changing gameplay state. |
| Player aliases | Approved | A charted system may receive a session-owned player alias. Collections show the alias first; detail retains the stable `FSC NNNNNN` label. Clearing restores the catalogue label. The TUI sends an intent and never owns the alias map. |

## Reference sensibility

### Caves of Qud — primary

Adopt:

- a map-centered composition supported by compact information panels;
- keyboard-first contextual interaction with obvious current focus;
- evocative terminal texture through restrained glyphs and purposeful space;
- layered inspection: overview first, exact state one action away; and
- overlays that preserve the spatial or systemic context beneath them.

Reject:

- literal palettes, terminology, layouts, glyph sets, or keybindings;
- decorative glyph noise that reduces legibility;
- an avatar-movement metaphor for governor actions;
- unexplained shortcuts; and
- color-dependent meaning.

### Dwarf Fortress — secondary

Adopt:

- dense but hierarchical tables;
- stable ordering and exact state inspection;
- explicit queue, progress, capacity, shortage, and limiting-reason detail; and
- drill-down from system to body to slot to development.

Reject:

- menu labyrinths and screen-specific navigation rules;
- data walls without a glanceable hierarchy;
- engine IDs as primary player language;
- unbounded panel proliferation; and
- inspection paths that bypass player knowledge.

The resulting identity is a **map-centered governance terminal**: atmospheric but restrained, dense but navigable, and explicit about focus, state, and command consequences.

## Canvas and shell

The minimum and reference canvas is exactly `160x45` terminal cells.

| Rows | Region | Contract |
| --- | --- | --- |
| `0` | Title rail | Surface, selected location, and essential gameplay time only. Startup shows only the game/surface title. |
| `1..43` | Workspace | Screen-specific panels use the full available height. Panels stretch to the workspace edge rather than reserving empty vertical bands. |
| `44` | Global action bar | Only stable global actions such as Help, Settings, and Quit. No status sentence, input count, canvas size, or diagnostic prose. |

Focused-panel prompts live in a `PanelActionBar` on the panel's bottom interior row, rendered as button-like labels such as `[Enter Confirm] [Esc Back]`. They are not appended to explanatory block text.

Larger terminals retain the title and single global action bar. Extra height extends workspace viewports. The selected-system and Energy columns keep their reference widths; extra width is assigned two cells to the map for every one cell assigned to the system list. Below the minimum, the workspace is replaced by `SafetyView` and gameplay input is blocked.

ASCII borders are the exact-cell baseline. Single-width box-drawing glyphs may be a theme substitution. Panels use one interior padding cell when doing so does not compromise exact numeric display.

## Semantic markers

Color may reinforce but never replace these text or structural cues:

| Marker | Meaning |
| --- | --- |
| focused border/title treatment | Focused panel or field; text does not say `[F]`. |
| `>` | Selected row |
| `@` | Selected charted system |
| `*` | Other charted system |
| `~` | Draft or pending action when needed |
| `!` | Warning or rejection, paired with concise natural-language copy |
| `-` or `[UNAVAILABLE]` | Unavailable action |
| `[EMPTY]` | Valid empty collection or slot |
| `^ more:N`, `v more:N` | Viewport overflow |
| `--` | Value not yet observed or not applicable |

Uncharted systems never receive glyphs or list identities. Their existence is shown only as an aggregate such as `Uncharted: 12`. The application maps the core's existence-only knowledge aggregate to player-facing `Uncharted`.

## Player-facing system names

Generated frontier systems use a fictional survey-catalog style modeled on real astronomical catalogs such as HD, HIP, and Gliese: `FSC NNNNNN`, where `FSC` means **Frontier Survey Catalogue** and the six-digit number is the system's existing stable generated ordinal.

Examples:

- `Origin` — the authored home system keeps its proper name;
- `FSC 000004` — identified frontier system;
- `FSC 000017 b` — first known body in that system;
- `FSC 000017 c` — second known body.

The scheme is deterministic, compact, sortable, and does not encode a hidden position or resource fact. `game-app` resolves stable IDs into these labels; the TUI only renders supplied labels.

Once a system is charted, the player may assign a personal alias. Lists and map detail show the alias as the primary label and retain `FSC NNNNNN` in selected detail for disambiguation. Aliases are trimmed, single-line, and at most 32 display cells. Duplicate aliases are allowed because the catalogue label remains stable. Clearing an alias restores the catalogue label. Stage 5 has no saves, so aliases last for the current application session only; ownership is nevertheless in `game-app`, not TUI-local state, so later persistence can retain the same contract.

## Component system

| Component | Purpose | Required variants |
| --- | --- | --- |
| `ScreenShell` | Startup, play, and safety composition | startup, playing, safety |
| `PanelActionBar` | Bottom-interior button-like prompts owned by one panel/overlay | default, confirm, back/cancel, unavailable |
| `Panel` | Bordered region with title | default, focused, unavailable |
| `List` | Ordered selectable values | selected, disabled, empty, overflow |
| `Table` | Stable labeled columns | selected row, disabled row, overflow |
| `Detail` | Label/value inspection | known, unknown, unavailable |
| `Metric` | Glanceable exact value | normal, warning, unavailable |
| `ResourceTable` | Exact quantities and commitments | normal, shortage, unavailable |
| `Progress` | Work or generation progress | pending, paused, complete, unavailable |
| `FormField` | Profile, seed, count, layout | focused, invalid, source error |
| `ActionList` | Available player intents | available, unavailable-with-reason |
| `KeyHint` | Semantic action and current key | contextual, global |
| `Notice` | Concise outcome inside the owning panel or overlay | success, warning, rejection |
| `KnowledgeMap` | Player-known spatial/chart state | selected, collision, unpositioned, empty |
| `Overlay` | Context-preserving temporary layer | help, value entry, confirm, rejection |
| `SafetyView` | Blocks unsafe interaction | undersized, unrecoverable startup |

An implementation agent may compose an unwireframed surface from these components. A missing component or semantic variant is a design-review gap, not permission to add a one-off widget language.

## Formatting and overflow

- Player-facing labels are primary. Stable IDs appear only for disambiguation, reproduction, or explicit detailed inspection.
- Long labels truncate with `...`; inspecting the row reveals the complete value.
- Exact integer quantities are right-aligned and never abbreviated. Preserve up to 20 digits by sacrificing label width first.
- Progress always includes exact values, for example `[####....] 4/8`.
- Selected rows remain in view after an immutable view refresh.
- Lists show selected position and hidden row counts.
- Empty collections say what is absent and, when application-provided, why.
- Useful unavailable actions remain visible with their limiting reason.
- Generator fingerprints, provenance, family/revision, canvas dimensions, and other reproduction/debug metadata do not appear in the human TUI.
- The TUI does not calculate costs, legality, seasonal output, forecasts, or runway.

## Navigation grammar

Widgets receive semantic actions, never raw layout-specific navigation keys.

| Direction | Arrows | QWERTY | Colemak-DH |
| --- | --- | --- | --- |
| Up | `Up` | `k` | `u` |
| Down | `Down` | `j` | `e` |
| Left | `Left` | `h` | `n` |
| Right | `Right` | `l` | `i` |

Printable keys enter text when a text/value editor has focus. Navigation mapping applies only where the focused component accepts navigation.

| Input | Semantic action | Contract |
| --- | --- | --- |
| Direction | Navigate | Changes local UI focus/selection only. |
| `Tab` / `Shift-Tab` | Next/previous focus | Cycles the current composition's focus order. |
| `Enter` | Inspect/activate/confirm | Meaning is stated in contextual hints. |
| `Esc` | Back/cancel | Never commits; moves back one draft layer. |
| `PgUp/PgDn`, `Home/End` | Viewport movement | Local UI state only. |
| `?` | Contextual help | Opens or closes help. |
| `F2` | Global user settings | Opens settings; keyboard layout changes update all hints immediately. |
| `r` | Rename charted system | Opens alias editor for the selected charted system. Contextual; unavailable for uncharted/unpositioned entries. |
| `.` | Advance one tick | Immediate only in playing, nonmodal state. |
| `t` | Advance multiple ticks | Opens count entry and confirmation. |
| `q` | Quit | Confirms when a live session would be lost. |

Input precedence is strict and one event is handled by at most one layer:

1. undersized safety state;
2. any focused text or numeric editor, whether embedded or modal;
3. active confirmation or non-editor modal;
4. help/layout overlay;
5. global controls; and
6. focused-panel controls.

A focused editor consumes printable characters before semantic navigation or global shortcuts. `Esc` exits the editor without committing; only then do layout navigation letters, `.`, `t`, or `q` regain their global/contextual meaning.

### Interaction-coherence gate

A composition must not imply more focus targets than it implements. Exactly one visible component owns directional input; every accepted direction changes a selection visible in that component; and the direction must agree with the rows' spatial arrangement. Tab is reserved for traversal between two or more interactive focus targets, never for traversing rows in a single list. A child surface must not retain navigation that changes an off-screen parent selection.

Before approval, each surface needs an interaction table and a keyboard-only prediction walkthrough by a reviewer who has not been taught the controls. Rendering, routing, and state tests must be reviewed together rather than as independently correct layers. The active checklist and exact review procedure are in [Terminal UX Guidelines and Review Checklist](../tui-ux-guidelines.md).

## Surface inventory and navigation

| Surface | Class/template | Entry | Default focus | Exit/actions |
| --- | --- | --- | --- | --- |
| Startup fields | `ScreenShell.startup` + form | Process start | Profile field | Generate, global settings, quit |
| Generation failure | Startup message/safety | Generate rejected | First invalid field, otherwise Retry | Edit and retry, inspect diagnostic, quit |
| Preview | Startup summary composition | Generate accepted | Start action | Regenerate, start confirm, quit |
| Start confirmation | `Overlay.confirm` | Activate Start | Cancel | Start consumes preview; Esc returns |
| Main dashboard | `ScreenShell.playing` | Start accepted | System list | Select a system, manage it when controllable or inspect received knowledge when read-only, tick, help, quit |
| Frontier overview | Read-only map + focused synchronized list | Main dashboard | Last selected system row | Up/Down select; Enter directly manages controllable systems or opens read-only details; rename charted selection |
| System detail | Full read-only knowledge/facts composition | Enter on a listed system | System knowledge panel | Browse received survey facts; manage when commandable; Esc back |
| Body/slot inspection | Flattened visible slot list + synchronized detail | Manage local system | First visible slot | Up/Down traverse displayed slots; construct or inspect Habitat; back |
| Stocks inspection | `ResourceTable` + detail | Main dashboard/system detail | First stock row | Inspect exact quantity, back |
| Construction queue | `Table` + `Progress` | Main dashboard/system detail | First queued item, or panel | Inspect commitment/progress, back |
| Population/Habitat inspection | `Detail` + `Progress` + actions | System/body detail | First Habitat or population summary | Toggle available Habitat generation, back |
| Construction draft | Standard action composition | Activate empty slot | Role list | Target if needed, confirm, cancel |
| Construction rejection | `Overlay.rejection` | Commit rejected | Return to draft | Keep/correct draft or cancel |
| Habitat control | Standard action + confirm | Inspect Habitat | Toggle action | Confirm or back |
| Multi-tick | `Overlay.value-entry` then progress | `t` | Count | Run, interrupt between ticks, close |
| Help | `Overlay.help` | `?` | Current-context help | `?`/Esc close |
| Global user settings | `Overlay.settings` | `F2` from any surface | Keyboard mode | Apply immediately, Esc close |
| System alias editor | `Overlay.value-entry` | `r` on charted selection | Alias text | Apply, clear alias, Esc close |
| Command outcome | `Notice` in owning panel/overlay | Any outcome | Never steals unrelated focus | Dismiss or replace within that flow |
| Undersized terminal | `SafetyView.undersized` | Resize below minimum | None | Resize recovery or confirmed quit |

## Element classification matrix

| Element | Information class | Source | Knowledge sensitivity | Component |
| --- | --- | --- | --- | --- |
| Profile path and seed | startup | local startup draft | none | `FormField` |
| Keyboard layout | global settings | TUI user settings | none | `FormField`/`ActionList` |
| Generate/regenerate/start | startup | startup coordinator | none | `ActionList`, `Overlay.confirm` |
| Source-aware failure | during failure | content/generation outcome | none | `Notice`, `SafetyView` |
| Seed and profile identity | preview inspection | allowlisted preview view | must omit frontier truth | `Detail` |
| Origin scaffold preview | preview inspection | allowlisted preview view | origin only | `Table`, `Detail` |
| Frontier map | always in play | application projection | plot known position only | `KnowledgeMap` |
| System alias | inspection/collection label | application session | charted systems only | `FormField`, `Detail` |
| Identified system list | always in play | player view projection | admitted identities only | `List` |
| Uncharted indication count | always in play | player view's existence-only aggregate | aggregate only | `Metric` |
| Selected-system summary | inspection | system knowledge | fact-by-fact visibility | `Detail` |
| Energy | always for local system | application projection | local state only | `Metric`, `ResourceTable` |
| Bodies/slots | inspection | knowledge/local projection | summary and complete variants differ | `List`, `Table` |
| Stocks | inspection | commandable local projection | no neutral runtime | `ResourceTable` |
| Construction queue | inspection | commandable local projection | no neutral runtime | `Table`, `Progress` |
| Development role | draft | application action model | no hidden rules | `ActionList` |
| Extractor target | conditional draft | application action model | eligible targets only | `List` |
| Habitat control | inspection/action | application action model | local only | `ActionList`, `Progress` |
| Accepted/rejected outcome | within owning flow | typed application outcome | redact hidden cause/detail | `Notice`, `Overlay` |
| Multi-tick history | active overlay | ordered application views/deltas | player-visible deltas only | `Overlay`, `List`, `Detail` |
| Help/key hints | contextual | TUI semantics | none | `Overlay.help`, `KeyHint` |

## Knowledge map contract

The map and list share one stable selected system ID. Selection from either updates both. The map never receives or infers a hidden position.

- The origin is centered as the player's reference point.
- A system with a player-known position is plotted relative to the origin using an application-provided chart coordinate.
- Identified systems without a known position appear only in the synchronized list with `--` for position, never at invented map coordinates.
- Uncharted indications appear only as a count.
- Colliding chart cells use a stack marker and cycle only among identities already admitted to the player view.
- Off-screen charted systems receive edge indicators and remain list-selectable.
- The map panel uses its full interior as a glyph field. Legends, knowledge explanations, and unpositioned-system prose belong in the list/detail surfaces rather than consuming map space.
- The TUI performs viewport clipping and selection; coordinate projection and knowledge admission are application responsibilities.

## Dedicated Energy render

For a commandable local system, the dedicated render answers:

1. How much Energy is available now?
2. How much can be retained?
3. What happened during the last completed tick?
4. Is life support currently short?

It renders only application-provided values:

- current Energy, capacity, and headroom;
- current seasonal position;
- last-tick required, paid, and unpaid life-support Energy;
- supported and underserved population;
- last-tick retention overflow; and
- an explicit unavailable state before the first completed tick.

Forecasts, expected production, and runway are absent unless later supplied as approved core/application values.

## Command-flow state machines

### Generate and start

```text
Editing inputs
  -> Generate
  -> Preview current
  -> edit seed/profile -> Preview stale (Start unavailable)
  -> Generate -> Preview current
  -> Start -> Confirm -> consume artifact -> Playing

Generate rejected -> retain inputs + source-aware error; no current preview
Start accepted -> consume exactly the current preview artifact -> Playing
Start rejected -> remain on current preview, show structural startup error,
                  focus Start/Retry, and do not create a partial session
Start cancel -> return to current preview
```

The startup coordinator outside `game-tui` owns compiled profile data and the current generated artifact. The TUI receives typed startup views/intents only. Slice 5b must choose whether this coordinator lives in `game-app` or the thin executable; `game-tui` never loads content or receives `WorldDefinition`.

### Rename a charted system

```text
Select charted system -> r -> edit alias
  -> apply accepted: refresh all labels; retain catalogue label in detail
  -> clear accepted: restore catalogue label
  -> rejected: retain entered text and show concise validation reason
Esc -> close without changing the current alias
```

Uncharted indications and identified systems without a player-known position cannot be renamed. Alias mutation changes application-session annotation only; it does not mutate `WorldState` or generated definitions.

### Slot-first construction

```text
Select empty slot
  -> choose role
  -> if Extractor, choose eligible resource target
  -> review exact application-provided cost and availability
  -> confirm
     -> accepted: close draft, keep slot visible, show queued project
     -> retainable rejection: show concise overlay, preserve slot/role/target
        -> Enter or Esc returns to the retained draft
     -> invalidated root: close draft, return to refreshed slot list
Esc in draft -> one draft layer back; Esc at root cancels
```

### Habitat control

```text
Inspect Habitat
  -> choose Enable/Disable
  -> confirm state and preserved progress
     -> accepted: close confirmation and refresh
     -> rejected: keep inspection and show typed reason
```

### One tick

```text
`.` -> AdvanceOneTick
  -> accepted: refresh PlayingView and show accepted player-visible delta
  -> rejected: retain prior PlayingView, show typed reason, keep dashboard focus
```

The rejected tick is uncommitted by the core and the TUI does not fabricate an intermediate state.

### Multi-tick

```text
`t` -> enter count (1..100) and pace (1, 5, or 10 ticks/sec; default 5)
  -> confirm -> advance one atomic tick
  -> append player-visible intermediate result and render
  -> wait for the selected presentation cadence and poll controls
     -> Space: pause/resume between ticks
     -> Enter while paused: advance exactly one tick and remain paused
     -> Esc: stop between ticks
  -> repeat until requested count, rejection, or stop
  -> summary retains all completed ticks
```

The cadence is presentation pacing for an explicit manual command, not real-time or autonomous simulation. Interruption happens only between ticks. A rejected tick remains uncommitted, stops the batch, retains all earlier committed rows, focuses the rejected summary row, and displays the typed reason with requested/completed counts. Resize below minimum requests the same between-tick stop and enters the safety view. Recovery returns to a stopped summary and requires a new explicit confirmation before any remaining ticks run. No tick is partially committed or displayed.

### Quit

```text
Startup -> q -> exit
Playing -> q -> warning: no save/resume -> Cancel (default) | Quit
Undersized -> apply the same session-sensitive rule: immediate before play,
              confirmation when a live session would be lost
```

## Application-view and intent boundary

### Ownership

- A startup coordinator owns profile compilation and the current generated artifact.
- Starting consumes that artifact and transfers its definition into the sole mutable session owner.
- The session owner exclusively owns `WorldState`.
- The TUI receives immutable, presentation-ready values and emits typed intents.
- The TUI never receives `GeneratedWorldArtifact`, `WorldDefinition`, `WorldState`, test-support snapshots, tuning, recipes, or hidden locations.

### Required view families

- `StartupView`: editable profile/seed inputs, validation, and Generate action.
- `GenerationPreviewView`: allowlisted seed/profile/origin summary, current/stale state, Start availability.
- `PlayingView`: seed/profile session identity plus a projection derived from `PlayerWorldView` and non-secret catalog labels.
- `SystemDetailView`: fact-aware summary or commandable local detail.
- `ConstructionDraftView`: stable IDs, resolved labels, exact costs, availability, and limiting reason.
- `EnergyView`: current values plus optional last-tick evidence.
- `ApplicationOutcome`: typed acceptance/rejection, player message, and explicit draft-retention classification.
- `TickStepView`: resulting immutable playing view plus player-visible delta.

Action availability is typed as `Available { ... }` or `Unavailable { limiting_reason }`. Displayed availability assists the player; commit-time legality still comes from core validation.

Intents carry stable domain IDs, never row indices, labels, widget IDs, terminal coordinates, or raw key events.

### Current contract gaps assigned to the application boundary

- resource labels and deterministic `FSC NNNNNN` system/body display labels;
- player-facing origin/session labels;
- player-safe chart coordinates;
- fact-aware summary rows;
- population and Habitat presentation;
- action costs, eligible targets, availability, and limiting reasons;
- player-facing mapping of `CoreError`; and
- player-visible per-tick deltas.

These gaps must not be filled by reading complete generated definitions in the TUI or duplicating core rules.

### Exact Slice 5b intent contract

Startup intents:

- `EditProfilePath { value }`
- `EditSeed { value }`
- `GeneratePreview`
- `RequestStartCurrentPreview`
- `ConfirmStartCurrentPreview`
- `CancelStart`

Session intents:

- `EnqueueConstruction { system_id, body_id, slot_id, role, extractor_resource_id }`
- `SetHabitatGenerationEnabled { system_id, body_id, slot_id, enabled }`
- `SetSystemAlias { system_id, alias }`, where `alias: None` clears it
- `AdvanceOneTick`

Multi-tick is a TUI/application orchestration over repeated `AdvanceOneTick` intents, never a core batch mutation. Navigation, focus, selected rows, help, global user settings (including keyboard mode), multi-tick count editing, and quit confirmation are local TUI intents and do not enter the simulation owner or generation identity.

### Exact Slice 5b view fields

`StartupView` contains profile-path text/error, seed text/error, Generate availability, optional source-aware diagnostic, and optional `GenerationPreviewView`. Keyboard mode is not a startup-generation field.

`GenerationPreviewView` contains current/stale state, seed, player-facing profile name, allowlisted origin ID/label/community, origin body count, guaranteed developments, initial origin stocks, and Start availability/reason. It contains no generator revision/fingerprint/provenance/count and no neutral identity, position, body, resource, or topology data.

`PlayingView` contains seed/profile session identity, time/season presentation, identified system-list entries, chart entries only for known positions, unpositioned identified entries, uncharted indication count, selected-system detail, optional commandable local detail, optional `EnergyView`, latest `ApplicationOutcome`, and contextual/global actions.

Each system-list entry contains stable system ID, stable catalogue label, optional player alias, resolved primary display label, knowledge level, positioned/unpositioned state, commandability when player-visible, and observation freshness supplied by admitted facts. Summary detail contains only admitted fact rows. Local detail contains bodies/slots, stocks, construction queue, completed assets, derived local population count, and Habitat rows with stable ID/label, functional and occupied state, generation-enabled state, exact progress and required Energy, ready state, and toggle availability/reason. Occupancy and population remain derived presentation, not separately writable state.

`ConstructionDraftView` contains system/body/slot stable IDs and labels, available and unavailable role choices, eligible Extractor target choices, exact application-provided costs, availability, and limiting reason.

`EnergyView` contains current quantity, capacity, headroom, seasonal position, and optional last-completed-tick evidence with required/paid/unpaid life support, supported/underserved population, and retention overflow.

`ApplicationOutcome` contains accepted/rejected kind, player-facing message, optional stable result IDs, and, for rejected draft commands, a `DraftDisposition` of `Retain` or `InvalidateRoot`. Accepted construction closes its draft without a rejection disposition. `TickStepView` contains the resulting immutable `PlayingView` and an ordered player-visible delta; an empty delta is explicit.

## UX acceptance walkthroughs

Slice 5b tests should use small deterministic authored fixtures except where an explicit generation request verifies preview identity.

1. Edit startup inputs, generate, inspect preview, make it stale, regenerate, and start exactly the current preview.
2. Enter play and confirm a distinctive hidden neutral system appears in neither preview nor initial playing output.
3. Synchronize map/list selection for positioned and unpositioned identified systems; show uncharted knowledge only as a count and use `FSC NNNNNN` labels only after identification.
4. Rename a charted system, observe the alias in map/list/detail presentation, retain its catalogue label in detail, reject an invalid alias without losing text, and clear back to the catalogue label.
5. Inspect origin Energy before tick one, then after a tick with payment, shortage, and overflow evidence.
6. Select an empty slot, construct successfully, and observe queue state.
7. Trigger a retainable construction rejection, correct the draft, and commit. In a separate deterministic branch, return `InvalidateRoot`, close the draft, refresh the slot list, and retain no stale role or target selection.
8. Enable Habitat generation; inspect functional/occupied/enabled state, exact progress and required Energy, ready state, and toggle availability/reason; observe preserved progress and unavailable states; advance until bootstrap completes; and observe the derived local population count.
9. Advance one tick successfully, then exercise a rejected uncommitted tick.
10. Use a controllable clock to verify the default 5 ticks/sec pace, selectable 1/5/10 rates, pause, one-tick step while paused, resume, and between-tick stop without real sleeps. In a separate run, reject a later tick, preserve earlier committed rows, and leave the rejected tick uncommitted.
11. Resize below minimum during a draft and during multi-tick; recover focus, overlay, draft, and completed history without dispatching an extra intent. A previously running batch recovers as Stopped and never resumes automatically.
12. Switch QWERTY/Colemak-DH layouts while arrow keys continue to work and text fields consume printable input before global shortcuts.
13. Attempt to quit a live unsaved session, cancel by default, then confirm.

## Deferred design

- palette and decorative glyph vocabulary;
- terminal dependency and widget implementation;
- mouse input;
- compact layouts below `160x45`;
- persistence of keyboard preference;
- production forecasts and runway;
- probe, route, expedition, and founding surfaces owned by Slices 5c and 5d;
- stable agent-facing CLI interaction; and
- any change that reveals position for identified-summary systems.
