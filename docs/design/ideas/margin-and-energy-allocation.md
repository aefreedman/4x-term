---
title: Margin and Energy Allocation Ideas
type: design-idea
status: draft
authority: non-authoritative
horizon: exploratory
tags:
  - energy
  - margin
  - governance
  - core-loop
---
# Margin and Energy Allocation Ideas

The committed core loop asks the player to allocate physical margin among banking, development, and expansion. Current Energy behavior instead follows a fixed global spending order. The mechanisms below are possibilities, not approved changes to that order.

## Per-system Energy reserve policy

A command could set a reserve floor for a commandable system. Selected discretionary activities would operate only from Energy available above that floor, making **bank** an explicit player decision rather than an outcome inferred from fixed spending order.

A narrow first design should preserve the current simulation phases and stable contention order. It should identify exactly which consumers respect the floor; plausible candidates include Shipyard progress, empty-Habitat population generation, and general construction. Collector production, life support, retention, receipts, overflow, and command-time launch costs must receive explicit treatment rather than inheriting ambiguous behavior.

The reserve must remain physical Energy in the system stock. It must not become a second treasury, an abstract solvency score, or a guarantee that future needs can be met. Commands and ticks must retain their existing atomicity and checked accounting.

## Questions before promotion

Before this idea can become current design, define:

- whether the floor protects Energy from all discretionary consumers or from an explicit reviewed subset;
- whether Extractors and Refineries respect it, given their place in production chains;
- whether explicit command-time spending may cross the floor after confirmation or is rejected mechanically;
- how a player changes or clears the floor and when the change takes effect;
- how reserve behavior interacts with insufficient Energy, seasonal production, retention capacity, and overflow;
- what authoritative runtime state and accounting expose about Energy withheld by policy; and
- how the TUI distinguishes current stock, protected reserve, and spendable margin without presenting a false runway guarantee.

## Promotion evidence

A proposal should include short deterministic scenarios covering:

1. a protected reserve pausing an eligible consumer while life support still resolves;
2. multiple eligible consumers retaining current stable ordering above the floor;
3. seasonal production moving a system above and below the floor;
4. changing or clearing the floor between ticks;
5. command-time launch or construction behavior at the boundary; and
6. tick failure leaving the policy, stock, accounting, and all consumers unchanged.

Any approved version must update [Energy and Seasons](../current/energy-and-seasons.md) and [Simulation Timing](../current/simulation-timing.md). Its purpose should remain aligned with [Margin and Capability](../direction/margin-and-capability.md), not merely reduce overflow or automate play.
