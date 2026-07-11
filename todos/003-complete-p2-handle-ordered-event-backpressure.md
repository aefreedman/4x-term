---
status: complete
priority: p2
issue_id: "003"
tags: [code-review, architecture, async]
dependencies: []
---
# Handle ordered event backpressure

## Problem Statement

The application contract says ordered events must not be silently coalesced, but the owner drops events whenever the bounded event channel is full.

## Findings

- `collect_events` calls `output.try_send(event)` and ignores the result. A full or closed channel silently discards the event. Evidence: `crates/game-app/src/lib.rs:236-245`.
- The TUI drains this event channel but renders event history from the `watch` snapshot, making the second event path redundant for the current frontend.
- This conflicts with the architecture's ordered-event channel guarantee in `docs/architecture.md:345-351`.

Source: root architecture review of branch `loop/20260710-initial-prototype-implementation`. Confidence: high.

## Proposed Solution

**Approach:**
- Prefer removing the redundant event receiver from `AppHandle` and treating bounded history in `ApplicationView` as the explicit UI event contract; or
- Define and test an explicit backpressure/overflow policy if independent ordered consumers are still required.

**Why this approach:**
- Removing the unused delivery path is simpler and avoids claiming lossless behavior that is not implemented.

**Trade-offs / risks:**
- Removing the channel postpones a dedicated event-stream API until a real consumer requires it.

## Recommended Action

Remove the redundant event channel for this prototype, update architecture wording to match snapshot history, and test bounded event-history retention. Keep a dedicated event stream only if a concrete ordered consumer is added.

## Technical Details

**Affected files/assets:**
- `crates/game-app/src/lib.rs` - event publication contract
- `crates/game-tui/src/lib.rs` - redundant event receiver branch
- `docs/architecture.md` - async channel contract

**Related systems:**
- Async application owner
- TUI event log

**Data/content impact:**
- Save data affected? No
- Serialized assets or prefabs affected? No
- Migration or content reimport needed? No

## Resources

- **Review/PR/changeset:** branch `loop/20260710-initial-prototype-implementation`
- **Documentation:** `docs/architecture.md:326-356`

## Acceptance Criteria

- [x] No ordered-event API silently discards events.
- [x] The selected UI event-history policy is bounded and documented.
- [x] Tests verify retention and overflow behavior.
- [x] TUI remains responsive and renders recent simulation/rejection events.

## Work Log

### 2026-07-10 - Review finding

**By:** OpenAI Codex

**Actions:**
- Compared channel implementation with the documented async contract.
- Confirmed full-channel errors are ignored.

**Learnings:**
- The current TUI does not need both a watch-contained history and a separate event stream.

### 2026-07-10 - Resolved

**By:** OpenAI Codex

**Actions:**
- Removed the redundant lossy event channel and TUI receiver branch.
- Kept bounded recent history in immutable watch snapshots as the prototype contract.
- Added a retention-cap test and updated architecture/prototype documentation.
- Validated game-app and game-tui tests and Clippy.

**Commit:** `6b56cc8`
