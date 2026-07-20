---
title: Mode-Based Terminal UI/UX Overhaul
type: feature
date: 2026-07-13
---
# Mode-Based Terminal UI/UX Overhaul

## Executive Summary

Replace the prototype's all-at-once pane dashboard with a keyboard-first, mode-based terminal interface. Systems, trade/travel, governance, and intelligence are equal gameplay activities rather than subordinate parts of one primary fantasy. Each activity gets one obvious interaction target, contextual controls, aligned data presentation, and progressive disclosure from visual summaries to exact detail.

The TUI will support terminal cell grids rather than pixel dimensions:

- Below `80x30`: unsupported-size screen.
- `80x30` through `159x44`: compact layout.
- `160x45` and above: regular layout.

Compact and regular layouts will share state, controls, formatting, and widgets. Compact presents one primary surface at a time; regular adds relevant context beside that surface. This is a full replacement of the current pane-focus model and provisional control documentation, with no compatibility layer for the old shortcuts.

## Problem Statement

The current TUI exposes systems, selected-system telemetry, routes, governance, markets, player trade status, events, and controls simultaneously. Five panes participate in a cyclic focus model, but economic and governance shortcuts are mostly global. Arrow keys therefore change different hidden selections depending on focus, while actions can target state that is not visibly active. The governor investment cursor is maintained but not rendered, making it impossible to know which allocation will change.

The fixed layout also compresses structured data into wrapped prose. System status, direct routes, energy claims, flow, population history, seasonal generation, governance, investment status, and route details share one paragraph. The Systems list puts variable-length metrics into unaligned strings, and the controls footer attempts to document every shortcut at once. The result is hard to scan even when sufficient terminal space is available.

There is also a target-identity ambiguity at the application boundary: the visible market belongs to the browsed system, while `Buy` and `Sell` execute at the player's current location. The replacement must explicitly distinguish inspection, local trade, governance, and travel destination contexts.

## Goals and Scope

### Goals

- Make Systems, Trade, Governance, and Intelligence explicit top-level activities.
- Remove pane-by-pane focus cycling and hidden action targets.
- Keep only one primary cursor or editable target active within each activity.
- Make selection, active mode, read-only state, and disabled actions understandable without relying on color.
- Add stable, deterministic system sorting without selection jumps during live simulation updates.
- Use real tables for comparative data and consistent alignment for changing numeric values.
- Use gauges, bars, trend arrows, and existing bounded histories for overview scanning while retaining exact values in detail views.
- Separate local trading from remote market inspection in immutable application projections.
- Support compact and regular layouts from shared rendering components.
- Preserve the headless simulation and existing TUI/application/core dependency boundaries.

### Non-goals

- A spatial or topology star map; gauges, bars, and sparklines satisfy the initial visual-overview requirement.
- Mouse input.
- Persisting TUI mode, selection, sort, or layout preference across process restarts.
- Save/load changes.
- Event filtering, search, or new simulation event categories.
- A third responsive layout class or a user-configurable pixel-size threshold.
- New crate boundaries or third-party dependencies.

## Proposed Solution

### Interaction hierarchy

Use four equal top-level activities:

1. **Systems** — compare systems, sort, inspect system health, and choose a travel destination.
2. **Trade** — inspect the local market, select goods and quantity, buy/sell, review cargo, and commit a previewed route.
3. **Governance** — inspect the governed system, edit policy, review imports, and adjust investments.
4. **Intelligence** — inspect recent events, aggregate trends, player/fleet summaries, and diagnostic detail that should not occupy the operational screens.

Canonical global navigation will use `F1` through `F4`, displayed in a persistent mode bar. `Tab` will not be required to reach a mode or pane. Global commands are limited to mode switching, pause/resume, single-step, tick rate, help, and quit. All economic, travel, governance, sorting, and detail commands are activity-local.

Each activity may have multiple visible regions in regular layout, but only one region owns the primary cursor. Context regions update from the active selection and do not enter a focus loop. A modal or detail overlay temporarily owns input; `Esc` closes that layer before returning to the activity root.

### Shortcut presentation

- Render contextual action labels as styled `Span`s with visible mnemonic notation such as `(B)uy`, `(S)ell`, `S(o)rt`, and `(T)ravel` where natural.
- Reserve a consistent accent style for shortcut characters.
- Pair color with textual notation; color is never the only indication of a key, selection, warning, or disabled state.
- Show only commands available in the active activity and current state.
- Keep full help contextual by default, with a secondary section for global controls.
- Name the current target and action consequence in the footer, for example `Buy 10 Structural Alloy at Frontier 01`.

### Layout classes

Layout selection is based on the `Frame` cell area:

```text
area < 80x30              -> Unsupported
area >= 160x45            -> Regular
otherwise                 -> Compact
```

Both dimensions must meet the regular threshold. A wide but short terminal remains compact.

- **Unsupported:** preserve TUI state, continue receiving application snapshots, do not alter simulation run state, and accept only resize and quit.
- **Compact:** render the activity header, one primary table/detail surface, a concise status summary, and contextual actions. Detail replaces the table until the player returns.
- **Regular:** compose the same activity widgets into a primary surface plus one or two passive context regions. Extra space adds relevant context, not unrelated subsystems.

Resizing across layout classes preserves activity, selections, sort, modal/detail state, quantity, event scroll position, and feedback.

### Systems activity

Render a stateful table rather than variable-length list strings. Initial sort keys:

- Name
- Risk/brownout severity
- Runway
- Energy fill percentage
- Population
- Route travel time from the player's location

Show the active key and direction in the title/header. Use deterministic tie-breakers ending in stable system ID. Store selection by `ContentId`; derive the rendered row after sorting or snapshot replacement. Do not allow live value changes to silently retarget the cursor.

Essential compact columns are marker/status, system name, risk, runway, and population trend. Regular mode adds energy stock/capacity, population, distance/travel time, and a selected-system overview. Exact coordinates, claims, flow, routes, seasonal values, and histories belong in the inspector.

Use redundant markers for important identities:

- `>` active cursor
- explicit player-location marker
- explicit player-governed marker
- warning glyph plus text label for severe states

### Trade activity

Make the trade target explicit and local:

- Title the actionable table `Local Market — <player location>`.
- Show remote market rows only in the selected-system inspector and label them read-only.
- Include selected good, quantity, held amount, quoted unit price, total, cargo result, and tank/energy effect in the action summary.
- Disable Buy/Sell while traveling or when the selected row cannot be traded, with a visible reason.
- Keep quantity entry modal, but make the current good and resulting total visible before confirmation.

Travel is selection-to-preview followed by explicit commit:

1. Systems selection identifies a proposed destination.
2. Entering Trade carries that destination as a route proposal without simulation mutation.
3. The Trade route panel shows jumps, distance, ticks, required travel energy, and energy after arrival.
4. A contextual Begin Travel action submits the command.
5. Rejection preserves the proposal and selection while showing corrective feedback.
6. During transit, route progress replaces proposal actions; market trading remains unavailable.

### Governance activity

Render policy and investment data as selectable rows/tables rather than joined prose.

- Default to the player-governed system; inspected autonomous systems remain visibly read-only.
- Render an explicit cursor for the selected policy, import priority, or investment.
- Use left/right or decrement/increment actions for discrete immediate edits.
- Preserve existing whole-policy application requests and core authorization checks.
- Show allocation percentage, level/max, next cost, cooldown, status, and a compact allocation bar in aligned columns.
- Show accepted, rejected, read-only, and no-op feedback adjacent to the edited target.
- Ensure the allocation total and per-investment constraints remain visible when changing a row.

### Intelligence activity

Move non-immediate information out of operational activities:

- Bounded recent event log with visible scroll position.
- Player status, cargo/value history summaries, rank, and trading totals.
- Fleet activity and aggregate world-dynamics diagnostics.
- Exact historical values that are useful for investigation but not routine action.

Follow the event tail only when the player is already at the newest event. When scrolled back, preserve the historical anchor and show that newer entries are available. Add a monotonically increasing presentation sequence to event views if needed to preserve that anchor across the bounded history rollover; do not add event persistence to `game-core`.

### Visual and exact-detail presentation

Use Ratatui's existing built-in widgets without new dependencies:

- `Gauge` or `LineGauge` for energy stock/capacity and cargo/allocation usage.
- `Sparkline` for existing population sufficiency history.
- Trend arrows and semantic text for population and seasonal direction.
- Tables for markets, routes, policies, investments, and comparative systems.

Every visual encoding must display an exact current value nearby or expose it through the activity's detail view. Do not synthesize time-series history in the TUI from replaceable `watch` snapshots; add bounded immutable history to `game-app` only when a required visual cannot use existing presentation data.

## Technical Approach

### Delivery and delegation protocol

Use acceptance tests as the executable contract for every implementation slice:

1. At the start of a phase, the primary agent writes or adjusts the phase's acceptance tests and runs the narrow commands needed to prove that new expectations fail for the intended reason while preserved behavior remains green.
2. The primary agent records the exact test names, commands, expected failures, scoped files, and constraints in each implementation delegation.
3. Delegated implementers change production code against those exact tests. They must not weaken, replace, skip, or broadly rewrite the acceptance tests; an incorrect or incomplete test is returned to the primary agent for adjustment.
4. The primary agent reruns the acceptance tests and inspects the resulting behavior before the slice is considered complete.

Keep delegated reviews narrow by concern and file scope—for example input routing, application-view targeting, layout/rendering, or test quality. Run reviewers in parallel only when every review is read-only and independent. Any review that may edit files runs sequentially under one implementation owner after findings are consolidated, preventing overlapping changes and conflicting fixes.

During implementation, use exact test names, module tests, or affected-crate tests. Do not repeatedly run full workspace checks after small edits. Run the full phase gate once when each phase's targeted tests are green, and run the complete acceptance suite once more at final acceptance.

### Architecture

The simulation remains unchanged and headless. Gameplay mode, layout class, sort, cursor, detail, modal, and scroll state remain TUI-local. Stable domain IDs may be used to identify selections and command targets, but terminal keys and widget/layout identifiers must not enter core commands.

Recommended TUI organization, retaining `game_tui::run` as the public entry point:

```text
crates/game-tui/src/
  lib.rs                    terminal lifecycle and async event loop
  state.rs                  activity, layout, sort, stable selections, overlays
  input.rs                  global/modal/activity input routing
  render.rs                 shell, breakpoint selection, shared theme
  screens/
    mod.rs
    systems.rs
    trade.rs
    governance.rs
    intelligence.rs
  widgets/
    mod.rs
    shortcuts.rs
    summaries.rs
    tables.rs
```

This is an internal module split, not a new crate boundary. Extract files as behavior moves; do not create empty abstraction layers ahead of use.

The input router should process keys in this order:

```text
unsupported-size gate
-> active modal/detail layer
-> global commands
-> active activity commands
-> no-op
```

Represent application request intent separately from local state mutation where practical so pure key-routing tests do not need an async application owner for every case. The existing terminal lifecycle guard and staged cleanup tests remain intact.

### Application view contract

Refine presentation models in `game-app` so the TUI cannot confuse targets:

- Keep a stable browsed/inspected system ID and selected-system projection for system inspection and route proposal.
- Rename or regroup selected-system market/energy/population/governor data under an explicit inspection view.
- Add an explicit local-trade projection keyed by `player.location`, including local market rows and whether trading is currently available.
- Add player-governed identity to system summaries or a small governed-system summary so Governance can open the correct context without inferring authority from terminal state.
- Add shortest-route distance/ticks from the player's location to each system summary if route-time sorting is included.
- Replace raw event strings with presentation event rows containing a sequence number and formatted text only if required for stable event anchoring.

`Buy` and `Sell` remain local commands without system IDs; the new local-trade projection makes that existing rule explicit. Travel and governance commands continue to carry stable target IDs and remain validated by `game-core`.

### Data / Content Impact

- No RON content, save data, ECS component, or simulation schedule changes are expected.
- No migration or backward-compatibility layer is required.
- View-model restructuring is internal and may update tests directly.
- The old focus model, old control strip, and old provisional key documentation are removed rather than retained as aliases.

### Runtime / Platform Impact

- Begin every implementation/review session by exporting Rust's user toolchain path and verifying the required commands:

  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  command -v cargo rustc rg
  ```

- Restore `rg` as a declared development prerequisite: add Homebrew `ripgrep` to `setup/Brewfile`, add an `rg` check to `setup/doctor.sh`, and install it in the active environment before repository research or implementation if it is absent.
- Rendering remains immediate-mode and redraws from the current `Frame` area after resize events.
- Use cell dimensions for support decisions. Do not gate on Crossterm pixel width/height because those values may be zero or unavailable.
- System counts and market sizes are small; sorting per snapshot is acceptable, but avoid repeated route calculations in render functions. Project route summaries in `game-app` once per published view.
- Shared widgets must truncate or omit lower-priority columns rather than wrap table rows unpredictably.
- Preserve terminal restoration, panic-hook behavior, and clean shutdown paths.

## Implementation Phases

For every phase, the primary agent owns the acceptance-test changes and delegation packet. Implementers receive the exact tests after they have been authored and observed failing where new behavior is expected. Iteration uses targeted tests; the full workspace phase gate runs once after targeted acceptance is green.

### Phase 1: Lock the UX contract and interaction state

- [x] Export `$HOME/.cargo/bin` into `PATH`, verify `cargo`/`rustc`, install or restore `rg`, and update the repository setup/doctor scripts so later sessions reproduce the tool environment.
- [x] Have the primary agent add characterization and acceptance tests for existing domain actions that must survive the UI replacement: pause/resume, paused single-step, tick rate, local buy/sell, travel submission, authorized governance, read-only rejection, quantity cancel/confirm, help, quit, and terminal cleanup.
- [x] Record the exact Phase 1 test names and targeted commands before delegating implementation against them.
- [x] Define `Activity`, `LayoutClass`, `SystemSortKey`, `SortDirection`, activity-local stable selections, activity-owned detail/modal state, and feedback lifecycle in TUI-local state.
- [x] Implement pure layout classification for unsupported, compact, and regular cell grids.
- [x] Implement pure system ordering and stable-ID selection reconciliation, including deterministic ties and missing/empty-list behavior.
- [x] Define and test the input precedence contract before replacing rendering.
- [x] Split state/input/render modules only as concrete behavior moves out of `lib.rs`.

Validation:
- [x] Unit tests cover each breakpoint edge, sort key/direction, selection preservation after reordered live data, modal precedence, and global-versus-contextual routing.
- [x] Existing terminal setup/cleanup regression tests continue to pass unchanged.

### Phase 2: Separate application presentation targets

- [x] Have the primary agent write the remote-inspection/local-trade, governed-target, route-summary, and event-presentation acceptance tests first; record their expected failures and exact targeted commands.
- [x] Delegate application-view implementation against those unchanged acceptance tests.
- [x] Refactor `ApplicationView` into explicit system-inspection and local-trade projections while retaining immutable, TUI-independent data.
- [x] Project local market rows from `player.location`; keep remote selected-system market rows inspection-only.
- [x] Expose governed-system identity/authority and route summary fields needed by the activity tables.
- [x] Add explicit trading availability/reason presentation for traveling and other known unavailable states without weakening core validation.
- [x] Add presentation event sequence data only if needed for stable Intelligence scrolling.
- [x] Update application tests to verify remote inspection never changes local trade target and that selected-system changes remain simulation-neutral.

Validation:
- [x] A test selects a remote system, verifies its inspection market differs from the local trade market, buys locally, and confirms the mutation occurred only at the player's current location.
- [x] View projections contain player-facing names and stable IDs but no Ratatui/Crossterm types or ECS entities.
- [x] Headless core tests continue to run without terminal initialization.

### Phase 3: Build the responsive shell and Systems activity

- [x] Have the primary agent write the mode-shell, breakpoint, sorting, selection, alignment, and resize acceptance tests first at the exact target grids.
- [x] Delegate responsive shell and Systems implementation against those unchanged tests.
- [x] Replace `Focus` and pane cycling with the persistent activity bar, global status line, contextual action footer, and activity-owned cursor.
- [x] Implement unsupported-size behavior at below `80x30`, compact composition from `80x30`, and regular composition at `160x45`.
- [x] Build the Systems stateful table with visible selection, player/governor/warning markers, aligned numeric columns, active sort indication, and deterministic scrolling.
- [x] Add the compact system inspector and regular selected-system context using shared widgets.
- [x] Convert energy/population summaries to gauges, exact labels, and existing-history sparklines where useful.
- [x] Preserve activity/selection/sort/detail state through compact-to-regular resize round trips.

Validation:
- [x] `TestBackend` tests cover `79x30`, `80x29`, `80x30`, `159x44`, `160x45`, and `200x60`.
- [x] Buffer assertions verify the active mode, visible `>` selection marker, sort key/direction, compact/regular composition, exact-value fallback, and absence of internal content IDs.
- [x] Resize tests prove selected `ContentId`, sort, and detail state survive both layout transitions.

### Phase 4: Implement Trade, Governance, and Intelligence activities

- [x] Have the primary agent write activity-specific acceptance tests first for contextual input isolation, trade, route transitions, governance, Intelligence scrolling, feedback, and help.
- [x] Delegate non-overlapping activity implementations against those exact tests; keep shared input/state changes with one implementation owner.
- [x] Implement the local Trade table, cargo/action summary, quantity modal, explicit remote/read-only inspection labeling, and disabled-state reasons.
- [x] Implement route proposal, explicit Begin Travel, rejection recovery, in-transit progress, and arrival-state transitions.
- [x] Implement Governance policy/import/investment tables with explicit selected rows, aligned values, allocation bars, immediate discrete edits, and adjacent result feedback.
- [x] Implement Intelligence event scrolling, tail-follow rules, player/fleet/world summaries, and bounded-history rollover behavior.
- [x] Scope every non-global key to its active activity and remove old punctuation/case-sensitive shortcuts where a clearer row/action interaction replaces them.
- [x] Update contextual help and action labels for all activity and modal states.

Validation:
- [x] Input tests prove inactive-activity actions cannot fire against hidden selections.
- [x] Trade tests cover local/remote distinction, empty markets, traveling, insufficient resources, quantity cancel, accepted trade, and rejected trade with selection preserved.
- [x] Travel tests cover preview without mutation, commit, rejection, in-transit state, and arrival.
- [x] Governance tests cover visible investment selection, allocation constraints, accepted edits, read-only systems, and rejected policy updates.
- [x] Intelligence tests cover newest-tail following, scrolled-back anchoring, new-event indication, empty history, and bounded rollover.

### Phase 5: Visual polish, documentation, and regression closure

- [x] Have the primary agent adjust final visual/accessibility/regression acceptance tests before delegating polish or documentation changes.
- [x] Run narrow, read-only reviewers for input behavior, app/core boundaries, layout/accessibility, and test quality in parallel; consolidate findings before any sequential fix pass.
- [x] Standardize a small semantic theme for active mode, shortcut accent, selection, warning, success, error, disabled, and secondary text.
- [x] Ensure every color cue has a textual marker, glyph, label, or style-independent fallback.
- [x] Right-align numeric cells, keep units in headers where practical, format large overview values consistently, and expose exact values in detail.
- [x] Audit truncation, long names, zero/maximum integer values, empty data, read-only data, and severe brownout states in both layouts.
- [x] Remove the old focus enum, old help copy, old all-context control strip, and obsolete tests after replacement coverage is in place.
- [x] Update player documentation and the Unreleased changelog.
- [x] Capture compact and regular terminal screenshots or text-buffer captures for review.

Validation:
- [x] Full workspace format, Clippy, test, content-validation, and headless smoke commands pass.
- [x] Manual keyboard-only playthrough completes system inspection, sorting, local trade, travel, governance, event review, resize, help, and quit in compact and regular layouts.

## Acceptance Criteria

### Functional Requirements

- [x] The TUI exposes Systems, Trade, Governance, and Intelligence as direct top-level activities with no required pane-focus cycle.
- [x] At most one primary cursor/edit target is active in an activity, and it is visibly marked without depending on color.
- [x] Global commands cannot trigger a trade, travel, governance, sort, or detail action against a hidden target.
- [x] Systems can be sorted by every specified key in both directions; the active key/direction is visible and ties are deterministic.
- [x] System selection remains attached to the same stable ID while live values reorder rows.
- [x] Local actionable market data is explicitly tied to player location; remote market inspection is labeled read-only.
- [x] Travel requires a visible route proposal followed by an explicit commit and preserves context after rejection.
- [x] Governance visibly identifies the selected policy/import/investment and whether the selected system is editable.
- [x] Comparative data uses aligned tables, with numeric columns consistently aligned and long values safely truncated or moved to detail.
- [x] Visual summaries include exact current values or an obvious exact-detail path.
- [x] Shortcut mnemonics are displayed contextually with textual notation and a consistent accent style.
- [x] Below `80x30` the unsupported-size screen appears without changing simulation run state; `80x30` uses compact and `160x45` uses regular layout.
- [x] Resizing between compact and regular preserves activity-local state.
- [x] Help, quantity input, read-only/rejection feedback, event scrolling, pause/run, step, rate, and quit remain usable.

### Quality Requirements

- [x] `game-core` remains free of Ratatui, Crossterm, terminal keys, widget IDs, layout classes, and TUI selection state.
- [x] No new dependency or crate boundary is introduced.
- [x] Existing staged terminal cleanup behavior and regression tests remain intact.
- [x] The primary agent authors or adjusts acceptance tests before each delegated implementation slice, and delegated code is validated against those exact tests.
- [x] Reviewer scopes are narrow; parallel reviews are read-only, and any editing follow-up is sequentially owned.
- [x] Targeted tests drive iteration; full workspace gates run once per completed phase and once at final acceptance.
- [x] Every implementation/review session verifies Rust's toolchain path and an available `rg` command.
- [x] Compile, formatting, Clippy, content validation, and all relevant automated tests pass.
- [x] Manual compact and regular keyboard-only validation is completed.
- [x] Visual review captures demonstrate selection, alignment, mnemonics, compact layout, regular layout, remote/read-only state, and a severe warning state.
- [x] Save/content compatibility is recorded as not affected.

## Validation Plan

### Test-development recommendations

The primary agent develops state, input, projection, and rendering acceptance tests before delegating each implementation slice. Prefer pure functions and small state transitions for layout classification, sorting, selection reconciliation, command availability, feedback expiry/replacement, and modal precedence. Keep async application-owner tests for request targeting and simulation effects, and use `TestBackend` for geometry/style behavior.

Every delegation packet must name the exact tests and targeted command that define completion. Implementers may add lower-level tests, but must not loosen or replace the primary acceptance tests. If an acceptance test proves invalid, stop the slice and return it to the primary agent rather than adjusting the contract implicitly.

Do not add a snapshot-test dependency for this pass. Extend the existing buffer helpers to inspect specific rows, cells, symbols, and styles so failures identify the broken contract instead of producing broad golden-file churn.

### Automated validation

Iteration policy:

- [x] Use exact tests first, for example `cargo test -p game-tui <test_name> -- --exact` or `cargo test -p game-app <test_name> -- --exact`.
- [x] Expand only to the affected crate or closely related crate pair after exact acceptance tests pass.
- [x] Do not run `cargo test --workspace --all-features` or full workspace Clippy repeatedly during implementation iteration.

Phase gate, run once after each phase's targeted tests pass:

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `cargo test --workspace --all-features`
- [x] Run `cargo run -p game-cli -- --validate-content` and `cargo run -p game-cli -- --headless` in phases that alter application projections, integration, or final behavior.

Final acceptance repeats the complete phase gate once after all review fixes are consolidated.

Specific automated coverage:

- [x] Layout classifier boundaries and wide-but-short behavior.
- [x] Compact/regular widget reuse and state-preserving resize round trips.
- [x] Stable-ID selection through sorting, live reorder, list shrink, and empty list.
- [x] Input routing for unsupported, modal, global, and each activity context.
- [x] Visible selection and mnemonic fallback using symbols plus style-cell assertions.
- [x] Column headers, right-aligned numeric values, truncation, and exact-detail availability.
- [x] Remote inspection versus local buy/sell target.
- [x] Travel preview/commit/reject/transit/arrival.
- [x] Governance edit/read-only/reject/allocation constraints.
- [x] Intelligence tail, anchor, empty history, and rollover.
- [x] Long names, maximum numeric values, zero values, empty tables, all energy health states, and all brownout stages.
- [x] Terminal staged setup and reverse-order cleanup.

### Manual validation

- [x] Start paused at exactly `80x30`; navigate all activities without using Tab.
- [x] Repeat the core flow at `160x45`; confirm added context does not change commands or targets.
- [x] Resize `80x30 -> 160x45 -> 80x30` while a system, good, investment, event position, and modal state are active.
- [x] Run continuous simulation while sorting by volatile metrics; confirm the selected system does not change unexpectedly.
- [x] Inspect a remote system, switch to Trade, and verify the local market/location is unmistakable before buying.
- [x] Preview and reject a route, correct the problem, begin travel, and inspect in-transit controls.
- [x] Edit a governed investment, inspect an autonomous system's read-only state, and verify feedback placement.
- [x] Review events while new events arrive; verify tail and historical-scroll behavior.
- [x] Verify mnemonic, selection, warning, disabled, success, and error cues in a reduced-color terminal profile.
- [x] Quit from each activity and after resize; verify cursor, alternate screen, and raw mode restore correctly.

### Evidence to capture

- Test command outputs for format, Clippy, workspace tests, content validation, and headless smoke.
- Terminal captures at `80x30`, `159x44`, `160x45`, and `200x60`.
- Compact and regular captures of each top-level activity.
- Before/after notes for remote inspection versus local trade targeting.
- A short keyboard-only playtest note listing confusing controls, missed information, and any target mistakes.

## Dependencies and Risks

### Technical Dependencies

- Rust toolchain commands available from `$HOME/.cargo/bin` at session start.
- `ripgrep`/`rg` installed as a repository development prerequisite and checked by setup diagnostics.
- Ratatui `0.30.2` and its built-in `Table`, `TableState`, `Gauge`, `LineGauge`, `Sparkline`, `Tabs`, and `Scrollbar` capabilities.
- Crossterm `0.29.0` key and resize events, expressed in columns and rows.
- Existing immutable `game-app` watch snapshots and typed request boundary.
- Existing stable `ContentId` identifiers for selection and command targeting.

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Compact and regular renderers diverge into separate UIs | Duplicate bugs and inconsistent controls | Share state, input routing, formatting, and widgets; vary composition only. |
| Mode replacement recreates hidden focus through nested panes | Original usability problem returns | One primary cursor per activity; passive context regions; explicit modal ownership. |
| Remote inspection and local trade remain coupled | Player acts on a different market than the one implied onscreen | Add explicit local-trade projection, titles, target summaries, and integration tests. |
| Live sorting changes the command target | Wrong system is inspected or traveled to | Store stable IDs, derive row indices, and use deterministic tie-breakers. |
| Visual widgets obscure exact numbers or rely on color | Reduced precision or inaccessible status | Pair every visual with exact text/detail and redundant symbols/labels. |
| Large `game-tui/src/lib.rs` refactor breaks terminal lifecycle | Damaged terminal on startup/exit failure | Keep lifecycle ownership in `lib.rs` and preserve staged RAII tests throughout extraction. |
| Too many metrics return to regular mode | Information overload persists despite more space | Define mode-specific essential/context/detail tiers and reject unrelated panes. |
| App view refactor leaks terminal concerns downward | Architecture boundary erodes | Keep layout, keys, modes, cursors, and sorting in TUI; app exposes immutable presentation facts only. |
| Event anchoring expands into a full event-query system | Scope and contract complexity grow | Limit to sequence/text and bounded history; defer filtering/search. |
| Breakpoints feel wrong under different fonts | Pixel expectations do not match cell grids | Document cell thresholds and test exact grids; do not use unreliable pixel dimensions. |
| Acceptance tests drift during delegation | Implementation passes a weakened contract rather than the intended UX | Primary agent owns acceptance tests; delegate exact names/commands and require handoff for contract changes. |
| Parallel reviewers or fixers overlap | Conflicting edits and unclear ownership | Parallelize only narrow read-only reviews; consolidate before one sequential fix owner acts. |
| Full workspace checks dominate iteration | Slow feedback encourages skipped validation | Use exact/affected-crate tests during iteration and one full gate per phase plus final acceptance. |
| Rust or `rg` is missing from the agent shell | Research and validation tools fail despite project setup | Export `$HOME/.cargo/bin`, declare `ripgrep` in setup, and verify commands at session start. |

## Documentation and Follow-up

### Documentation to update

- [x] `README.md` — replace the old Tab/global shortcut list, document activities, contextual controls, and `80x30`/`160x45` cell-grid behavior.
- [x] `archive/market-trading-prototype/docs/initial-prototype.md` — intentionally left unchanged; this legacy prototype specification is no longer maintained.
- [x] `CHANGELOG.md` — add the user-visible mode-based UX replacement under `Unreleased`.
- [x] `setup/Brewfile`, `setup/doctor.sh`, and `setup/README.md` — declare and verify `ripgrep`, and document exporting `$HOME/.cargo/bin` when the shell does not already include it.
- [x] `docs/architecture.md` — update only if the final application-view grouping adds a durable contract not already covered by immutable frontend projections and TUI-local state.

### Intentional follow-up

- [ ] Evaluate a topology-first system network map after the mode/table UX is playtested.
- [ ] Consider mouse support only after keyboard interaction stabilizes.
- [ ] Consider persisted UI preferences and manual compact override when save/settings support exists.
- [ ] Revisit breakpoints using actual playtest captures rather than pixel assumptions.

## References & Research

References use paths relative to the repository root unless noted.

### Internal references

- `crates/game-tui/src/lib.rs:22-68,197-414` — current focus cycle, UI state, global input matching, selection movement, and index clamping.
- `crates/game-tui/src/lib.rs:416-510` — current `70x24` gate, fixed pane layout, all-context controls, help, and focus styling.
- `crates/game-tui/src/lib.rs:512-735,782-901` — unaligned system strings, dense selected-system paragraph, market table, player paragraph, and event slicing.
- `crates/game-tui/src/lib.rs:1270-1597` — existing TestBackend, edge-case rendering, request mapping, focus, modal, governance, and terminal behavior coverage.
- `crates/game-app/src/lib.rs:63-101,104-311` — typed requests and current immutable system, market, player, governor, fleet, and application view models.
- `crates/game-app/src/lib.rs:350-405,540-890` — app-owned browse selection, local command submission, selected-market projections, route previews, and player/governor projections.
- `crates/game-core/src/lib.rs:2051-2137,2754-2836` — local trade targeting and core validation for location, travel, and governance authority.
- `docs/architecture.md:5-9,92-109,139-154,202-218,316-320` — headless core, dependency boundary, typed command constraints, TUI-local state, and TUI test strategy.
- `archive/market-trading-prototype/docs/initial-prototype.md:45-59,306-318,423-478,550-579` — prototype activity scope, local trade/travel rules, current pane requirements, and acceptance flow.
- `README.md:26-43` — current player controls and minimum terminal documentation to replace.
- `Cargo.toml:14-16`; `Cargo.lock:429-430,1407-1408,1423-1424,1488-1489` — pinned Crossterm and Ratatui package versions.

### External/package references

- [Ratatui 0.30.2 documentation](https://docs.rs/ratatui/0.30.2/ratatui/) — immediate-mode full-frame rendering, responsive nested layouts, and resize redraw behavior; cross-checked against the installed `0.30.2` crate source.
- [Ratatui widgets documentation](https://docs.rs/ratatui-widgets/0.3.2/ratatui_widgets/) — built-in tables, selection state, gauges, sparklines, tabs, and scrollbars available without a new dependency.
- [Crossterm 0.29.0 terminal documentation](https://docs.rs/crossterm/0.29.0/crossterm/terminal/index.html) — terminal size uses columns/rows and pixel dimensions may be zero or unreliable.
- [Crossterm 0.29.0 event documentation](https://docs.rs/crossterm/0.29.0/crossterm/event/enum.Event.html) — resize events report columns and rows and may arrive in batches.

### Institutional knowledge

- `docs/solutions/rust-terminal-staged-raii-cleanup.md` — preserve staged acquisition flags, reverse-order cleanup, and failure-path tests while restructuring the TUI.
- `archive/market-trading-prototype/docs/plans/2026-07-10-feature-initial-prototype-implementation-plan.md` — retain the original frontend/application/core separation and TestBackend validation strategy while replacing the provisional UX.
