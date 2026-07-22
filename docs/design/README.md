---
title: "Game Design Wiki"
type: design-index
status: active
---
# Game Design Wiki

These pages are the durable gameplay and world-model contracts for current and
future stages. Implementation plans should link to the relevant pages rather
than duplicate their rules.

## World and generation

- [World Generation](world-generation.md) — constructive origin, procedural
  frontier, bodies, resources, positions, and geometric reachability.
- [Generator Identity](generator-identity.md) — seed/version/profile identity,
  fingerprints, revisions, and generated artifacts.
- [Frontier Generator Revision 1](generator-revision-1.md) — exact canonical
  bytes, random streams, distributions, integer noise, placement, and test
  vectors for `core:frontier_world@1`.
- [Tuning Profiles](tuning-profiles.md) — designer-authored RON configuration,
  fixed design ranges, and the editable `starter` profile.

## Simulation model

- [Systems and Resources](systems-and-resources.md) — map/runtime ownership,
  body resources, developments, stocks, founded systems, and accounting.
- [Energy and Seasons](energy-and-seasons.md) — strength, eccentricity,
  Collector output, Energy priority, spending, and retention.
- [Population and Habitats](population-and-habitats.md) — population tokens,
  Habitat capacity/generation, commandability, movement, founding, and loss.
- [Simulation Timing](simulation-timing.md) — global tick phases, ordering,
  activation, travel timing, atomicity, stable IDs, and reconciliation.

## Information and expansion

- [Scouting and Knowledge](scouting-and-knowledge.md) — knowledge levels,
  initial information, probes, observations, transmissions, and fact merging.
- [Ships and Expansion](ships-and-expansion.md) — Shipyard projects, ship
  travel, expedition commitments, settlement, failure, and remote control.

## Direction and evidence policy

- [Architecture](../architecture.md)
- [Completed Stage 4 Resource-Engine Plan](../plans/2026-07-20-feature-constructive-world-generation-stage-4-plan.md)
- [Governance Sandbox](../2026-07-20-design-direction-governance-sandbox.md)
- [Testing Stance](../plans/2026-07-20-testing-stance-correction.md)
- [Engine Invariant Registry](../2026-07-20-engine-invariant-registry.md)
- [Future Feature Ideas](../ideas.md)
