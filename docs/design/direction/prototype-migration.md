---
title: Prototype Migration Boundaries
type: design-direction
status: approved
authority: directional
horizon: long-term
design_ids:
  - prototype.compatibility-not-required
  - economy.physical-energy
  - economy.life-support-margin
  - economy.physical-production-inputs
  - logistics.physical-resource-transfer
  - economy.reconciled-arithmetic
tags:
  - migration
  - economy
  - physicality
---
# Prototype Migration Boundaries

## Superseded trader-first direction

The authored market network and autonomous trader ecology were implementation experiments, not product contracts. Surviving seasonal, shortage, population, and logistics concepts are justified by the governance-and-expansion game rather than inherited from that prototype.

## Compatibility is not a design constraint

Keeping the workspace buildable during migration does not require preserving trader gameplay, authored trade routes, market makers, pricing, wallets, commercial delivery contracts, or associated UI and diagnostics. Remove a conflicting prototype system rather than preserving it as an accidental product requirement. Git history is the archive.

## Retained economic substrate

The durable substrate is represented by focused current contracts:

1. [Energy](../current/energy-and-seasons.md) is a physical, discrete survival resource rather than universal money.
2. [Population and life support](../current/population-and-habitats.md) create unavoidable pressure; Energy above it becomes allocable margin.
3. [Extraction and production](../current/systems-and-resources.md) consume Energy and physical inputs to create capability.
4. [Ships and expansion](../current/ships-and-expansion.md) move physical resources with capacity, time, and Energy consequences; resources do not teleport.
5. [Simulation timing](../current/simulation-timing.md) keeps arithmetic checked, deterministic, atomic, and exactly reconcilable.

Future economy content should serve survival, governance, development, and expansion. Exact chains, specialist substrates, and additional goods remain [exploratory](../ideas/production-and-capability.md).
