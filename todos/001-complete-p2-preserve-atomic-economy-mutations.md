---
status: complete
priority: p2
issue_id: "001"
tags: [code-review, data-integrity, economy]
dependencies: []
---
# Preserve atomic economy mutations

## Problem Statement

Checked arithmetic can fail after an economy operation has already mutated inventory. This violates the prototype's atomic transaction and recipe guarantees and can leave state partially updated when quantities approach numeric limits.

## Findings

- `GameSession::buy` removes market stock before checking whether adding the payment to market currency overflows. An overflow returns `CoreError::Overflow` after goods have already disappeared from the market. Evidence: `crates/game-core/src/lib.rs:742-762`.
- Recipe execution consumes every input before checking output additions. An overflowing output returns after inputs were consumed. Evidence: `crates/game-core/src/lib.rs:998-1014`.
- Cargo usage uses an unchecked `u32` sum, and successful cargo/ledger increments use unchecked addition. Evidence: `crates/game-core/src/lib.rs:738-740,774-777`.

Source: root data-integrity review of branch `loop/20260710-initial-prototype-implementation`. Confidence: high.

## Proposed Solution

**Approach:**
- Precompute and validate every resulting balance/inventory/ledger value before mutating either entity.
- Apply validated values only after all checks succeed.
- Add boundary tests that intentionally trigger overflow and compare complete before/after snapshots.

**Why this approach:**
- It preserves the specified all-or-nothing behavior without introducing rollback complexity.

**Trade-offs / risks:**
- Requires slightly more temporary values or a small transaction result structure.

## Recommended Action

Refactor buy, sell, source replenishment, and recipe execution to use validate-then-apply helpers, then add success and overflow atomicity tests.

## Technical Details

**Affected files/assets:**
- `crates/game-core/src/lib.rs` - economy mutation ordering and tests

**Related systems:**
- Market transactions
- Recipe and source processing

**Data/content impact:**
- Save data affected? No
- Serialized assets or prefabs affected? No
- Migration or content reimport needed? No

## Resources

- **Review/PR/changeset:** branch `loop/20260710-initial-prototype-implementation`
- **Documentation:** `docs/initial-prototype.md:185-206,268-272,512-526`

## Acceptance Criteria

- [x] Buy and sell validate all resulting numeric values before mutation.
- [x] Recipe/source processing cannot partially mutate inventory on overflow.
- [x] Cargo and ledger totals use checked arithmetic.
- [x] Tests prove full state equality before and after rejected overflow operations.
- [x] Workspace tests and Clippy pass.

## Work Log

### 2026-07-10 - Review finding

**By:** OpenAI Codex

**Actions:**
- Traced transaction and recipe mutation order.
- Confirmed concrete partial-mutation paths.

**Learnings:**
- Normal authored values do not trigger the defect, so explicit boundary fixtures are required.

### 2026-07-10 - Resolved

**By:** OpenAI Codex

**Actions:**
- Changed transactions to compute all resulting values before applying mutations.
- Changed sources and recipes to stage inventory updates before replacing ECS state.
- Added buy, sell, source, and recipe overflow regression tests.
- Validated with game-core tests and Clippy.

**Commits:** `69895f5`, `36d55ef`, `6258a3e`, `b85d26b`
