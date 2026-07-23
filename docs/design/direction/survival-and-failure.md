---
title: Survival and Failure
type: design-direction
status: approved
authority: directional
horizon: long-term
design_ids:
  - failure.world-absorbed
  - simulation.local-collapse-permitted
tags:
  - survival
  - failure
---
# Survival and Failure

## Local collapse is world texture

The world must be mechanically possible, not screened for stability. Local collapse is permitted and expected. A difficult or non-viable generated region is not a bug unless it violates a named engine invariant or constructive generation contract.

This principle is already reflected in [current world generation](../current/world-generation.md#valid-outcomes-and-non-goals) and repository testing guidance. It must not be expanded into a statistical world-quality gate.

## World-absorbed failure

Long term, valid gameplay failure should become persistent world state rather than an out-of-world terminal rejection. Communities may fail through the same simulation rules that pressure every other community, leaving history and possible future recovery content.

This is a low-priority destination, not a current persistence contract. Exact ruin transitions, reclamation, origin succession, run boundaries, and cross-run state remain [exploratory](../ideas/expeditions-and-reclamation.md#failure-persistence-and-succession). Implementations must not invent those mechanisms merely because the directional principle is approved.
