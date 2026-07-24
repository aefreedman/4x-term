---
title: "Playtest Analytics: Local Ring 1 Session Tracing"
type: feature
status: completed
date: 2026-07-23
tags:
  - analytics
  - playtesting
  - core-loop
  - tui
---
# Playtest Analytics: Local Ring 1 Session Tracing

## Objective

Add an opt-in, local playtest trace to augment the [Ring 1 Maturity Audit](2026-07-23-ring-1-maturity-audit.md) with descriptive behavioral evidence. The trace should connect semantic TUI interactions, application intents, player-visible consequences, and Ring 1 milestones without collecting raw keystrokes, transmitting data, exposing hidden world state, or turning generated-world observations into acceptance gates.

The executable enables tracing with `-T` or `--playtest-trace`. Either form may omit its path and use a safe default under `playtest-logs/`.

## Implementation audit — 2026-07-23

The feature is implemented. `game-play` owns dependency-free CLI parsing, collision-safe raw/summary artifact reservation, compact RON-lines serialization, typed summary accumulation, and finalization after terminal cleanup. `game-tui` owns a no-I/O semantic event queue and an injectable fallible observer boundary. Ordinary and paced-batch application dispatch now pass through one observed path, while tracing-disabled startup remains unchanged.

The trace records startup transitions, semantic surfaces and drafts, typed intents and outcomes, assessments, committed tick deltas, admitted selection changes, Ring 1 milestones, and orderly shutdown. Summaries report typed intent/rejection/assessment/draft/tick distributions, milestone intervals, admitted stock and population deltas, selection changes, and explicitly inferred possible-banking windows. Raw keys, rendered screens, aliases, machine paths, and privileged or unreceived world state are absent.

The originally proposed normalized profile fingerprint is not present in `TraceStarted`: the approved startup/player boundary intentionally omits reproduction identity and exposing it solely for analytics would violate the plan's player-safe constraint. `TraceStarted` records the initial seed, display profile name, package version, and schema version; `PreviewAccepted` records the final admitted seed and profile name after startup edits. No in-game annotation UI was added.

Validation passed formatting, workspace all-target/all-feature checking, Clippy with warnings denied, and the all-feature workspace suite (128 tests, none ignored). CLI/artifact tests cover default and explicit paths, collision handling, and no-overwrite behavior. TUI tests cover traced startup, direct and batch tick uniqueness, milestones, and orderly shutdown; existing deterministic core redaction tests continue to own hidden-route, report, and founding boundaries.

## Player and researcher outcome

A playtester can start an ordinary TUI session with:

```text
4x-term -T
```

The executable reports the chosen local trace path before acquiring the terminal, runs the same game, and writes an append-only semantic trace plus a compact summary. The playtester can refer to event sequence numbers and simulation ticks in separate experiential notes. After several sessions, traces can quantify completion, friction, correction, consequence timing, and allocation patterns while the notes retain authority over intent, understanding, and strategic meaning.

## Authority and interpretation

This is internal evidence tooling, not a gameplay mechanic. It supports the Ring 1 dependable-baseline foundation milestone defined by the audit.

Analytics remain descriptive:

- a generated seed outcome is not a failure unless it violates an active engine invariant or the constructive-generation contract;
- session counts and timing do not prove that a choice was meaningful;
- advancing a tick does not by itself prove that the player intended to bank;
- repeated rejections or cancellations are friction signals to investigate, not automatic defects;
- small internal samples are not population-level balance evidence; and
- experiential notes and direct observation remain necessary for legibility and decision-quality judgments.

## Settled CLI contract

Supported forms:

```text
4x-term                                      # tracing disabled
4x-term -T                                   # default trace path
4x-term --playtest-trace                     # default trace path
4x-term -T playtest-logs/my-session.ronl     # explicit path
4x-term --playtest-trace my-session.ronl     # explicit path
4x-term --playtest-trace=my-session.ronl     # explicit path
```

Rules:

1. `-T` is the short form of `--playtest-trace`.
2. A following non-option token is the explicit path. With no value, tracing uses the default path.
3. The default is `playtest-logs/playtest-<unix-milliseconds>-p<process-id>.ronl`, relative to the current working directory.
4. Default-path creation creates `playtest-logs/` as needed and uses create-new semantics. A collision receives a monotonic numeric suffix rather than overwriting evidence.
5. An explicit path creates missing parent directories but never silently overwrites an existing file. The command fails with a clear message and leaves the terminal untouched if the trace cannot be created.
6. `-h` and `--help` document tracing. Unknown options, duplicate trace options, missing required values for future options, and unexpected positional arguments fail before terminal acquisition.
7. Optional trace values are acceptable because the executable currently has no positional gameplay arguments. If future CLI growth makes the syntax ambiguous, preserve bare `-T` and `--playtest-trace` for the default while requiring `--playtest-trace=<PATH>` or a separately reviewed explicit-path form.

No command-line parsing dependency is justified for this bounded contract. Use a small tested parser in `game-play`; reconsider a parser crate only when additional executable options create a concrete need.

## Default artifacts

A traced session writes:

- `<name>.ronl` — append-only, one compact RON event per line; and
- `<name>.summary.ron` — a derived summary written on orderly shutdown.

`playtest-logs/` is machine-local and must be added to `.gitignore`. Explicit paths outside that directory remain the operator's responsibility. Neither artifact is committed by default.

The raw trace is primary evidence. If shutdown is interrupted before the summary is written, complete raw lines remain usable. A summary must be reproducible from its event meanings; it must not introduce hidden data or balance judgments.

## Architecture

Preserve the current dependency direction and ownership:

```text
game-play composition and file sink
  -> game-tui semantic observation
  -> game-app typed intents and player-safe outcomes
  -> game-core
```

### `game-play`

Own:

- CLI parsing and help;
- default-path selection and collision handling;
- directory and file creation before terminal acquisition;
- the concrete RON-lines recorder;
- orderly flush/finalization and summary writing; and
- concise startup/shutdown messages naming the artifacts.

This keeps process arguments and filesystem policy in the executable composition package.

### `game-tui`

Own:

- a small injectable playtest-observer interface;
- stable semantic UI event categories;
- observation at the central application-dispatch path;
- observation of batch tick dispatches, which currently bypass the ordinary dispatch helper;
- player-visible context needed to connect UI activity to application consequences; and
- a no-op default used by the existing `run` path.

The observer receives semantic events, not `KeyEvent` values. TUI state continues to own selection, drafts, pacing, and rendering, but not trace-file paths or serialization policy.

Centralize session dispatch sufficiently that single ticks, paced batch ticks, assessments, launches, rejections, and applied outcomes cannot silently escape observation. Do not duplicate gameplay rules or infer availability in the recorder.

### `game-app` and `game-core`

Do not add logging, filesystem access, terminal types, observer callbacks, or analytics policy. Analytics consume existing `SessionIntent`, `SessionOutcome`, `PlayingView`, `ApplicationOutcome`, assessments, and `TickDeltaView` values through the player adapter.

Production tracing must never enable `test-support`, call privileged snapshots, inspect pending transmissions, reveal hidden route stops, or serialize authoritative neutral-system state.

### Dependencies

Use the existing workspace `serde` and `ron` versions where stable event serialization requires them. Do not add `tracing`, Tokio, a database, a network client, a time/date crate, or a new crate boundary.

## Trace schema

Begin every file with a versioned header. Every later event has a strictly increasing sequence number, elapsed monotonic milliseconds, and the current player-visible simulation tick when available.

Minimum event families:

| Event | Player-safe content |
| --- | --- |
| `TraceStarted` | schema version, package version, seed, logical profile name, normalized profile fingerprint when already available, and trace mode; no machine profile path |
| `StartupAction` | preview generated, regenerated, accepted, or startup abandoned |
| `SurfaceChanged` | semantic screen/modal kind opened or closed; no rendered text or raw key |
| `DraftChanged` | construction, operation, Habitat, probe, or expedition draft started, cancelled, retained, invalidated, or committed |
| `IntentDispatched` | typed intent kind and admitted source/target/system context required for grouping; aliases and arbitrary editor text omitted |
| `IntentResolved` | accepted/rejected, typed limiting reason, draft disposition, and admitted project/ship identity where returned |
| `AssessmentResolved` | probe/expedition availability, player-visible commitment/readiness, travel Energy, knowledge level, and route summary without hidden stops |
| `TickAdvanced` | one committed `TickDeltaView`, whether direct or part of a batch, and the resulting player-visible aggregate context needed by approved metrics |
| `SelectionChanged` | admitted selected system identity and semantic panel/surface, sampled only on actual semantic change |
| `MilestoneObserved` | milestone derived from a transition between consecutive player-safe views |
| `TraceEnded` | orderly quit, final tick, event count, and recorder status |

Stable enums and typed numeric fields should be preferred to rendered labels and free-form messages. Human-readable application messages may change independently and are not analytic keys.

### Ring 1 milestones

Derive milestones only from transitions in player-safe views and accepted outcomes:

- session confirmed and origin commandable;
- first construction queued and first development completed;
- first intentional development/Habitat operational change;
- first Shipyard project queued and first probe ready;
- first probe launched;
- first probe awaiting report and first report reflected in admitted knowledge;
- first expedition queued and ready;
- first expedition launched;
- first founding outcome received;
- first founded daughter becomes commandable; and
- first governance action accepted in a founded daughter system.

A milestone is evidence that a transition was visible, not that the player noticed or understood it.

## Privacy and knowledge boundary

The trace is opt-in and local only.

Do not record:

- raw keys, terminal input bytes, or rendered screen contents;
- free-form aliases or future player-authored text;
- absolute profile paths, usernames, hostnames, environment variables, or terminal metadata;
- privileged world snapshots or global accounting;
- pending report contents, hidden mission outcomes, unidentified system identities, or hidden route stops; or
- any network identifier or remote destination.

Tests must specifically show that pre-receipt scouting and founding traces contain no hidden intermediate IDs, report contents, daughter state, or loss evidence. Trace context must be projected from the same admitted views used by the TUI.

## Derived summary

The orderly-shutdown summary should contain facts and measurements, not judgments:

- session duration, final tick, and orderly/incomplete status;
- milestone sequence and elapsed/tick intervals;
- intent attempts and acceptances by typed kind;
- rejections by typed limiting reason;
- draft starts, cancellations, retained corrections, and invalidations by kind;
- direct and batch tick counts;
- semantic surface/modal opens and abandonment counts;
- system-selection changes and post-founding governance actions;
- stock and population delta counts and magnitudes by admitted system/resource;
- report and founding-outcome latency in simulation ticks where both endpoints are player-visible; and
- observed develop/expand commitments.

Label inferred values explicitly. In particular:

- `possible_banking_windows` may count tick advances while no new development or expansion commitment is observed, but must not be named `banking_decisions`;
- elapsed wall time is interaction time, not proof of confusion; and
- action diversity is a distribution, not proof of strategic diversity.

The playtester should record actual bank/develop/expand intent, expected consequences, understanding, and alternative credibility in experiential notes, referring to trace sequence numbers or ticks.

## Implementation slices

### Slice A — CLI and artifact lifecycle

1. Add a dependency-free argument parser to `game-play` with the settled `-T` and `--playtest-trace` contract.
2. Add default-path generation, parent creation, no-overwrite behavior, and test-injectable time/process components.
3. Add `playtest-logs/` to `.gitignore`.
4. Open the trace and report its path before `game_tui` acquires the terminal.
5. Preserve the existing no-argument startup behavior exactly.

### Slice B — Semantic observer and raw trace

1. Define versioned serializable trace events and the observer boundary without exposing terminal keys or app-private state.
2. Add an observed TUI run entry point while retaining the current no-op `run` convenience path.
3. Route ordinary and paced-batch session dispatch through observation-complete code.
4. Emit startup, semantic surface/draft, intent, outcome, tick, selection, milestone, and shutdown events.
5. Write and flush one compact RON value per line.
6. Treat recorder failures as explicit runtime errors; never silently claim a complete trace. Preserve staged terminal cleanup before reporting the failure.

### Slice C — Summary and audit workflow

1. Implement a pure event accumulator for the approved summary measures.
2. Write the sibling summary after terminal cleanup on orderly shutdown.
3. Print final trace and summary paths with event count and final tick.
4. Document a short playtest workflow that pairs trace references with experiential notes.
5. After real sessions exist, update the Ring 1 audit with findings and sample limits; do not mark dependability from instrumentation alone.

## Tests

### CLI and filesystem

- no arguments leaves tracing disabled;
- `-T` and `--playtest-trace` choose an injected deterministic default;
- short, long, long-equals, and separated explicit-path forms agree;
- duplicate options, unknown options, and unexpected positionals reject before TUI startup;
- parent directories are created;
- existing files are never overwritten;
- default collisions receive a suffix; and
- help text documents the default and explicit forms.

### Event semantics

- sequence numbers are monotonic and elapsed time never decreases;
- one application dispatch produces one intent/result pair where applicable;
- each committed direct or batch tick produces exactly one `TickAdvanced` event;
- rejected ticks and atomic application failures do not fabricate committed deltas;
- draft retain/invalidate/cancel paths are distinguished;
- repeated rendering and wake events produce no analytic noise;
- selection events emit only when semantic selection changes; and
- orderly quit emits `TraceEnded` after the final observed gameplay event.

### Knowledge safety

Use small hand-computable Tier 1 fixtures to prove:

- probe traces omit unidentified intermediate identities and pending report contents;
- founding traces omit physical success/loss and daughter state before approved receipt;
- trace milestones occur only when the corresponding fact enters the player-safe view; and
- no production trace path accesses `test-support` diagnostics.

### Summary

- a short constructed event stream produces exact counts and milestone intervals;
- possible banking windows remain explicitly inferred;
- incomplete streams can be summarized without inventing `TraceEnded`;
- duplicate or out-of-order sequence values reject analysis; and
- summary ordering and RON serialization are deterministic for equal event streams.

## Documentation updates after implementation review

Before merge approval:

- update `docs/architecture.md` with the opt-in local observer/file-sink boundary;
- update executable play instructions with `-T`, `--playtest-trace`, explicit paths, and the ignored default directory;
- update [current Terminal Interactions](../design/current/terminal-interactions.md) if tracing introduces visible status or error behavior, and update the [Terminal UX Review Checklist](../tui-ux-guidelines.md) only if the review procedure changes;
- update the [Ring 1 Maturity Audit](2026-07-23-ring-1-maturity-audit.md) only after actual playtest evidence is reviewed; and
- add the user-visible opt-in trace mode to `CHANGELOG.md` under `Unreleased`.

No gameplay-mechanics contract or direction document needs a semantic update merely because evidence collection exists. Any later conclusion that changes banking, allocation, scouting, or expansion behavior requires separate design review.

## Acceptance

- ordinary `4x-term` startup and gameplay are behaviorally unchanged when tracing is disabled;
- `4x-term -T` creates a non-overwriting trace at the documented default path;
- explicit short and long path forms work;
- the trace and summary contain only player-safe semantic data;
- every committed tick, including paced batch ticks, is represented exactly once;
- recorder failures are visible and terminal cleanup remains correct;
- deterministic Tier 1 redaction and summary tests pass;
- formatting, all-target/all-feature checking, Clippy with warnings denied, and all-feature workspace tests pass; and
- dependency inspection confirms no network, async runtime, database, tracing framework, date/time crate, or new crate boundary was added.

## Exclusions

- remote telemetry, upload, accounts, consent servers, or analytics dashboards;
- raw keystroke capture, screen recording, or player-authored text capture;
- save files, replay, command replay, or restoration from traces;
- authoritative simulation snapshots or hidden-information diagnostics;
- statistical balance claims from small samples;
- generated-seed quality gates, survival gates, or content rejection;
- automated player-policy implementation;
- in-game annotation/editor UI; and
- changing gameplay, tuning, or design semantics in response to unreviewed metrics.
