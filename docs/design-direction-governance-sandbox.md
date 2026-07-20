# Design Direction: Governance Sandbox — Gameplay Foundations

## Purpose

High-level design direction for where the gameplay lives on top of the
economic terrain from [Slice 1][slice-1] and the now-obsolete
[Slice 2 direction][slice-2].
This is a reference document, not a work item: it records the diagnosis,
genre framing, settled decisions, and open questions. Individual plans and
todos should be carved out of the open directions below and cite decisions
here by number (G1, G2, ...).

## Diagnosis

Slices 1–2 built terrain (economic solvency), not gameplay. The game feels
"mathy" because the player's verbs are the simulation's verbs — the player
has no informational or positional advantage that the pricing model doesn't
already capture, so optimal play is arithmetic. Separately, agent iteration
is slow because correctness is dominated by long-run soak modeling.

## Genre framing

- **Survival-inflected 4X** (reference cluster: Caves of Qud, Frostpunk,
  Dwarf Fortress, FTL, Stellaris): scarcity and coping, not a power curve
  toward a win screen.
- **Not anti-expansion, but anti-displacement**: no fully populated map
  where growth means combat over occupied systems. The frontier is
  empty-because-broken. Conflict exists eventually but is not the focus.
- **Scope**: governance mode is the game. The single-ship trader mode is a
  terrain testbed and may spin off as a separate game. Play starts as a
  governor; no trader→governor progression.
- **Mechanics-first**: design the mechanic; fiction develops alongside and
  may trail it. No mechanics that require pretending.

## Fiction premise (loose, generative)

**Post-apocalyptic 4X**: the player inhabits the aftermath of some other
civilization's 4X game. The precursors played the accumulation game —
megastructures, extraction monocultures, stellar-scale energy grids — and
collapsed. Consequences:

- Worldgen can place ruins by running crude precursor accumulation logic:
  "this was their mining hub" → resource-rich, habitat-poor ruin; "this was
  a collector farm" → energy-infrastructure salvage. Ruin variety and the
  difficulty curve fall out of what the precursors optimized for.
- The energy economy is diegetically their ruined grid.
- Thematic loop: the player plays the humane, survival-scale version of the
  game whose maximalist version produced the wreckage.
- The player community is the **origin** community (cf. the Radch in Ann
  Leckie's novels): a fixed home that expansion radiates from.

## Decisions

- **G1. Failure model: the world absorbs failure** (DF-style), not terminal
  runs. The player community can die (go dark) via the same mechanic as NPC
  systems; its ruin persists and could be reclaimed in a later run.
- **G2. Metastability bar relaxed**: the world must be *possible*, not
  provably stable. Local collapse is permitted, expected, and is content
  (future reclamation sites), not a bug.
- **G3. eXpand = reclamation** of broken/abandoned places. Phase (a):
  worldgen-scattered ruins only. Phase (c), later: the live simulation also
  produces ruins.
- **G4. eXterminate deferred** to an environmental conflict engine
  (ecological disaster, disease). Seed for later: hazards should travel
  along the same routes trade does.
- **G5. Player identity = the community**, not an individual. No separate
  governor treasury; the system ledger is the player's ledger. Roles within
  the community are fluid; groups may specialize. Verbs lean toward
  direction-setting over micromanagement, matching the
  identical-policy-component constraint.
- **G6. Core loop = margin allocation.** Untouched systems idle at
  subsistence via the self-correcting economy; the player's job is
  generating and spending *surplus* above life-support burn. Three uses of
  margin: **bank** (reserve against shocks), **expand** (expeditions),
  **develop** (infrastructure). Runway — survivable ticks at zero income —
  is the glanceable pivot stat; specialist capacity gives it siblings.
- **G7. Expeditions are lumpy and physical**: a reclamation attempt is a
  laden convoy using existing travel physics. No abstract resource-transfer
  button. Expeditions **can fail and can lose pops**.
- **G8. Hidden information is core, and hidden ≠ random.** All ruin state
  is committed at worldgen seed time; uncertainty lives only in the
  player's information state. The simulation stays deterministic
  end-to-end.
- **G9. Scouting is layered**: chart → flyby → deep survey → ground truth,
  each layer costing margin. Surveys yield estimates with error bars, never
  paywalled truth. Failure should be player-authored ("went in
  under-scouted"), not dice-authored.
- **G10. Two-channel information model: comms + ships.**
  - **Comms (light-speed): fast, thin, continuous.** Systems broadcast a
    summary layer (stocks, population, alerts, prices) received at a
    distance-based delay (~distance × k ticks). Automatic, cheap.
  - **Ships: slow, thick, discrete.** Survey detail, samples, people,
    goods, and (tunably) founding-level authority move only physically.
  - Payoff: *you always know something is wrong; you rarely know exactly
    what, and you can never respond instantly.* A distress call arrives at
    light speed; the rescue travels at ship speed.
  - Implementation: one observation model with `tick_observed`; two
    writers (periodic delayed comms, event-on-arrival ships). Prototype the
    freshness machinery on trade data first.
  - **Worldgen scale mandate**: the map must be large enough that comms
    lag is a mechanic the player confronts when operating at scale — not a
    cosmetic delay. The delay constant k is a config/worldgen parameter,
    but defaults and map size must be chosen together so the frontier is
    genuinely stale. This is a sizing constraint on worldgen, not a tuning
    afterthought.
  - Stale data must *look* stale in the TUI ("as of tick N"); this UI work
    is part of the feature, not a follow-up.
- **G11. Specialists are a state on pops**, not a new resource line.
  Life-support burn and capacity travel with the pop automatically; a pop
  on expedition leaves vacant capacity at home. Pops **lock into
  specialization types** — the crunchy strategic choice (lock granularity
  open, Q4).
- **G12. Tertiary production is the specialist substrate.** Training (and
  possibly sustaining) specialists consumes tertiary goods, giving the
  tertiary loop its purpose and making intersystem trade structurally
  necessary for capability growth. Chain: raw → processed → tertiary →
  specialists → surveys/expeditions/reclamation. What accumulates over a
  run is *capability*, not wealth — and it can die.
- **G13. Slots come from bodies.** Systems contain planets and moons; body
  count and type describe potential development slots and pre-existing
  (precursor) infrastructure. Data-model shape: system → bodies → slots →
  developments, where a **development** is one concept with states
  (functional / damaged / ruined) and reclamation is a state transition,
  not a separate system. Worldgen describes systems on near-orthogonal
  axes — energy supply (star + collectors) vs. development capacity
  (bodies) vs. resource profile — and strategy texture comes from the axes
  disagreeing (bright star over barren rock = exporter; rich bodies under a
  dim star = capacity that must be fed by energy hauling).
- **G14. Ruin taxonomy, phase (a)**: two types. **Resource ruins** —
  deposits to strip (low stakes, exercises expedition logistics).
  **Site ruins** — reclaimable into outposts (full commitment).
  Distinguishing them is what layered scouting is for. Ruins can also yield
  *people* (found pops/specialists), making reclamation self-reinforcing.
- **G15. Origin seat with delegation-by-distance.** The player's control
  and information are fullest at the origin and attenuate with distance by
  the same physics (G10). NPC management is **delegation, not opposition**:
  daughter systems run the identical policy component AI-managed, with
  standing priorities; light-lag is the honest fiction for why remote
  micromanagement is impossible. Late-game scale = presiding over a
  civilization you can only loosely steer. Drifted daughters are
  proto-factions for the eventual politics layer, unauthored.
- **G16. Anti-strand invariant scoped**: it protects the liveness of
  automated/delegated logistics ships, not player expeditions. Stranding a
  crewed convoy is a permitted, player-authored catastrophe.
- **G17. The world starts dead except for the player.** One living origin
  community; everything else is ruins, resources, and empty geography from
  the precursor collapse. Living neighbors are outputs of play (daughters
  via reclamation), never worldgen guarantees. Independent NPC communities
  are deferred entirely, possibly forever. Ships trading between living
  systems are the player community's own logistics (a delegation/fleet
  mechanic), not an ecology of independent agents.
- **G18. Worldgen viability is constructive, not statistical.** Viability
  is built where it matters and irrelevant everywhere else. Exactly two
  guarantees, both exactly assertable per seed:
  1. **Origin solvency with surplus margin**: the origin system alone
     covers its burn through the worst seasonal phase, with margin above
     subsistence so the bank/expand/develop choice exists from tick one.
  2. **Neighborhood affordance**: within starting expedition/scouting
     range there are enough extractable resources and at least one
     reclaimable site for the expansion loop to turn.
  Everything beyond the neighborhood is don't-care frontier. Distant
  non-viability is not a generation failure — it is ruins, i.e. the
  expand content. Trade gradients, tertiary-chain closure, and comms-lag
  texture are emergent consequences of expansion, never generation-time
  guarantees. No reject/reroll, no viability screening, no statistical
  acceptance criteria over seed batches.
- **G19. [Todo 007][slice-2] is obsolete.** Surviving ideas are re-derived
  from this
  document rather than inherited: the brownout ladder (now the *player's*
  failure mechanic, and more central for it), seasonal variability, and
  population hysteresis. The endogenous NPC trader fleet is struck; its
  spawn/retire-on-opportunity design may later be reincarnated as
  automated logistics for the player's own network (the fleet-management
  layer).
- **G20. Economy/goods content will be revised** to serve the actual
  chain (energy + raw → processed → tertiary → specialists) rather than
  the trader-among-NPCs prototype it was built for. Separate content
  slice.
- **G21. Prototype compatibility is not a design constraint.** The durable
  economic core is Energy as the physical survival pressure plus a resource
  economy that forces a choice between sustaining current communities and
  funding expansion. Existing markets, pricing, independent traders, NPC
  fleet ecology, authored trade routes, and related UI or diagnostics are
  implementation experiments, not guaranteed foundations. Keep or reshape
  them only when they directly serve the governance-and-expansion game;
  otherwise remove them. During prototyping, deleting a conflicting system
  is preferable to preserving it as an accidental product requirement.
- **G22. Durable economic substrate.** Preserve only these design-level
  contracts from the former economy prototype:
  1. Energy is a physical, discrete resource rather than an abstract score.
     Living communities generate, store, consume, transport, and sometimes
     lose it to explicit sinks or curtailment.
  2. Life support creates an unavoidable baseline burn. Energy above that
     burn is margin the player can bank, spend on current capability, or
     commit to expansion (G6).
  3. Extraction and production consume Energy and physical inputs. The
     resource chain exists to create legible pressure between maintaining
     current communities and building the capability to reclaim new ones.
  4. Physical resources do not teleport. When resources move between
     locations, movement has capacity, time, and Energy costs appropriate to
     the mechanic carrying them.
  5. Resource arithmetic is checked, deterministic, and exactly
     reconcilable. Failed mutations validate before changing state.
  This does **not** preserve Energy as universal money, automated market
  makers, bids and asks, trader wallets, commercial delivery contracts,
  reservation policy, NPC fleet ecology, or the authored 20-system balance.
  Those are archived implementation history, not design requirements.

## Testing implications

- Two tiers, both cheap and deterministic (see
  [Testing and Worldgen Direction Shift](plans/2026-07-20-testing-stance-correction.md)
  for the full direction):
  1. **Authored micro-fixtures** (3–6 systems, hand-computable outcomes)
     test mechanisms exactly.
  2. **Generated worlds** test only (a) engine invariants — conservation,
     determinism, anti-strand, no deadlock — on arbitrary seeds, and
     (b) the constructive guarantees of G18 as exact per-seed assertions.
- No statistical acceptance criteria over seed batches. Distribution
  shape, frontier life/death rates, and emergent texture are *descriptive*
  diagnostics for tuning worldgen feel, never pass/fail.
- Rule of thumb: a feature validatable only by soak run is a simulation
  feature; gameplay features must verify in short authored scenario
  fixtures (small world, tens of ticks, deterministic expected outcome).
- Expeditions, surveys, training, and reclamation are all discrete,
  deterministic, fixture-testable events by construction (G7, G8).
- Local collapse anywhere is expected and permitted (G2); only engine
  invariant violations and G18 guarantee failures are bugs.

## Open directions (converging; need specs in future slices)

- **Q1. Building/infrastructure loop**: buildings as recipes consuming
  goods over ticks, producing slots/effects on bodies. Keep develop and
  expand in tension: home slot growth should hit energy/resource-supply
  diminishing returns so capability growth eventually requires territory —
  make that cap legible.
- **Q2. Specialization pipeline**: training recipe costs and duration;
  found-in-ruins specialists (G14) as an alternate source.
- **Q3. Specialist upkeep vs. training-cost-only**: upkeep makes vacancy
  bite and adds a second runway number, but must remain an automatic flow
  (no chore), like life support.
- **Q4. Lock/retrain granularity**: locked forever (max crunch, dead-end
  risk) vs. costly retrain. Dead ends may be acceptable as world-absorbed
  stories (G1).
- **Q5. Expedition roster granularity**: typed counts vs. temporarily
  individuated members (FTL-scale attachment exactly where the risk is);
  candidate shape is a scale-dependent view over the same pop data.
- **Q6. Stranded convoys as rescuable entities**: distress state rather
  than deletion; convoys need survivable failure states in the data model.
  Pairs with G10's distress-at-lightspeed / rescue-at-shipspeed drama.
- **Q7. Governor verb set**: which policy knobs are player-exposed vs.
  AI-defaulted; the tick-to-tick interface beyond runway. Where the
  comms/ships authority line sits (G10) — what can be commanded remotely
  vs. requires presence/envoy.
- **Q8. Reclamation outcomes**: daughter communities with their own ledgers
  (implied by physicality); partial reclamation (salvage vs. full
  revival); whether expedition composition defines the daughter's
  character.
- **Q9. Resource-ruin working model**: expedition-strips-and-returns
  (discrete, simpler — phase (a) default) vs. claimed remote worksites
  (ongoing objects to supply and defend).
- **Q10. Origin vulnerability / run structure**: the origin can go dark
  like any system — de facto loss condition, or succession event (a
  daughter becomes the new center)? Interacts with session structure,
  lineage across communities, and whether any end state exists.

## Open questions (deferred; architecture-aware)

- Time pressure beyond seasons: pop growth as internal clock; what forces
  decisions *now* so banking forever isn't dominant. Slot-to-pop ratio is
  a pacing dial (early labor-scarce → later pop-pressure driving
  expansion) and can move across a run's arc.
- Ruin distribution and difficulty grading in worldgen (the precursor
  accumulation-logic pass is the candidate generator).
- Information decay/contestability for surveys (deferred). Hedge already
  adopted: all observations carry `tick_observed` from day one.
- Phase (c) live ruin generation; environmental conflict engine design
  (G4).
- Group opinion/coalition dynamics (the social model on top of the
  capability model) — explicitly deferred politics.
- Fiction specifics: what energy *is*, the precursor collapse story
  (doubles as worldgen logic). Allowed to trail the mechanics.

## Notes

- [Slice 1 (006)][slice-1] decisions D1–D5 stand. [Todo 007][slice-2] is
  obsolete (G19); its surviving ideas are re-derived from this document.
  Future todos are carved from Q1–Q10 and cite G-numbers.
- The single-ship trader mode may still serve as a cheap harness for
  prototyping G10 information freshness, with the caveat that under G17
  there is no NPC market network for it to trade against — freshness
  prototyping targets the player's own remote systems and scouted sites.

[slice-1]:
  ../todos/006-complete-p1-slice-1-energy-denominated-economy-foundation.md
[slice-2]:
  ../todos/007-complete-p1-slice-2-world-dynamics-population-and-player-progression.md
