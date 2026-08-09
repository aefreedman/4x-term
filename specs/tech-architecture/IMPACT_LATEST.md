# Impact Assessment

## Target

The crate-internal ship-movement seam in `game-core`: the sole caller's construction of `MovementPhaseContext` for ship travel, arrival or loss resolution, observation creation, and due-transmission receipt during each global tick.

The proposed structural change makes movement a crate-visible `WorldState` operation and keeps `MovementPhaseContext` private as a borrow-management implementation detail.

## Dependents (3)

- `crates/game-core/src/simulation.rs`: the only caller constructs all eleven context fields for the ship-movement and information-delivery stage of the transactional tick.
- `crates/game-core/src/ships.rs`: `resolve_expedition_arrival` consumes the context privately.
- `crates/game-core/src/ships.rs`: `observe_stop` consumes the context privately.

No external crate, public re-export, content schema, or player-facing adapter depends on this seam.

## Affected Stories

- No active release story owns this refactor.
- Archived epic `e00`, “Repository baseline,” records the existing Stage 4B/Stage 5 movement, founding, population, observation, and delayed-information capabilities that must remain unchanged.

## Test Coverage

- `crates/game-core/tests/global_tick_integration.rs`: covers exact global phase order, arrival, observation, delayed receipt, retention, rollback-sensitive accounting, and population reconciliation through `WorldState::advance_tick`.
- `crates/game-core/tests/ships_expansion.rs`: covers probe and expedition movement, founding success and loss, deterministic simultaneous-arrival order, population transfer, and player-safe mission outcomes through the public world interface.
- `crates/game-core/src/stage5_boundary_tests.rs`: covers launch-to-movement behavior and player-safe projections.
- Gap: no test calls the crate-internal movement seam directly. This is intentional because `WorldState::advance_tick` is the test surface and the refactor removes knowledge of the internal field partition.

## Risk: Low

There is one production caller, the remaining dependents are private helpers in the same implementation module, and deterministic behavior is covered through the established transactional tick interface.

## Recommended action

Proceed with one structural commit. Add no implementation-coupled test unless behavior changes. Run focused global-tick and ship-expansion tests, then the complete workspace preflight.
