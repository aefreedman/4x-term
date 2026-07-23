---
title: Tuning Profiles
type: design-current
status: approved
authority: normative
horizon: current
tags:
  - tuning
  - profiles
  - configuration
  - validation
---
# Tuning Profiles

Gameplay and generator tuning are strict authored RON content under `content/profiles/`. The engine has no balance fallbacks or implicit profile defaults. A world uses one validated world-level configuration shared by all systems.

See also:

- [World generation](world-generation.md)
- [Generator identity](generator-identity.md)
- [Stage 4b implementation plan](../../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md)

## Profile contract

A profile must explicitly provide all gameplay and generator fields needed by its revision. Required gameplay tuning includes:

- construction recipes and work costs for Habitats and Shipyards;
- probe and expedition project commitments, durations, and per-progress-tick Energy;
- expedition founding stocks and deployable Collector recipe;
- Habitat population-generation cost;
- ship jump limits, speeds, and travel-Energy rates;
- probe reveal radius;
- communication rate;
- ordered `Poor`/`Normal`/`Rich` thresholds;
- capacities, upkeep, retention, overflow, and action tuning used by the resource engine; and
- fixed-point coordinate resolution.

Required generator tuning includes:

- target system count, including the mandatory origin;
- centered 2D X/Y bounds and cell dimensions;
- fixed-point coordinate resolution;
- validated integer noise, density, and jitter parameters;
- explicit natural-deposit eligibility for each resource; and
- separate origin and frontier parameters for every deposit-bearing resource: system presence where applicable, resource-bearing-body-count triangle, nonzero per-body quantity triangle, and placement inputs.

Frontier presence is an integer basis-point value in `0..=10_000`. Origin presence is mandatory and does not use a presence roll. Resource-bearing-body counts are truncated and renormalized to the generated body's valid range rather than clamped.

Reviewed origin/frontier ranges and structural guarantees are fixed design
contracts rather than missing defaults. Their single canonical statement is
[World Generation](world-generation.md#constructive-origin-contract); this page
owns the profile schema and validation semantics, not a duplicate range table.
Changing a reviewed range requires design review even when its active
representation is configuration.

## Shipped `starter` profile

`starter` is an explicitly selected iteration baseline. It is not a canonical
world, preferred seed, generation-quality target, or acceptance oracle. Its
active mutable values are owned by
[`content/profiles/starter.ron`](../../../content/profiles/starter.ron); this page
does not reproduce them.

The profile supplies complete values for:

- life support, free work, retention, development recipes, and production;
- Habitat, Shipyard, probe, expedition, and founding commitments;
- jump limits, speeds, travel-Energy rates, reveal radius, and communication
  rate;
- frontier density, coordinate bounds, cells, integer noise, and jitter;
- resource declarations, origin/frontier deposit distributions, and summary
  thresholds; and
- seasonal shape and baseline.

These values have distinct semantics even when a shipped profile currently makes
them equal. Reveal radius is not an alias for probe jump range; probe and
expedition movement values are independently authored; project material
commitment is distinct from per-progress-tick and launch Energy; and naturally
deposit-bearing declarations are resource-specific.

The durable behavior using those values is owned by the focused current pages:

- [Systems and Resources](systems-and-resources.md) — funding, construction,
  production, physical ownership, and reconciliation;
- [Energy and Seasons](energy-and-seasons.md) — production, spending priority,
  retention, and overflow;
- [Population and Habitats](population-and-habitats.md) — generation progress and
  life support;
- [Ships and Expansion](ships-and-expansion.md) — project and founding
  commitments; and
- [World Generation](world-generation.md) — reviewed ranges, placement,
  reachability, and generated knowledge.

## Editing and identity

Designers may change `starter` or add another complete profile without changing Rust code. Every profile and future resource must supply the same required validated fields.

A profile edit changes the normalized configuration fingerprint. The generator revision changes only when output-affecting generation behavior changes; see [Generator identity](generator-identity.md).

Mutable `starter` values must not be treated as universal generation guarantees. Validation should enforce ranges, references, arithmetic, and named invariants—not an exact output count, a favorable distribution, or the strategic quality of generated seeds.
