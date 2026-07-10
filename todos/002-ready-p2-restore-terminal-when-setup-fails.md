---
status: ready
priority: p2
issue_id: "002"
tags: [code-review, architecture, tui]
dependencies: []
---
# Restore terminal when setup fails

## Problem Statement

Terminal raw mode is enabled before the RAII guard exists. If entering the alternate screen or hiding the cursor fails, `TerminalGuard::enter` returns early and leaves raw mode enabled.

## Findings

- `enable_raw_mode()` succeeds before `execute!(EnterAlternateScreen, Hide)` runs.
- `TerminalGuard` is only returned after both calls, so its `Drop` cleanup cannot run for the second-call failure path.
- Evidence: `crates/game-tui/src/lib.rs:62-66`.

Source: root architecture/lifecycle review of branch `loop/20260710-initial-prototype-implementation`. Confidence: high.

## Proposed Solution

**Approach:**
- Construct a guard immediately after raw mode succeeds, then attempt remaining setup while the guard is live.
- Track which terminal states were successfully changed so cleanup is idempotent and only reverses applicable operations.
- Exercise setup failures through an injectable terminal-operations abstraction or focused helper tests.

**Why this approach:**
- It guarantees cleanup on every `?` path and makes lifecycle behavior testable without a real TTY.

**Trade-offs / risks:**
- Adds a small amount of state to the guard.

## Recommended Action

Refactor `TerminalGuard::enter` into staged acquisition and add setup-failure cleanup tests before manual TTY acceptance.

## Technical Details

**Affected files/assets:**
- `crates/game-tui/src/lib.rs` - terminal acquisition and cleanup

**Related systems:**
- Crossterm lifecycle
- Panic/error recovery

**Data/content impact:**
- Save data affected? No
- Serialized assets or prefabs affected? No
- Migration or content reimport needed? No

## Resources

- **Review/PR/changeset:** branch `loop/20260710-initial-prototype-implementation`
- **Documentation:** `docs/initial-prototype.md:476-478`

## Acceptance Criteria

- [x] Any failure after raw mode is enabled restores raw mode before returning.
- [x] Cleanup remains safe and idempotent after partial and complete setup.
- [x] Automated tests cover each staged setup failure without a real TTY.
- [ ] Manual normal-quit and forced-error checks restore the shell.

## Work Log

### 2026-07-10 - Review finding

**By:** OpenAI Codex

**Actions:**
- Reviewed terminal setup, guard construction, drop, and panic-hook paths.

**Learnings:**
- Drop-based cleanup only protects resources acquired after guard construction.

### 2026-07-10 - Partial resolution

**By:** OpenAI Codex

**Actions:**
- Refactored terminal acquisition into staged, tracked operations with reverse-order cleanup.
- Added fake-terminal tests for alternate-screen failure, cursor-hide failure, and successful cleanup.
- Validated game-tui tests and Clippy in commit `6b56cc8`.
- Attempted an Expect-based pseudo-TTY smoke run; Ratatui could not read cursor position from that harness, returned an error, and emitted cursor/alternate-screen restoration sequences.

**Disposition:** Partial. Automated failure cleanup is resolved, but real-terminal normal-quit validation remains required. Next step: run `cargo run -p game-cli` in a real interactive terminal, press `q`, and confirm shell modes with `stty -a`.
