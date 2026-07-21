---
title: "Scouting and Knowledge"
type: design
status: approved
source: "../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md"
---
# Scouting and Knowledge

Scouting is the player's imperfect, delayed view of the generated frontier. The
origin knowledge store is the sole information authority: remote settlements do
not create independent copies of scouting knowledge, and direct control of an
owned system does not silently update the map.

See [Ships and Expansion](ships-and-expansion.md) for Shipyards, probe and
expedition travel, founding, population movement, and control.

## Knowledge levels

A system progresses through four player-facing knowledge levels:

| Level | Meaning | Targetable? |
| --- | --- | --- |
| `Unknown` | No known indication of the system. | No |
| `Anonymous` | An existence-only indication is known, but not the system's identity. | No |
| `IdentifiedSummary` | The system is identified and has summary map information. | Yes |
| `Complete` | Exact map facts have been observed, including stable bodies and slots. | Yes |

Knowledge is fact-based rather than a replaceable system dossier. A system can
therefore retain exact facts while a newer report contributes only lower-detail
or unrelated facts.

## Initial frontier knowledge

At tick `0`, the origin receives observations made at tick `0`:

- Every system within one maximum probe jump of the origin begins at
  `IdentifiedSummary`.
- Systems reachable in two or three maximum-probe-range geometric legs receive
  `Anonymous` existence indications.
- More distant systems remain `Unknown`.

An initial identified summary reveals:

- body count;
- exact system strength;
- each body's slot count; and
- system-level presence of each material resource, classified as `Poor`,
  `Normal`, or `Rich` from its aggregate initial generated quantity.

It does not reveal exact slot identities or current slot availability, body
eccentricity, exact resource quantities, or the bodies that hold a resource.
Richness thresholds are ordered authored tuning. Exact resource observations
supersede richness labels for presentation without removing unrelated summary
facts.

Initial knowledge describes whatever frontier generation produced. It does not
imply that nearby systems are connected, reachable by expedition ships,
resource-rich, or suitable for settlement.

## Probes

Probes are constructed by Shipyards and stored as system-owned assets until
launch. A launch:

- names a distinct `IdentifiedSummary` or `Complete` target;
- chooses a desired jump limit no greater than the probe's authored maximum;
- requires a nonempty deterministic route whose legs fit that limit; and
- atomically pays the complete route Energy cost from the source system.

The adjustable jump limit lets a player ask a long-range probe to use only legs
that a shorter-range expedition can later traverse. The route is fixed at
launch. A launched probe cannot be cancelled, recalled, or retargeted, and
scouting travel has no random failure.

A probe observes every system where it stops, including intermediate systems,
and performs an existence scan around each stop using its authored reveal
radius. Reveal radius and maximum jump distance are separate tuning values. The
probe is consumed after observing its final target.

## Routes and hidden stops

Routes are derived from fixed system positions for the selected ship's jump
limit; there is no authored route graph. Disconnected and unreachable regions
are valid frontier texture.

Routefinding minimizes total integer leg distance. Equal-cost routes are broken
by the lexicographically ordered sequence of stable system IDs. It may use
unidentified systems as intermediate stops, but a player-facing route redacts
those systems until the traveling ship reaches them. Reaching a hidden stop
identifies it through the ship's complete observation.

Leg duration is the ceiling of leg distance divided by ship speed. Total launch
Energy is based on the sum of route-leg distances and the ship kind's travel
Energy rate. Probe and expedition ships have independently authored jump limits,
speeds, and Energy rates.

## Complete observations

Every probe or expedition stop creates a complete visited-system observation at
the current simulation tick. It records exact map facts:

- system strength;
- stable body identities and order;
- each body's eccentricity;
- stable slot identities and order;
- each body's initial and currently remaining material-resource quantities; and
- other map-owned properties.

The observation also records whether the system is currently inhabited.
Resource depletion and inhabited status are the only runtime facts included in
a detailed scouting observation. Scouting does **not** reveal population count,
stocks, developments, queues, ships, support status, or other runtime state.

Probes additionally report existence-only facts for systems within reveal
radius. Expeditions make the same complete stop observations but do not perform the
reveal-radius scan. At a final expedition stop, the transmission also carries a
typed mission outcome: `Founded` with target/community/development identities,
or `FoundingLost` with the insufficient-slot reason and the ship/population/
payload loss identities.

## Transmission and receipt

Each stop observation creates a pending transmission to the origin knowledge
store. A fact records:

- `tick_observed`;
- its detail level;
- the stable observer or ship ID; and
- `tick_received` when delivered.

Communication delay is the ceiling of direct distance from the observed system
to the origin multiplied by the authored communication rate; it does not use
route distance. Zero-delay reports arrive in the same movement phase;
positive-delay reports arrive on their exact scheduled tick. The report survives consumption or loss of the observing ship. Until the final
outcome transmission is received, origin-facing mission state remains
`AwaitingOutcome`; it does not expose authoritative arrival/loss state through a
global ledger or missing-ship shortcut. A received successful outcome unlocks
direct commands for the founded target. A received loss outcome exposes the
typed loss evidence. Physical target simulation begins at arrival regardless of
report delay.

The editable `starter` profile uses a probe reveal radius of `1_500` coordinate
quanta and a communication delay of `1` tick per `500` quanta. These are
independent authored tuning values rather than engine defaults.

Every transmission has a stable ID. Duplicate receipt is idempotent, and a
transmission is fully validated before all of its fact merges are applied
atomically. The model retains current facts and pending transmissions, not a
permanent report history.

## Fact merge rules

Facts merge independently by stable key: summary fact, exact map field, body,
slot, body-resource quantity, and inhabited status do not overwrite one another
as a bundle.

1. Lower-detail information never replaces higher-detail information.
2. At equal detail, the newer observation tick wins.
3. At equal detail and equal observation tick, the lexicographically lower
   stable observer or ship ID wins, independent of receipt order.
4. Higher-detail information may add exact facts even when observed earlier
   than a summary report.
5. Once known, immutable exact map facts may only be repeated identically. A
   contradiction invalidates the entire received transmission.

These rules prevent a stale dynamic fact from rolling back an independently
observed field and prevent existence or summary reports from erasing exact map
knowledge.

## Design boundaries

Scouting knowledge is deliberately distinct from physical ownership. Direct
control of an inhabited player-founded system becomes available after the
origin receives its successful founding outcome, then exposes the authoritative
runtime state needed to issue commands there, but it neither creates a second
knowledge store nor bypasses delayed map observations.

The design does not currently include random travel failure, probe recall or
retargeting, permanent report history, or knowledge stores owned by remote
systems.

## Related pages

- [Ships and Expansion](ships-and-expansion.md)
- [Architecture](../architecture.md)
- [Stage 4b implementation plan](../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md)
