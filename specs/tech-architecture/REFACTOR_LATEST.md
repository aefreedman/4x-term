# Refactor Plan

## Problem Statement

The global tick implementation currently constructs `MovementPhaseContext` by naming eleven fields owned by `WorldState`, then passes that field bag to the ship-movement implementation. This makes the tick orchestrator know how movement partitions world state even though there is only one caller and no test uses that seam directly.

The runtime behavior is not wrong. The architectural problem is a shallow crate-internal module: its interface exposes nearly as much state knowledge as its implementation needs. That reduces locality and makes the simulation ordering code harder to read.

The preservation invariant is:

> `WorldState::advance_tick` must preserve ship movement behavior. Ships process in stable ship-ID order. Each ship advances at most one leg tick. Expedition settlement or loss occurs before observation. Information is scheduled and received at the same ticks. Population and accounting invariants remain intact. Movement remains inside the atomic whole-tick transaction.

## Solution

Deepen the existing world simulation module by making ship movement a crate-visible operation on `WorldState`. The global tick implementation will request ship movement without constructing its internal field partition.

Keep `MovementPhaseContext` as a private borrow-management implementation detail in the ship implementation. This gives callers a smaller interface without forcing a larger mechanical rewrite. Replace the isolated numbered-phase description with semantic wording that explains the behavior without requiring the reader to know the complete tick sequence.

## Commits

1. Deepen the crate-internal ship-movement seam: add the world-owned movement operation, move context construction behind it, make the context and free implementation private, simplify the global tick caller, and clarify the movement description → verify: `cargo test -p game-core --all-features --test global_tick_integration --test ships_expansion && cargo test -p game-core --all-features --lib stage5_boundary_tests`

After the focused verification, run the repository preflight before review: `cargo fmt --all -- --check && cargo check --workspace --all-targets --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`.

## Decision Document

- Modify the world simulation orchestration module and the ship implementation module only.
- Replace the eleven-field crate-internal movement interface with one world-owned operation.
- Keep the movement context private inside the implementation rather than deleting it; it remains useful for borrow management and private helper reuse.
- Preserve the existing public world interface and all player-information rules.
- Preserve the approved global tick ordering and atomic transaction.
- Use semantic movement language outside the contextual numbered tick list.
- Add no module, adapter, dependency, crate, or import edge.

## Testing Decisions

- The interface is the test surface: verify behavior through `WorldState::advance_tick`, not through the private movement implementation.
- Existing global-tick integration tests cover ordering, arrival, observation, due-transmission receipt, Energy retention, accounting, and population reconciliation.
- Existing ship-expansion tests cover probe and expedition travel, founding success and loss, deterministic simultaneous arrivals, and player-safe outcomes.
- Existing Stage 5 tests cover launch-to-movement behavior and projections.
- Add no implementation-coupled test for the removed shallow seam. Add a regression test only if implementation reveals an observable behavior not already covered.

## Out of Scope

- Changing movement, travel duration, settlement, loss, observation, transmission, population, accounting, or tick-order semantics.
- Deleting the private movement context or rewriting its private helpers.
- Deepening population transitions or launch assessment behavior.
- Changing the numbered phase list in the current simulation-timing design authority.
- Adding a trait, adapter, dependency, crate, or new public interface.
- Broad reformatting or unrelated cleanup.

## Further Notes

The impact assessment classifies this as low risk because there is one production caller and existing deterministic tests exercise the behavior through the established world interface.

Implementation commit: `b50c7f9 refactor(core): deepen ship movement boundary`. Verification evidence commit: `797c8c0 chore(verify): record ship movement evidence`.
