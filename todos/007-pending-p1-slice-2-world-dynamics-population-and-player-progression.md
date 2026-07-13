---
status: pending
priority: p1
issue_id: 007
tags: [economy, worldbuilding, population, npc-traders, player, simulation, design]
dependencies: [006]
---
# Slice 2: World Dynamics, Population, and Player Progression

## Purpose

Second design slice, building directly on Slice 1
(`todos/006-*-slice-1-energy-denominated-economy-foundation.md`). Slice 1
delivers a solvent economy on a physical energy layer with static population
and a fixed trader count. This slice makes the world *dynamic*: staged
system decline, seasonal supply variation, a self-scaling NPC fleet,
population with memory, and the investment mechanics that open governor-tier
play.

## Design goal: a metastable world

The world must run indefinitely without the player while remaining visibly
responsive to player action. The mechanism is **timescale layering**: the
world self-corrects through forces that are slower and dumber than the
player.

| Layer | Corrects over | Primary knobs |
| --- | --- | --- |
| Storage buffers (Slice 1) | ticks | per-system storage cap |
| NPC trade fleet | tens–hundreds of ticks | spawn/retire thresholds, spawn rate |
| Population | hundreds–thousands of ticks | growth/decline rates, supply-history window |

The player acts faster than every layer except storage. Tuning rule: world
too stable → shrink storage caps or slow fleet adaptation; world too fragile
→ the reverse. These constants should scale with system count; the
prototype's 20 systems and 9 traders are placeholders, and larger maps are
naturally more redundant.

## Instructions: brownout ladder (staged decline)

A threshold ladder on a system's energy position (stock and/or
ticks-of-burn remaining), with thresholds authored in `economy_config.ron`:

1. **Surplus/Normal** — full behavior.
2. **Throttled** — recipe throughput scales down; industry is sacrificed
   first.
3. **Emergency** — market bids collapse to energy and minimal survival
   goods; all other advertised demand is withdrawn; the energy bid rises
   toward the market's ceiling.
4. **Starvation** — population declines (below), shrinking burn until the
   system re-balances at a smaller size or empties.

Requirements:

- Each stage is visible in the market view and changes prices, so distress
  is advertised through the same signals traders already read — the ladder
  needs no scripting or events to function.
- Stage transitions emit typed events for the log and diagnostics.
- Funded quantity and operating reserve (Slice 1) are recomputed
  consistently per stage: energy earmarked for survival is not purchasing
  power.
- The anti-strand invariant holds at every stage.

## Instructions: seasonal variability

Support an authored oscillation (amplitude, period, phase) on a system's
generation rate. Deterministic and learnable: periodic gluts and famines
turn route knowledge into player skill. Keep prototype content to 2–3
variable systems so patterns are discoverable; default amplitude is zero.

## Instructions: population

1. **State.** One integer per system, now dynamic. Population drives
   life-support burn (sink), recipe throughput / labor (source), and
   tertiary goods demand (sink). This three-way coupling makes growth
   genuinely double-edged.
2. **Hysteresis (required).** Decline under starvation is fast; growth under
   surplus is roughly 5–10× slower and gated on a long moving average of
   energy/goods sufficiency, never on instantaneous state. Systems that
   brown out settle into smaller stable configurations; the population map
   becomes a persistent record of history and of player action.
3. **Model.** Logistic growth toward a cap derived from sustained supply
   history. Migration (population as cargo, player-founded markets) is a
   recorded later hook.
4. **Determinism.** Integer, deterministic arithmetic like all simulation
   math.

## Instructions: endogenous NPC trader fleet

1. Replace the fixed authored count with spawn/retire dynamics:
   - **Spawn** when network-wide unserved profitable opportunity persists
     above a threshold for N consecutive ticks, measured from Slice 1 data
     (reservation shortfall, persistent margin). Spawn location follows a
     deterministic rule (e.g., highest-surplus system).
   - **Retire** when a trader cannot fund a jump even after an anti-strand
     liquidation sale, or after sustained unprofitability.
2. Spawn rate is deliberately *slow* relative to player reaction time. The
   lag between a disruption and fleet adaptation is the player's designed
   window of impact — do not tune it away.
3. Tuning target: NPC capacity alone keeps importers alive but hovering
   around the Throttled band. Player activity should visibly move systems
   across ladder thresholds.
4. `traders.ron` shifts to fleet-dynamics parameters (spawn threshold,
   evaluation window, retire rules, per-trader stats). Keep a fixed-count
   mode for deterministic regression tests.

## Instructions: investment sinks

Conversions from a market's energy stock into parameter increases, executed
per the market's policy component (plumbed in Slice 1):

- **Collectors** → raises `energy_output_per_tick`.
- **Storage** → raises `energy_storage_cap`.
- **Population support** → raises growth rate / cap trajectory.
- **Route subsidy** → the market pays above-market bids to attract the
  endogenous fleet; the governor manipulates the same profit signals the
  trader tier teaches.

Each is "spend energy stock, increment a parameter" with authored costs,
diminishing returns, and rate limits. These give structural exporters a use
for surplus and are the substance of governor play. The prototype may
implement collectors and storage first, but define all four data shapes
together.

## Player progression

- **Tier 1 — Trader (current):** one ship, tank-as-wallet, learning to read
  ladders, prices, and seasons. This tier is the tutorial for the whole
  game's literacy.
- **Tier 2 — Governor:** ownership of one market's policy component and
  treasury. The player sets policy — reserve ratio, margins, import
  priorities, investment allocation — and the market executes autonomously
  every tick. An absent player and an AI-defaulted market are the same code
  path, so governance is never upkeep. Long-run score: the governed
  system's population tier and ladder history.
- **Tier 3 — Multi-system:** several policy components plus genuinely new
  mechanics: transfers between owned markets, coordinated policy, and
  network-scale route subsidies.
- The acquisition mechanism for governance (purchase, grant, founding) is a
  later decision; the policy component from Slice 1 is the prerequisite.
- Self-set goals must work without quest scaffolding (e.g., "hold System 14
  in surplus for 500 ticks and watch its population tier rise") — the
  hysteresis design is what makes such goals meaningful and persistent.

## Diagnostics and validation additions

Extend `--economy-diagnostics` beyond Slice 1:

- Per-system: net energy flow, storage % of cap, ladder stage occupancy over
  time, population trajectory.
- Network: % of importers at each ladder stage (should hold roughly constant
  as map size scales), fleet size vs. opportunity backlog, spawn/retire
  events.
- Player-impact probe: with identical seeds, diff a baseline run against a
  run with one injected exogenous delivery; the difference must be visible
  in ladder stages or population within a bounded horizon. This
  operationalizes "player actions are visible."
- Long-run soak: a 10,000-tick unattended run shows no deadlock, no global
  collapse, ongoing ladder-stage churn, and at least one system settled at a
  changed population level — metastability, not equilibrium.

## Sequencing within this slice

1. Brownout ladder on the Slice 1 physical layer (static population,
   fixed fleet) — this alone delivers the life-support drama.
2. Diagnostics extensions and the player-impact probe.
3. Endogenous fleet behind a mode flag; fixed-count mode retained for tests.
4. Population hysteresis.
5. Investment sinks; governor-tier commands and UI last.

## Known risks / watch items

- **Oscillation coupling:** ladder thresholds + storage + fleet lag can
  resonate into boom/bust cycles. Lumpy is the goal, but diagnostics must
  expose cycle amplitude so it stays tunable.
- **Population ratchet:** fast decline plus slow growth means a badly tuned
  early network permanently shrinks the world. The soak test's
  churn-without-collapse criterion is the guard.
- **Policy component scope:** keep the struct small and boring until the
  governor tier ships; it is a seam, not a feature.
- **Energy decay (demurrage)** remains the reserve tool from Slice 1 if
  concentration persists once all sinks are live.

## Notes

- All new state is `game-core` components/resources; all parameters flow
  through `game-content` validation with source-aware diagnostics.
- Preserve determinism and validate-before-mutate guarantees throughout.
- Display naming follows the `energy` internal-ID note in Slice 1.
- The unrelated untracked `.obsidian/` directory must remain untouched.
