---
title: System Service Fleets Design Plan
type: feature
date: 2026-07-14
status: draft
---
# System Service Fleets Design Plan

## Status and Purpose

This is a **working design plan**, not an implementation-ready specification. It tracks the idea of system-controlled physical ships that pursue system resilience rather than personal profit.

Service fleets are intentionally separated from physical Energy logistics. The Energy request, source-consignment, bulk-capacity, ownership, and settlement model should be designed and tested first in `docs/plans/2026-07-14-feature-physical-energy-logistics-plan.md`.

A service fleet is not necessary to validate commercial Energy contracts. This feature is a later reliability layer that reuses those contracts when market incentives alone are insufficient.

“Service fleet” is the preferred umbrella term. Individual vessel/class names remain open.

## Current Direction

A system may control a small, bounded set of persistent physical vessels. These vessels:

- Have an owner/home system, current location, capacity, speed, and drive Energy.
- Follow system logistics policy rather than personal profit.
- Initially perform strategic Energy-relief missions only.
- Use the same physical source offers, cargo locking, travel burn, and destination settlement as commercial Energy contracts.
- Receive no magical cargo, instant travel, or external currency.
- Provide a guaranteed **attempt** when a feasible mission exists, not guaranteed rescue.

The motivating distinction is:

- **Commercial trader:** “Will this mission produce enough personal Energy profit?”
- **Service fleet:** “Will this physically feasible mission improve the owning system’s resilience enough to justify its cost?”

A trip can therefore be commercially unattractive but system-positive:

```text
Energy loaded at source: 10,000
Mission travel burn:         200
Commercial profit:             0
Net delivered to system:    9,800
```

A private trader may reject that route. A system approaching brownout may rationally dispatch its own vessel.

## Design Principles

1. **Service vessels obey the physical economy.** They need fuel, cargo, routes, time, storage headroom, and source surplus.
2. **Public capacity is bounded.** Service fleets cannot be free, unlimited, or universally capable.
3. **Service fleets supply agency, not immunity.** A system can still fail for visible physical reasons.
4. **Commercial traders remain the default economic layer.** Public dispatch should not replace ordinary market activity.
5. **Service behavior is policy-driven.** The player configures doctrine and thresholds rather than issuing repetitive per-tick orders.
6. **Persistent location matters.** A service vessel can only act from where it physically is and may need an empty outbound leg before loading.
7. **No commercial self-payment.** An owner does not pay profit to its own vessel; mission Energy allocation covers physical operating needs.
8. **The first scope is Energy relief.** Ordinary-goods procurement, warfare, exploration, and construction fleets are future systems.
9. **AI and player-governed systems use the same core rules.** Authority changes controls, not physical simulation.

## Problem Statement

### Critical systems currently depend on passive response

A market can advertise demand or increase incentives, but that does not ensure:

- An eligible trader exists.
- A trader is nearby.
- A trader has sufficient bulk capacity.
- A route is individually profitable.
- A shipment can arrive before the system loses critical runway.

For survival Energy, “raise the signal and hope someone responds” is too passive. A system should be able to invest in bounded logistical agency.

### Subsidies and committed capacity solve different problems

A subsidy can make a route more attractive. It cannot create a nearby vessel, free cargo capacity, or guaranteed response.

The current route-subsidy concept should therefore not be asked to solve both:

1. Increase private contract attractiveness.
2. Ensure strategically necessary capacity attempts a mission.

Service fleets address the second problem.

### Free public ships would overwhelm the market

If every system receives unlimited, costless, general-purpose vessels, public logistics can erase scarcity and crowd out commercial traders. Service fleets need capital, operating, timing, specialization, and policy constraints strong enough to preserve private trade and system vulnerability.

## Proposed Model

### 1. Persistent system ownership

Each service vessel has at least:

- Stable vessel ID.
- Owning/home system.
- Current system or in-transit route.
- Drive Energy and capacity.
- Bulk Energy capacity.
- Optional general cargo capacity.
- Speed and travel burn.
- Current mission and contract/cargo claims.
- Service state such as reserve, preparing, outbound, loading, returning, delivering, or recovering.

Ownership means the system controls mission policy and bears the capital/operating cost. It does not allow teleportation, remote refueling, or withdrawal of another system’s Energy without an accepted source consignment.

### 2. Service mission economics

A service mission reuses the physical Energy logistics contract:

1. A destination/owner projects a dangerous Energy shortfall.
2. A source offers exportable surplus.
3. The service vessel reserves mission drive Energy and source cargo.
4. It travels to the source if necessary.
5. It loads contract-locked Energy.
6. It travels to the target system.
7. Settlement replenishes an allowed operating reserve and deposits the remainder.

Unlike a commercial carrier, a service vessel normally receives no profit allocation. Its relevant condition is physical usefulness:

```text
payload > required mission allocation
net destination delivery > 0
```

Provisional mission allocation may include:

- Realized or expected travel-burn replenishment.
- A protected minimum drive reserve after arrival.
- No carrier profit.

Whether mission fuel is funded entirely before departure or partly replenished from the incoming payload remains open. In either case, all Energy remains physical and reconciled.

### 3. Dispatch policy

The system should not wait until current stock reaches zero. Dispatch should compare projected runway with plausible private and service arrival times.

Provisional escalation ladder:

| Condition | Preferred behavior |
| --- | --- |
| Normal | Publish commercial requests; service vessels remain in reserve. |
| Throttled | Improve private terms or charter capacity; prepare a service mission. |
| Emergency | Dispatch eligible service capacity immediately when a physically positive mission exists. |
| Starvation | Continue the best feasible relief mission; do not fabricate fuel, cargo, or instant arrival. |

The final rule should consider:

- Current and projected life-support runway.
- Current brownout stage and trend.
- Known inbound commercial deliveries and arrival ticks.
- Available source surplus.
- Vessel location and total mission travel time.
- Departure fuel and protected reserve.
- Expected net delivery and recovery target.

A known inbound commercial shipment should suppress unnecessary public dispatch when it is sufficient and timely. A critical system with no plausible inbound delivery should not wait through a commercial grace period.

### 4. Commercial-first behavior

A likely default doctrine is **commercial first, service fallback**:

1. Publish commercial Energy requests during healthy operation.
2. Allow private traders to accept profitable work.
3. Prepare a service mission when projected runway crosses a warning threshold.
4. Dispatch if private deliveries cannot restore the target before the critical deadline.
5. Stand down or cancel safely if sufficient private relief becomes committed before public departure.

Other possible doctrines:

- Commercial only.
- Commercial first.
- Resilience first.
- Service reserve disabled.

The initial UI should expose only a small understandable policy set rather than every internal threshold.

### 5. Constraints that prevent domination

Candidate constraints:

- A system starts with zero or a small authored number of service vessels.
- Additional vessels require development, construction, or later shipyard capacity.
- Vessels require advanced goods and Energy to acquire.
- Maintenance, crew, or replacement demand may apply later.
- A vessel has one physical location and can serve only one mission at a time.
- Importer-owned vessels may pay an empty outbound travel leg before loading.
- Departure requires protected mission fuel before the home system is fully exhausted.
- Source cargo requires permission and true exportable surplus.
- Service ships are initially specialized for Energy and do not enter ordinary-goods arbitrage.
- Public missions stop after an authored recovery/runway target.
- Fleet size and mission concurrency are bounded per system.
- Mission claims compete deterministically with accepted commercial contracts.

These constraints make logistical reliability an investment choice rather than a free replacement for market trade.

### 6. Relationship to private traders

Private traders retain important advantages:

- The system does not fund their construction or ownership cost.
- They handle ordinary goods and advanced materials.
- They cover long-tail demand beyond public capacity.
- Specialized commercial ships may be faster or more efficient.
- They can accept profitable Energy work before public fallback becomes necessary.
- They are not limited by one system’s service doctrine.

Service fleets should provide a resilience floor. They should not optimize the whole economy or eliminate profitable opportunities.

### 7. Acquisition and progression

The first implementation may author existing service vessels directly. It should not require a complete ship-construction feature.

Longer-term acquisition could connect to development projects:

- Industrial Machinery for construction/maintenance.
- Reactor Assemblies for propulsion and bulk Energy handling.
- Habitat Modules for crew/support capacity.
- Large Energy capital cost.
- A real shipyard or service-fleet project once ship construction has a physical output.

This link should be coordinated with `docs/plans/2026-07-14-feature-advanced-goods-development-projects-plan.md`, but service-fleet implementation should not force that plan to absorb shipbuilding prematurely.

## Player and Governance Experience

Potential governance controls:

- Logistics doctrine: commercial-only, commercial-first, or resilience-first.
- Preparation runway threshold.
- Dispatch runway threshold.
- Recovery target.
- Protected service-fleet drive reserve.
- Maximum concurrent missions.
- Service vessel standby assignment, if several exist.

Potential system/service views:

- Vessel owner and home system.
- Current location and mission phase.
- Drive reserve, bulk capacity, and locked payload.
- Source and destination.
- Departure/arrival estimate.
- Expected net delivery.
- Why dispatch occurred.
- What currently blocks the mission.
- Whether an inbound commercial shipment changed the decision.

The player should be able to answer:

- Why did this system dispatch its service fleet?
- Why did it not dispatch?
- What did the mission cost physically?
- How much Energy will the target actually gain?
- What commercial opportunity or public mission is being displaced?

The first version should favor policy configuration over direct per-vessel micromanagement. A future explicit emergency mission command may be useful, but it is not assumed.

## Naming

Accepted umbrella term:

- **Service fleet**

Provisional supporting terms:

- **Service vessel** — generic member.
- **Service auxiliary** — possible broad class name.
- **Energy hauler** — functional specialization.
- **Home system** — policy owner.
- **Relief mission** — Energy-delivery mission triggered by resilience policy.

Avoid using “grid tender” as the primary player-facing name.

## Scope

### In scope

- Persistent system ownership of bounded vessels.
- Energy-only relief missions.
- Projected-runway and inbound-delivery-aware dispatch.
- Commercial-first fallback behavior.
- Physical location, fuel, capacity, travel, loading, and settlement.
- AI and player-governed policy using the same core logic.
- Immutable service-fleet views and diagnostics.

### Out of scope

- Defining the underlying commercial Energy contract system; that belongs to the prerequisite plan.
- Ordinary-goods arbitrage by service fleets.
- Combat, escorts, piracy, interception, or defense fleets.
- Exploration or colony ships.
- Full shipyard/construction gameplay.
- Crew simulation.
- Maintenance/depreciation details unless required to prevent free fleets.
- Factions, diplomacy, alliances, or cross-system political ownership.
- Multi-system player fleet micromanagement.
- Instant or abstract emergency transfers.

## Dependency on Physical Energy Logistics

This plan assumes the Energy logistics feature provides stable contracts for:

- Destination requests.
- Source exportable-surplus offers.
- Source-backed consignment.
- Contract-locked bulk Energy.
- Dedicated Energy capacity.
- Route-burn and net-delivery arithmetic.
- Source and destination claims.
- Partial settlement and failure recovery.
- Exact physical ledgers.

Service fleets should call those contracts rather than introducing a second loading or settlement path.

Commercial Energy logistics should pass its own tests and long-run diagnostics with service fleets disabled before this feature is enabled.

## SpecFlow Sketch

### Normal commercial-first operation

1. A system publishes an Energy request.
2. One or more private traders consider it under the commercial contract rules.
3. A timely sufficient delivery becomes committed.
4. The service fleet observes the inbound commitment and remains in reserve.
5. The commercial delivery settles through the shared Energy logistics path.

### Service fallback

1. Projected runway crosses the preparation threshold.
2. The system evaluates known inbound deliveries.
3. No private delivery can restore the recovery target before the deadline.
4. Policy chooses an eligible service vessel and feasible source in stable order.
5. Mission fuel, source cargo, and destination headroom claims are prepared atomically.
6. The vessel travels empty to the source if necessary.
7. It loads locked contract Energy and travels to the target.
8. Settlement replenishes permitted mission reserve and deposits the net payload.
9. The vessel enters reserve/recovery at its physical arrival location.

### Important variations

- **No source surplus:** the fleet cannot dispatch and reports the blocker.
- **No departure fuel:** the owner waited too long or underfunded its reserve; no rescue is fabricated.
- **Vessel already busy:** the system waits, charters privately, or remains exposed.
- **Commercial shipment commits during preparation:** safely stand down before loading when sufficient.
- **Commercial shipment commits after departure:** continue, reroute, or cancel according to an explicit mission rule.
- **Source revokes before loading:** release mission claims and search again.
- **Target storage fills:** reuse shared Energy-contract partial settlement/recovery.
- **Several systems claim one source:** resolve urgency and stable IDs explicitly.
- **Policy changes in transit:** existing physical commitments normally continue.
- **Vessel arrives away from home:** it remains there; ownership does not teleport it home.

## Open Design Questions

### Ownership and starting distribution

- Does every system begin with a service vessel, only selected hubs, or none?
- Are vessels owned by systems, factions, or broader logistics authorities?
- Can one system’s service fleet assist another before diplomacy exists?
- Does a vessel remain controlled by its home system when stationed elsewhere?

### Dispatch policy

- What projected-runway threshold begins preparation?
- What deadline triggers immediate dispatch?
- How are inbound private deliveries discounted for cancellation or lateness risk?
- When should public dispatch bypass the commercial grace period?
- What recovery target ends the mission cycle?
- Can players issue one-off emergency orders, or only doctrine?

### Physical operating cost

- Must all expected travel burn be available before mission acceptance?
- May the payload replenish the vessel’s protected reserve at delivery?
- Does the owner, consigning source, or payload fund an empty outbound leg?
- What minimum net delivery makes a public mission rational?
- Does a stranded service vessel have stronger refueling rights than a private trader?

### Market coexistence

- Do accepted commercial contracts always outrank service claims made later?
- Can emergency service claims preempt unaccepted source offers?
- How long should private traders have to respond under each condition?
- What evidence would show that service fleets are crowding out private traders?
- Can service vessels ever take commercial work while healthy, or must they remain idle reserve?

### Capital and progression

- Are first service vessels authored world infrastructure or player-built assets?
- Which advanced goods construct or maintain them?
- Is capacity increased by adding vessels, upgrading vessels, or both?
- Does a service fleet require a dedicated development project?
- How is replacement handled if ships can later be destroyed?

## Provisional Implementation Sequence

This sequence begins only after commercial physical Energy logistics is independently validated.

### Phase 0: Policy simulation

- [ ] Run deterministic paper/test scenarios for commercial-only, commercial-first, and resilience-first doctrines.
- [ ] Model vessel locations, empty outbound legs, runway deadlines, inbound deliveries, and source contention.
- [ ] Choose initial fleet distribution and Energy-only mission restrictions.
- [ ] Define preparation, dispatch, stand-down, and recovery rules.
- [ ] Demonstrate that service dispatch improves resilience without guaranteeing survival.

### Phase 1: Service ownership and state

- [ ] Add system ownership/home identity to authored service vessels.
- [ ] Add reserve/preparing/mission/recovery states.
- [ ] Add service doctrine and bounded concurrency policy.
- [ ] Expose immutable owner, location, capacity, reserve, and mission snapshots.
- [ ] Validate fleet definitions and ownership references in content.

### Phase 2: Dispatch and shared-contract integration

- [ ] Evaluate projected runway and known inbound commercial deliveries.
- [ ] Select service missions in stable system/vessel/source order.
- [ ] Reuse source consignment, locked cargo, travel burn, claims, and settlement from Energy logistics.
- [ ] Implement safe stand-down before loading.
- [ ] Preserve physical mission state after policy changes or changed market conditions.

### Phase 3: Governance and observability

- [ ] Add a small logistics-doctrine control surface.
- [ ] Explain dispatch, stand-down, and blocker reasons.
- [ ] Show mission cost, payload, route, arrival, and expected net delivery.
- [ ] Distinguish commercial and service Energy movement in diagnostics.
- [ ] Keep AI markets read-only while using the same policies and views.

### Phase 4: Balance and progression decision

- [ ] Compare commercial-only and service-enabled deterministic runs.
- [ ] Measure crowd-out, crisis prevention, futile dispatch, and fleet utilization.
- [ ] Decide whether authored fleets are sufficient for the first release.
- [ ] If acquisition is needed, coordinate a separate ship construction/development design.
- [ ] Update economy docs, encyclopedia, README, and changelog.

## Acceptance Direction

The final implementation criteria should include:

- [ ] Commercial Energy logistics passes independently with service fleets disabled.
- [ ] A service vessel uses the same physical loading, ownership, travel, and settlement contracts as a commercial carrier.
- [ ] A critical system with an eligible vessel, departure fuel, reachable source, and positive net mission attempts relief without waiting indefinitely for private profit.
- [ ] No mission departs without physical drive Energy or loads nonexistent source surplus.
- [ ] Service missions never create Energy, teleport cargo, or pay the owner fictitious profit.
- [ ] A timely sufficient commercial delivery suppresses unnecessary public dispatch.
- [ ] An occupied or stranded service vessel cannot serve another mission simultaneously.
- [ ] Service fleets initially abstain from ordinary-goods arbitrage.
- [ ] AI and player-governed systems execute through the same core dispatch logic.
- [ ] Same-tick contention is invariant to ECS insertion order.
- [ ] UI views explain why dispatch occurred or why it was blocked.
- [ ] Long-run commercial ordinary-goods trade remains materially active.

## Success Metrics to Define During Tuning

- Service-fleet dispatches per system/stage.
- Preparation-to-departure and departure-to-delivery time.
- Net Energy delivered per mission and per travel Energy burned.
- Percentage of missions suppressed by timely commercial arrivals.
- Commercial versus service share of Energy movement by stage.
- Service-vessel occupied and idle-reserve time.
- Preventable starvation periods with and without service fleets.
- Failed or futile mission count.
- Source contention involving commercial and service claims.
- Final-window ordinary-goods commercial activity.

The target is not maximum survival. It is visible strategic resilience with meaningful cost and residual failure risk.

## Testing Strategy

### Pure/unit tests

- Projected runway versus earliest commercial/service arrival.
- Preparation, dispatch, and recovery threshold boundaries.
- Commercial-inbound sufficiency checks.
- Stable vessel/source selection.
- Positive-net mission viability.
- Protected mission-fuel calculations.
- Policy replacement validation.

### Integration tests

- Sufficient commercial arrival prevents service dispatch.
- Missing commercial response triggers a feasible service mission.
- Service vessel performs empty outbound leg, loads, returns, and deposits net Energy.
- Insufficient drive reserve blocks departure without partial mutation.
- Missing source surplus blocks loading and releases claims correctly.
- Vessel already on mission cannot double-dispatch.
- Policy changes during preparation and transit follow explicit rules.
- AI and governed systems produce equivalent decisions from equivalent state.

### Long-run diagnostics

- Compare identical-seed commercial-only and service-enabled runs.
- Preserve exact Energy reconciliation.
- Report service and commercial Energy movement separately.
- Detect public-fleet monopolization, repeated futile dispatch, permanent idle fleets, and locked-cargo deadlocks.
- Require late-window ordinary production and commercial trade.
- Measure crisis reduction without requiring crisis elimination.

### Manual validation

- Observe a healthy system leave its service fleet in reserve.
- Observe a commercial delivery suppress public dispatch.
- Observe a critical system dispatch before losing departure capability.
- Follow a vessel’s empty leg, loading, return, reserve replenishment, and net deposit.
- Inspect an impossible mission and understand its physical blocker.
- Confirm private traders still have profitable work after service fleets activate.

## Risks

| Risk | Consequence | Current mitigation direction |
| --- | --- | --- |
| Every system receives free capacity | Public ships overwhelm commercial trade. | Start with zero/few authored vessels and require later investment for expansion. |
| Dispatch is too reliable | Brownouts and logistics decisions become irrelevant. | Guarantee attempts only; preserve source, fuel, distance, time, and capacity failures. |
| Dispatch occurs too late | Home system cannot fund its own rescue. | Use projected runway and protected mission reserve. |
| Dispatch occurs too early | Public ships crowd out viable private contracts. | Commercial-first doctrine and inbound-delivery awareness. |
| Service ships trade ordinary goods | They become free market competitors. | Restrict initial mission permissions to Energy relief. |
| Separate service settlement path emerges | Accounting and edge cases diverge. | Reuse physical Energy contract primitives directly. |
| Public mission claims starve source systems | Relief exports create a second crisis. | Compute true exportable surplus after source protections and allow pre-load revocation rules. |
| Fleet state adds excessive complexity | Feature expands into full fleet gameplay. | Keep persistent movement but defer combat, crews, maintenance, and construction. |
| Authored vessels have no progression cost | Reliability feels arbitrary. | Treat authored fleets as prototype scaffolding and make acquisition an explicit later decision. |

## Files and Systems Likely Affected

These are candidate surfaces after Energy logistics exists.

- `docs/energy-economy.md` — service dispatch and physical mission accounting.
- `docs/plans/2026-07-14-feature-physical-energy-logistics-plan.md` — prerequisite commercial contract model.
- `docs/plans/2026-07-14-feature-advanced-goods-development-projects-plan.md` — possible later acquisition/infrastructure relationship.
- `content/traders.ron` — service vessel configurations and ownership.
- `content/economy_config.ron` — doctrine and dispatch defaults.
- `content/economy.ron` — per-system fleet and policy overrides.
- `content/encyclopedia.ron` — player explanation of service fleets.
- `crates/game-core/src/lib.rs` — ownership, mission state, dispatch, shared-contract integration, snapshots, and diagnostics.
- `crates/game-content/src/lib.rs` — fleet source schemas and validation.
- `crates/game-app/src/lib.rs` — immutable service-fleet/governance views.
- `crates/game-tui/src/lib.rs` — Governance, system, fleet, and help presentation.
- `crates/game-cli/src/main.rs` — comparative service/commercial diagnostics.
- `CHANGELOG.md` and `README.md` — current player-facing behavior after implementation.

## References & Research

### Internal references

- `docs/architecture.md` — headless simulation, immutable views, content pipeline, and dependency direction.
- `docs/energy-economy.md:75-81,93-104,125-141,151-187` — current reservations, brownout demand, dynamic fleets, investments, phase order, and diagnostics.
- `docs/plans/2026-07-14-feature-physical-energy-logistics-plan.md` — prerequisite physical Energy request, offer, consignment, contract, capacity, and settlement model.
- `docs/plans/2026-07-14-feature-advanced-goods-development-projects-plan.md` — durable advanced-goods uses and possible future service-fleet capacity project.
- `content/traders.ron:1-26` — current player/NPC archetype values and dynamic-fleet settings.
- `content/economy_config.ron:15-35` — current repositioning, brownout, and survival policy.
- `crates/game-core/src/lib.rs:1706-1733,2254-2265` — current trader lifecycle/state and snapshots.
- `crates/game-core/src/lib.rs:4130-4165` — deterministic idle-NPC tank balancing.
- `crates/game-core/src/lib.rs:5983-6060,6230-6338` — trader movement/lifecycle and dynamic spawning seams.
- `crates/game-app/src/lib.rs:314-336,425-442,1220-1285` — current governance and immutable market/trader view seams.
- `crates/game-tui/src/lib.rs:1934-2075,2167-2273` — current governance and investment presentation.

### External references

None. This is project-specific economic and fleet design without external API assumptions.

### Institutional knowledge

No repository solution document currently defines system-controlled service fleets or public fallback logistics.
