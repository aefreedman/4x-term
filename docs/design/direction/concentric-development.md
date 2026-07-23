---
title: Concentric Development
type: design-direction
status: approved
authority: directional
horizon: long-term
founded: 2026-07-23
tags:
  - development-strategy
  - prioritization
  - core-loop
  - vertical-slices
---
# Concentric Development

## Purpose

Concentric development means building the core first, polishing it enough to be stable, and then working outward. It is the default rule for prioritizing design, implementation, testing, and player-facing polish.

The ordinary order is:

1. identify primary mechanics;
2. finish, expose, polish, and test them until they are dependable;
3. add secondary mechanics that deepen or scale them; and
4. add tertiary mechanics and supporting content.

This reduces instability, makes playtests more trustworthy, reveals scope problems earlier, and preserves clean cuts when time or attention is limited. It rejects building many features on wobbly fundamentals, deferring polish on central interactions, or treating every mechanic as equally important.

Concentric development is a prioritization model, not a prohibition on vertical slices. A deliberately bounded slice may cut through all three rings to test whether a possible experience is viable.

## Ring 1: primary game loop

Ring 1 is the smallest complete version of the intended game. It includes the full governor cycle:

```text
bootstrap
  -> produce and survive
  -> bank or develop
  -> scout
  -> prepare expansion
  -> launch
  -> found a settlement
  -> receive and understand the outcome
  -> continue governing
```

The repeated decision loop is:

1. observe the community's physical state and available knowledge;
2. assess survival pressure and available margin;
3. choose whether to bank, develop, or expand;
4. commit physical resources, Energy, population, and time;
5. advance the deterministic simulation; and
6. understand the consequences well enough to make the next decision.

Ring 1 therefore includes the minimum player-facing forms of:

- Energy production, seasons, consumption, retention, and shortage;
- life support, population, Habitats, extraction, refinement, and construction;
- scouting and knowledge sufficient to choose an expansion target;
- Shipyard projects, expeditions, physical travel, and founding;
- delayed receipt of scouting and founding outcomes;
- continued governance of the origin and founded systems; and
- TUI controls and feedback required to perform and understand the complete cycle.

Simulation implementation alone does not complete a primary mechanic. Because the TUI is the human play surface, player-facing interaction and legibility are part of Ring 1 rather than secondary presentation work.

### Ring 1 dependability

A primary mechanic advances through these maturity states:

| State | Required evidence |
| --- | --- |
| **Contracted** | Approved design defines the behavior and its boundaries. |
| **Implemented** | The headless simulation and application boundary support it. |
| **Exposed** | The normal TUI can inspect and exercise it. |
| **Legible** | Choices, commitments, limiting reasons, and consequences are understandable in play. |
| **Verified** | Short deterministic scenarios cover its mechanical contract. |
| **Dependable** | Repeated full-cycle play no longer exposes foundational design or interaction instability. |

Ring 1 is dependable only when the complete cycle is playable through the TUI; bank, develop, and expand create meaningful competing commitments; consequences are attributable; primary behavior has deterministic evidence; and removing every outer-ring mechanic still leaves a coherent game.

## Ring 2: secondary mechanics

Ring 2 mechanics deepen, differentiate, or scale decisions already established by Ring 1. Possible examples include expedition loadouts, differentiated infrastructure, two-channel information, intersystem freight, remote operating policies, delegation by distance, and richer scouting layers.

Placement in Ring 2 does not approve an exploratory mechanism. It says that the mechanism should justify itself by improving an established primary decision rather than substituting for a missing one.

Before broad Ring 2 investment, the affected Ring 1 interaction should be dependable and the proposal should identify:

- the primary decision it improves;
- a demonstrated limitation in the existing loop;
- how it deepens or scales that decision;
- the stable inner behavior on which it depends;
- deterministic integration evidence; and
- a cut boundary if the added complexity does not earn its cost.

If Ring 2 work exposes Ring 1 instability, development moves inward to repair the primary interaction before broadening the secondary system.

## Ring 3: tertiary mechanics and supporting content

Ring 3 adds breadth, texture, adversity, replayability, recovery, or supporting content around established primary and secondary mechanics. Possible examples include specialists, precursor ruins, reclamation, environmental hazards, political drift, rescue, succession, persistent failure, tertiary goods, and broader content variation.

A Ring 3 proposal should depend on stable inner behavior, create recognizable new situations, reuse established physical rules where possible, and remain removable without breaking the primary loop. Its content, presentation, and testing costs are part of its scope.

Ring 3 is not synonymous with low value. It means that the feature earns priority after, or through a bounded slice with, the inner mechanics that make it meaningful.

## Vertical slices

A vertical slice is a deliberate radial cut through the three rings. It proves a potentially viable experience without first completing every mechanic in an inner ring or generalizing every outer-ring system.

A valid slice includes:

- enough Ring 1 behavior to exercise a complete player cycle;
- one narrowly selected Ring 2 mechanic that tests depth or scale; and
- only enough Ring 3 mechanic or content to test the intended texture.

Every slice must state:

1. the player-experience hypothesis being tested;
2. the Ring 1 interactions on which it depends;
3. the minimum Ring 2 extension needed for the test;
4. the fixture-scale or otherwise bounded Ring 3 content needed;
5. evidence that would justify further investment;
6. what can be deleted or deferred if the hypothesis fails; and
7. where work stops and moves inward if the slice reveals primary instability.

A slice is not permission to build speculative outer-ring infrastructure. Exploratory mechanics still require explicit approval before implementation, and accepted mechanics must be reconciled into current and direction documentation before merge approval.

## Feature relationships

Every proposed outer-ring feature must name its relationship to the inner game. Prefer explicit terms over saying that a feature merely "supports" another feature.

| Relationship | Meaning |
| --- | --- |
| **enables** | Makes an inner player verb possible. |
| **pressures** | Creates urgency or opportunity cost. |
| **competes with** | Draws from the same physical margin. |
| **informs** | Supplies evidence used by a decision. |
| **gates** | Requires capability, knowledge, or authority. |
| **carries** | Moves people, resources, information, or authority physically. |
| **scales** | Preserves an inner decision at greater scope. |
| **diversifies** | Adds situations without redefining the core loop. |
| **recovers** | Creates continued play from loss or damaged state. |

A proposal should answer which inner mechanic it serves, how it changes the player's decision, what stable behavior it requires, and what can be cut without weakening the inner loop.

## Independent classifications

Concentric position, design authority, and delivery maturity answer different questions and must not be conflated.

| Classification | Question |
| --- | --- |
| Ring | How should this mechanic relate to development order and inner stability? |
| Authority | Is it current, directional, or exploratory? |
| Maturity | Is it contracted, implemented, exposed, legible, verified, and dependable? |

An exploratory proposal may be a candidate Ring 1 repair without being approved. A committed long-term outcome may remain Ring 2 because it should be built only after the local interaction it scales is dependable.

## Roadmap use

A roadmap may contain two kinds of internal milestone:

- **foundation milestone** — closes maturity gaps or repairs instability in an inner ring; and
- **vertical-slice milestone** — tests a named hypothesis through a bounded radial slice.

A meaningful milestone describes a player-visible proof, not a subsystem inventory. Roadmap sequencing, confidence, and active scope belong outside design direction; this page supplies the durable prioritization rules by which those choices are judged.
