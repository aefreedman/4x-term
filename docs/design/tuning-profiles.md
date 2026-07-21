---
title: "Tuning Profiles"
type: design
status: approved
---
# Tuning Profiles

Gameplay and generator tuning are strict authored RON content under `content/profiles/`. The engine has no balance fallbacks or implicit profile defaults. A world uses one validated world-level configuration shared by all systems.

See also:

- [World generation](world-generation.md)
- [Generator identity](generator-identity.md)
- [Stage 4b implementation plan](../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md)

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

The following generation bounds are fixed design contracts rather than missing defaults:

| Property | Origin | Frontier |
| --- | --- | --- |
| Strength | exactly `1.00` | triangular `0.10/1.00/3.00` |
| Eccentricity | exactly `1.00` per body | triangular `0.00/1.00/1.50` |
| Body count | discrete triangle `4/4/12` | discrete triangle `1/4/12` |
| Slots per body | discrete triangle `3/3/8` | discrete triangle `1/3/8` |
| Deposit-bearing resource presence | mandatory | configured independently per resource |
| Generated `z` | `0` | `0` |

Triangle notation throughout this page is `minimum/mode/maximum`.

## Shipped `starter` profile

`starter` is an explicitly selected iteration baseline. It is not a canonical world, preferred seed, generation-quality target, or acceptance oracle.

### Retained resource-engine tuning

| Parameter | `starter` value |
| --- | --- |
| Life support | `10 Energy` per population per tick |
| Free origin construction work | `1` per tick |
| Intrinsic Energy retention | `10` |
| Functional Battery retention | `+100` each |
| Collector recipe | `10 Energy + 2 Alloy`, `4 work` |
| Battery recipe | `10 Energy + 2 Alloy`, `4 work` |
| Extractor recipe | `10 Energy + 2 Alloy`, `4 work` |
| Refinery recipe | `10 Energy + 2 Ore`, `4 work` |
| Extractor operation | `10 Energy`, `1`-tick cycle, `1 Ore` output |
| Refinery operation | `10 Energy`, `1`-tick cycle, `2 Ore -> 1 Alloy` |

The detailed retained behavior—funding, FIFO construction, cancellation,
retention, overflow, shortages, and production atomicity—remains recorded in the
[completed Stage 4 plan](../plans/2026-07-20-feature-constructive-world-generation-stage-4-plan.md)
until those contracts receive their own durable design page.

### Construction and ship projects

| Item | Complete material commitment | Work/duration | Per-progress-tick Energy |
| --- | --- | ---: | ---: |
| Habitat | `40 Energy + 4 Alloy` | `8 work` | none |
| Shipyard | `80 Energy + 8 Alloy` | `12 work` | none while idle |
| Probe project | `20 Energy + 2 Alloy` | `4 ticks` | `10 Energy` |
| Expedition hull | `40 Energy + 6 Alloy` | `8 ticks` | `10 Energy` |

An expedition project also commits:

- one deployable Collector using its normal `10 Energy + 2 Alloy` recipe; and
- founding stocks of `10 Energy + 10 Ore + 0 Alloy`.

Its complete enqueue commitment is therefore:

```text
60 Energy + 10 Ore + 8 Alloy
```

Per-progress-tick project Energy is operational spending and is not part of this material commitment.

Habitat population generation costs `500 Energy`. Habitats and idle Shipyards have no additional upkeep.

### Travel and information

| Parameter | `starter` value |
| --- | ---: |
| Probe maximum jump | `1_500` coordinate quanta |
| Expedition maximum jump | `1_000` coordinate quanta |
| Probe speed | `500` quanta/tick |
| Expedition speed | `250` quanta/tick |
| Probe travel Energy | `1` per `200` quanta |
| Expedition travel Energy | `1` per `100` quanta |
| Probe reveal radius | `1_500` quanta |
| Communication delay | `1` tick per `500` quanta |

Reveal radius is independent tuning and is not an alias for probe maximum jump. Probe and expedition jump limits, speeds, and Energy rates are separately authored.

The runtime formulas using these rates are documented in [Geometric reachability](world-generation.md#geometric-reachability).

### Generator parameters

| Parameter | `starter` value |
| --- | --- |
| Coordinate scale | `100` quanta per map unit |
| Target system count | `128`, including the origin |
| X bounds | `[-5_000, 5_000)` quanta |
| Y bounds | `[-5_000, 5_000)` quanta |
| Generated Z | `0` |
| Placement cells | `500 × 500` quanta |
| Noise octaves | `4` integer-noise octaves |
| Base wavelength | `4_000` quanta |
| Lacunarity | `2` |
| Persistence | `1/2` |
| Jitter | deterministic full-cell jitter |

Natural-deposit declarations:

| Resource | Naturally deposit-bearing |
| --- | --- |
| `core:ore` | yes |
| `core:energy` | no |
| `core:alloy` | no |

Ore generation parameters:

| Scope | Presence | Resource-bearing body count | Quantity per selected body |
| --- | ---: | ---: | ---: |
| Origin | mandatory | `1/2/4` | `200/300/500` |
| Frontier | `6_500` basis points | `1/1/4` | `50/200/500` |

Ore summary richness thresholds use aggregate initial generated quantity:

- `Poor`: `1..=199`;
- `Normal`: `200..=499`; and
- `Rich`: `500+`.

### Shared seasonal shape

The approved ten-phase Collector shape is:

```text
[40, 40, 30, 20, 10, 10, 20, 30, 40, 40]
```

Its baseline average is `28` and cycle total is `280`. Strength scaling, fixed-point rounding, eccentricity adjustment, and largest-remainder phase apportionment are specified in [Strength and seasonal output](world-generation.md#strength-and-seasonal-output).

## Editing and identity

Designers may change `starter` or add another complete profile without changing Rust code. Every profile and future resource must supply the same required validated fields.

A profile edit changes the normalized configuration fingerprint. The generator revision changes only when output-affecting generation behavior changes; see [Generator identity](generator-identity.md).

Mutable `starter` values must not be treated as universal generation guarantees. Validation should enforce ranges, references, arithmetic, and named invariants—not an exact output count, a favorable distribution, or the strategic quality of generated seeds.
