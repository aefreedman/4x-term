---
title: Staged RAII Cleanup for Crossterm Setup
tags: [rust, tui, crossterm, lifecycle]
module: game-tui
problem_type: resource-lifecycle
---
# Staged RAII Cleanup for Crossterm Setup

## Problem

Enabling raw mode and then entering the alternate screen before constructing a guard leaves the terminal damaged if the second operation fails. A `Drop` implementation cannot clean up resources acquired before the guard exists.

## Solution

Construct the guard first with flags for each acquired terminal state. Perform setup through the guard one operation at a time, setting each flag only after success. On failure or normal drop, reverse only the completed operations:

```text
enable raw      → raw = true
enter alternate → alternate = true
hide cursor     → cursor_hidden = true

cleanup: show cursor → leave alternate → disable raw
```

Put terminal operations behind a small internal trait. Fake operations can then force failure at each stage and verify cleanup order without requiring a real TTY.

## Evidence

- Implementation: `crates/game-tui/src/lib.rs`
- Regression tests: `partial_terminal_setup_is_cleaned_up`, `complete_terminal_setup_is_cleaned_up_in_reverse_order`
- Resolution commit: `6b56cc8`

## Applicability

Use this pattern whenever terminal or platform setup consists of multiple fallible state changes. A panic hook remains useful as a final fallback, but it does not replace staged ownership.
