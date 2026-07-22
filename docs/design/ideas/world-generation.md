---
title: "World Generation Ideas"
type: design-idea
status: draft
authority: non-authoritative
horizon: exploratory
tags:
  - world-generation
  - geometry
---
# World Generation Ideas

## Three-dimensional frontier positions

Current generator revision 1 places the frontier in two dimensions with `z = 0`
while retaining a three-coordinate position type. A future revision could use a
true three-dimensional volume if vertical separation creates enough strategy to
justify presentation and navigation costs.

Before promotion, define the decision added by the third axis, projection and
depth presentation, volume dimensions relative to density and jump ranges,
deterministic 3D sampling and separation, checked distance bounds, and focused
route/projection scenarios. Connectivity and visual preference must not become
generated-world quality gates.

## Precursor accumulation generation

World texture might be produced by a crude precursor history pass that leaves
resource hubs, collector infrastructure, or other remnants according to what the
precursors optimized. This could connect [canonical lore](../lore/precursor-aftermath.md)
to the [direction for pre-existing world texture](../direction/world-structure.md#pre-existing-world-texture).

It remains exploratory. A proposal must define generated facts, revision
identity, constructive guarantees, observability, and failure fixtures without
screening seeds for desirability.
