---
title: "Systems and Resources"
type: design-current
status: approved
authority: normative
horizon: current
---
# Systems and Resources

Systems are persistent physical places. Their map facts are distinct from their
mutable simulation state, and neither community population nor player knowledge
owns a second copy of either.

## Ownership model

A system's map definition owns immutable physical facts:

- stable system identity and fixed-point position;
- stellar strength;
- stable body identity and order;
- each body's eccentricity;
- stable generic construction-slot identity and order; and
- each body's initial generated material-resource quantities.

Every generated system also has persistent runtime state keyed to that map
definition. Runtime state owns remaining body-resource quantities, available
stocks, developments, construction and Shipyard queues, committed projects,
completed assets, reservations, accounting, founded/control state, and any
system-owned simulation state. Revision-1 generated neutral systems begin
without stocks, developments, projects, ships, or population, but their generated
remaining resource quantities still live in runtime state rather than in the map
definition. This is a generated-start property, not a universal restriction on
coherent authored Tier 1 scenarios.

The world owns gameplay tuning shared by all systems. A neutral system does not
carry an optional private copy of recipes, capacities, upkeep, seasonal curves,
or action tuning.

Population is world-owned token state rather than a writable system aggregate;
see [Population and Habitats](population-and-habitats.md). Seasonal Collector
output is defined in [Energy and Seasons](energy-and-seasons.md).

## Bodies, slots, and developments

Bodies and slots have stable order. That order is a gameplay tie-breaker for
same-tick resource contention, Habitat Energy allocation, population selection,
and unreserved settlement.

A body resource consumes no construction slot. A development does consume a
slot. Reservations use typed owners, so a construction sequence cannot collide
with an expedition ship that happens to have the same numeric sequence. A valid
known-slot expedition reservation blocks construction and other expeditions
until arrival atomically replaces the reservation with occupancy.

System-level resource totals and occupied-slot views are derived from their
body-owned authorities. They are never independent mutable totals.

## Material resources

Each resource definition explicitly states whether it is naturally
deposit-bearing. Generation never infers this from the resource ID. Energy is
not deposit-bearing: stellar strength and Collector production are its physical
source.

For every material resource on a body:

- the map definition owns one initial generated quantity;
- runtime owns one remaining quantity;
- a body has at most one quantity for a given resource; and
- multiple generated occurrences merge into that body/resource total.

A frontier system may contain no body resources. If a resource is present, it
may occur on one or more distinct bodies, each with its own nonzero generated
quantity. Aggregate system quantities are reporting views only.

An Extractor occupies a slot and targets `(body_id, resource_id)` on its own
body. Cross-body extraction is invalid. Multiple Extractors may draw from the
same total; they do not reserve exclusive rights to it. If the remaining
quantity cannot satisfy every Extractor in a tick, stable body/slot order decides
which Extractors draw first.

## Origin and frontier structure

[World Generation](world-generation.md#constructive-origin-contract) owns the
origin scaffold, reviewed generator ranges, and valid frontier outcomes. This
page owns their runtime and physical-ownership consequences:

- generated system and body facts remain map-definition-owned;
- body resources receive runtime depletion state without becoming stocks;
- generated developments occupy their generated body slots;
- profile-authored starting stocks become origin system stocks;
- the origin begins commandable at population zero; and
- neutral frontier systems begin without control, stocks, population,
  developments, queues, projects, or ships.

The active `starter` quantities are owned by
[`content/profiles/starter.ron`](../../../content/profiles/starter.ron). No
runtime ownership rule creates a neighborhood, solvency, connectivity, or
resource-distribution guarantee.

## Stocks and physical accounting

Available stock is only one location in the physical-resource lifecycle. The
simulation distinguishes:

- system available stocks;
- material committed to a specific Shipyard project;
- per-tick Shipyard Energy expenditure;
- completed-asset payload stocks and deployable Collectors;
- in-transit payload and population;
- launch travel Energy expenditure;
- arrival receipts and overflow;
- ship, payload, and population loss; and
- population creation and removal.

Enqueue transfers materials from available stock into a project commitment.
Cancellation before the project's first progress step returns the entire
unchanged commitment under the normal overflow rules. Completion records hull or
probe expenditure while transferring expedition founding stock and its
Collector into the completed asset. Launch transfers payload into transit.
Arrival transfers it once into the target; a failed settlement transfers it once
into explicit typed loss evidence. No stage silently destroys, duplicates, or
re-owns physical quantities.

All resource arithmetic is checked. Command and tick transaction boundaries are
described in [Simulation Timing](simulation-timing.md).

## Founded systems and control

Systems, not communities, own stocks, infrastructure, queues, projects, assets,
and accounting. Each inhabited player system has one population-only community.
A successful first settlement marks the target player-founded and creates its
community.

The origin is directly commandable at population zero. A founded remote system
begins automatic physical simulation on arrival and becomes directly commandable
after the origin receives its successful founding outcome; it remains
commandable while inhabited. If its population falls to zero, it
retains ownership, physical state, and automatic simulation but rejects player
commands until repopulated. Neutral systems are never commandable. Direct
control exposes authoritative local runtime state but does not create a second
scouting-knowledge store or bypass delayed observations.

## Related design

- [Energy and Seasons](energy-and-seasons.md)
- [Population and Habitats](population-and-habitats.md)
- [Simulation Timing](simulation-timing.md)
