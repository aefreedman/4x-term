---
title: Survey Courier Ideas
type: design-idea
status: draft
authority: non-authoritative
horizon: exploratory
tags:
  - scouting
  - information
  - ships
  - core-loop
---
# Survey Courier Ideas

The committed information direction distinguishes thin light-speed communications from richer information physically carried by faster-than-light ships. Current probes and expeditions instead transmit complete visited-system observations to the origin at the authored communication rate. The mechanism below is a possibility, not an approved replacement for current scouting.

## Returnable survey courier

A Shipyard could construct a survey courier that travels to an identified destination, records a complete observation, and returns physically to the origin with that observation. Its strategic purpose would be to trade additional construction cost, travel Energy, and round-trip time for potentially faster delivery than the destination's light-speed transmission delay.

A narrow first design should support one destination and one fixed outbound-and-return mission. The ship would carry information only: no population, freight, founding payload, salvage, or remote authority. It should reuse deterministic routefinding, ordinary ship movement, stable observations, and fact-based knowledge merging wherever those current contracts remain applicable.

The courier must not reveal authoritative destination truth before it returns. Player-facing mission state may expose committed routes and expected timing derived from known facts, but it must not leak survival, arrival, observation, or return state through a global ledger. World truth and mission resolution remain deterministic.

## Questions before promotion

Before this idea can become current design, define:

- whether the courier follows one round-trip route fixed at launch or independently derives its return route;
- whether it observes intermediate stops, the destination only, or both;
- whether carried observations merge only on physical return or can also generate a slower communication transmission;
- what happens if the origin is no longer a valid delivery destination;
- whether the courier is consumed, retained as a completed asset, or requires explicit recovery after return;
- how construction cost, jump range, speed, and travel Energy differ from probes without making either ship strictly dominant;
- how observation timestamps, observer identity, and receipt timestamps integrate with current fact merging; and
- whether return arrival and knowledge receipt occur in the same movement phase.

## Promotion evidence

A proposal should include short deterministic scenarios covering:

1. a courier returning an observation before the equivalent communication transmission would arrive;
2. a nearby destination where an ordinary probe remains the cheaper or faster choice;
3. hidden intermediate stops without premature knowledge leakage;
4. equal-tick courier and communication receipt using deterministic fact merging;
5. route or launch validation failing without spending Energy or committing the asset; and
6. tick failure leaving the courier, carried observations, knowledge, accounting, and clock unchanged.

Any approved version must update [Scouting and Knowledge](../current/scouting-and-knowledge.md), [Ships and Expansion](../current/ships-and-expansion.md), and [Simulation Timing](../current/simulation-timing.md). It should realize the two-channel outcome in [Information and Distance](../direction/information-and-distance.md) without silently introducing general freight or remote-control mechanics.
