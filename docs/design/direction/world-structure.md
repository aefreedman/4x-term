---
title: World Structure and Generation Direction
type: design-direction
status: approved
authority: directional
horizon: long-term
design_ids:
  - world.player-origin-only
  - world.preexisting-remnants
tags:
  - world-generation
  - origin
  - remnants
  - testing
---
# World Structure and Generation Direction

## Player origin

The generated world begins with one player origin. Living neighbors arise from player-community founding or later recovery mechanics rather than being required world-generation witnesses. Current revision 1 may otherwise contain empty geography and physical resources; lore about a dead precursor world does not imply that every ruin mechanic already exists.

## Pre-existing world texture

Long-term generation should contain pre-existing things with histories and strategic texture, analogous to generated artifacts and sites in simulation-led worlds. Precursor remnants are one expression of this outcome. A fixed two-type resource/site ruin taxonomy, generated accumulation simulation, body suitability, and reclamation rules remain ideas rather than commitments.

## Orthogonal physical axes

World texture comes from physical axes such as stellar Energy supply, development capacity, and material-resource profile varying independently. The strategic value lies in their disagreement: capacity may need imported Energy, or abundant Energy may lack local material opportunity. Current bodies and slots remain generic; this direction does not commit to body types or slot modifiers.

## Constructive generation

Generation constructs the approved origin scaffold directly and then generates frontier texture. It does not select a favorable origin after the fact or require a neighborhood viability witness. The exact current scaffold, reviewed ranges, and revision contract are owned by [World Generation](../current/world-generation.md) and [Frontier Generator Revision 1](../current/generator-revision-1.md).

## Generated-world testing

Generated-world tests verify deterministic mechanics, identity, references, ranges, arithmetic, and named constructive guarantees. They do not play worlds, reject seeds for qualitative outcomes, or impose statistical desirability thresholds. Difficult, disconnected, sparse, or locally non-viable outcomes are valid texture unless a named invariant is violated.

Gameplay-facing behavior instead requires small authored, hand-computable scenarios. A result visible only through a soak is simulation behavior, not a gameplay acceptance test.
