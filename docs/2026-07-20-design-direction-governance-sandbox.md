# Design Direction: Governance Sandbox — Gameplay Foundations

## Purpose

High-level design direction for where gameplay lives after the authored market
prototype. G22 restates its durable physical-resource contracts; Git history
preserves the superseded slice documents.
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
- **Scope**: governance mode is the game. Play starts as a governor; there is
  no trader→governor progression and no retained single-ship trader mode or
  compatibility harness in this repository.
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
- **G3. eXpand begins with founding and later adds reclamation.** Stage 4b
  implements bounded one-population founding in generated empty systems.
  Reclamation of broken/abandoned places and live ruin production remain later
  expansion layers.
- **G4. eXterminate deferred** to an environmental conflict engine
  (ecological disaster, disease). Seed for later: hazards should travel
  along the same routes trade does.
- **G5. Player identity = the community**, not an individual. No separate
  governor treasury; the system ledger is the player's ledger. Roles within
  the community are fluid; groups may specialize. Verbs lean toward
  direction-setting over micromanagement, matching the
  identical-policy-component constraint.
- **G6. Core loop = margin allocation.** The player's job is generating and
  allocating physical margin around life-support and infrastructure pressure.
  Three uses of margin remain **bank** (reserve against shocks), **expand**
  (outward actions/expeditions), and **develop** (infrastructure), but they need
  not all be available at tick zero. Stage 4 implements the bank/develop origin
  engine; Stage 4b owns the first bounded expand action. Runway may become a
  glanceable pivot stat when its player-facing contract is designed; it is not
  a Stage 4 solvency oracle.
- **G7. Expeditions are lumpy and physical**: a reclamation attempt is a
  laden convoy using existing travel physics. No abstract resource-transfer
  button. Expeditions **can fail and can lose pops**.
- **G8. Hidden information is core, and hidden ≠ random.** All ruin state
  is committed at worldgen seed time; uncertainty lives only in the
  player's information state. The simulation stays deterministic
  end-to-end.
- **G9. Scouting information is layered.** Stage 4b uses anonymous existence,
  identified summary, and complete probe/ship observation. Probes can be
  constrained to expedition jump paths; an unprobed summary target may be
  founded without slot reservation and can fail deterministically if no landing
  capacity remains. Additional flyby/deep-survey/error-bar layers may be added
  later; truth is never permanently paywalled and failure is player-authored,
  not dice-authored.
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
- **G17. The world starts dead except for the player.** One persistent origin
  seat/community record starts at population zero and may bootstrap before
  population arrives; everything else is ruins, resources, and empty geography
  from the precursor collapse. Living neighbors are outputs of Habitat-backed
  founding and later reclamation, never worldgen guarantees. Independent NPC communities
  are deferred entirely, possibly forever. Ships trading between living
  systems are the player community's own logistics (a delegation/fleet
  mechanic), not an ecology of independent agents.
- **G18. Worldgen construction is structural and tests are not gameplay
  judges.** Stage 4b constructs only the approved origin scaffold before
  procedural frontier texture. It does not guarantee a neighborhood witness,
  connectivity, reachability, target system count, affordability, resource-
  quantity floor, survival, favorable distribution, or reclaimable site.
  Generated-world tests verify deterministic mechanics, identity, references,
  ranges, arithmetic, and the origin scaffold; they do not play worlds, reject
  seeds for qualitative outcomes, or apply statistical desirability thresholds.
  Difficult, disconnected, sparse, or non-viable frontier outcomes are texture
  unless a named engine invariant is violated.
- **G19. The former Slice 2 direction is obsolete.** Surviving ideas are re-derived
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
  2. **Generated worlds** begin in Stage 4b and test only named applicable
     engine invariants, deterministic generator identity/mechanics, and the
     approved origin scaffold. Target count and frontier quality are not exact
     per-seed assertions.
- No statistical acceptance criteria over seed batches. Distribution
  shape, frontier life/death rates, and emergent texture are *descriptive*
  diagnostics for tuning worldgen feel, never pass/fail.
- Rule of thumb: a feature validatable only by soak run is a simulation
  feature; gameplay features must verify in short authored scenario
  fixtures (small world, tens of ticks, deterministic expected outcome).
- Expeditions, surveys, training, and reclamation are all discrete,
  deterministic, fixture-testable events by construction (G7, G8).
- Local collapse anywhere is expected and permitted (G2); only applicable
  engine-invariant violations, generator-mechanic defects, and failure to build
  the approved origin scaffold are bugs.

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

- G22 is the current authority for the durable physical-resource contracts
  extracted from the completed prototype. The former Slice 2 direction is
  obsolete (G19); its surviving ideas are re-derived here. Future todos are
  carved from Q1–Q10 and cite G-numbers.
- Prototype G10 information freshness directly with a small deterministic
  origin/remote-community or scouted-site fixture. Do not retain trader startup,
  market content, or trader UI as a speculative harness.
