# Initial Prototype Specification

## Purpose

The prototype validates the technical stack and architectural boundaries with a small, headless economic simulation rendered through a terminal UI. It is not intended to establish final game mechanics, balance, naming, or visual design.

It must demonstrate:

- A data-driven `bevy_ecs` world
- A three-dimensional map containing 20 star-system stand-ins
- Distances derived from system positions
- A minimal multi-stage trading economy
- An async application boundary around a synchronously stepped ECS simulation
- A Ratatui frontend that only submits commands and renders immutable views
- Headless tests of the same simulation used by the TUI

## Terminology

To resolve the initial layer terminology for this prototype:

1. **Raw resources** are base goods available at source systems.
2. **Primary consumers** consume raw resources and produce processed goods.
3. **Secondary consumers** consume at least one processed raw resource plus at least one additional raw or secondary-stage good and produce another good.
4. **Tertiary consumers**, also called **end consumers**, consume goods and remove them from the simulation without producing a trade good.

Raw resources are inputs rather than a consumer layer. Primary, secondary, and tertiary consumers are the three consumer layers.

## Scope

### Included

- Exactly 20 systems loaded from content definitions
- Three-dimensional system positions
- Calculated pairwise distances
- A connected travel/trade graph
- Generic currency represented by `¤`, the Unicode generic currency sign
- Data-defined goods and processing recipes
- Inventories and currency balances
- Primary, secondary, and tertiary economic processing
- A deterministic simulation tick
- Basic price calculation from local supply and demand
- Simple automated trade movement sufficient to exercise the economy
- A player-controlled trader using the same economic components and transaction rules as automated traders
- Multi-hop route search across the connected system graph
- TUI map list, system detail, market view, player status, trade controls, simulation controls, and event log
- Pause, single-step, and continuous-run controls
- Headless simulation and content validation tests

### Excluded

- Final galaxy generation
- Final economic balance
- Player progression or character systems
- Combat, diplomacy, factions, or quests
- Save/load support
- Mod discovery and dependency management
- Content hot reload
- Multiplayer or a separate server process
- Graphical or ASCII-art map rendering
- Orbital mechanics or physically accurate travel

## Currency

The prototype uses a generic currency with the display symbol:

```text
¤
```

Internally, currency is stored as an integer number of minor units:

```rust
pub struct Money(i64);
```

Floating-point values must not be used for balances or transactions. Display formatting belongs to the application or TUI layer, not the ECS component.

## System map

### Content definition

The repository contains a RON definition for exactly 20 systems. Each definition has:

- Stable namespace-qualified ID
- Temporary display name
- Three-dimensional position
- Economic site configuration

Conceptual definition:

```ron
(
    id: "core:system_01",
    name: "System 01",
    position: (x: 0.0, y: 4.0, z: -2.0),
    economy: (...),
)
```

System positions use typed coordinates expressed in prototype distance units:

```rust
pub struct Position3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

Positions must be finite. Duplicate positions are rejected during content validation.

### Distances

Distance is derived rather than authored:

```text
distance(a, b) = sqrt(
    (a.x - b.x)² +
    (a.y - b.y)² +
    (a.z - b.z)²
)
```

Distances must differ across the map; the content-validation suite verifies that the map does not collapse to one uniform distance.

Calculated distances are stored in a map graph resource rather than duplicated on system components.

### Connectivity

The prototype constructs an undirected graph after loading system positions:

1. Connect each system to its three nearest neighbors.
2. Normalize duplicate edges.
3. Verify the resulting graph is connected.
4. Fail startup with a content diagnostic if it is not connected.

An edge represents an available direct route. Its cost is its Euclidean distance. This provides varied route lengths without requiring a final galaxy-connectivity design.

Traders are not limited to adjacent destinations. The graph exposes deterministic shortest-path queries using total route distance as edge cost. A trader may evaluate every reachable market, select a destination, and store the resulting sequence of route legs as its travel plan. Movement still occurs one direct edge at a time.

## Economy

### Prototype theme

The initial content uses a **remote industrial frontier** theme. The 20 systems represent isolated settlements reconnecting through trade. Their economy centers on extraction, industrial processing, infrastructure fabrication, and settlement consumption.

The tone is practical retro-futurism rather than abstract labels or highly specific fictional lore. This gives the prototype readable names while allowing the setting to be replaced later without changing economic code.

### Goods

Every good has a stable content ID, display name, category, and base price in minor currency units. The initial set is:

| Layer | ID | Display name | Base price |
| --- | --- | --- | ---: |
| Raw | `frontier:ferrite_ore` | Ferrite Ore | ¤8 |
| Raw | `frontier:silicate_crystals` | Silicate Crystals | ¤10 |
| Raw | `frontier:hydrocarbon_feedstock` | Hydrocarbon Feedstock | ¤12 |
| Raw | `frontier:biomass` | Cultured Biomass | ¤9 |
| Primary | `frontier:structural_alloy` | Structural Alloy | ¤24 |
| Primary | `frontier:ceramic_composite` | Ceramic Composite | ¤30 |
| Primary | `frontier:biopolymer` | Industrial Biopolymer | ¤28 |
| Secondary | `frontier:industrial_machinery` | Industrial Machinery | ¤85 |
| Secondary | `frontier:habitat_modules` | Habitat Modules | ¤100 |
| Secondary | `frontier:reactor_assemblies` | Reactor Assemblies | ¤110 |

Prices are provisional test data. They exist to exercise quotes and trading and are not balance targets.

Conceptual definition:

```ron
(
    id: "frontier:structural_alloy",
    name: "Structural Alloy",
    category: PrimaryProduct,
    base_price: 24,
)
```

### Recipes

Economic behavior is data-defined through recipes rather than hard-coded per good.

```ron
(
    id: "core:primary_recipe_a",
    layer: Primary,
    inputs: [(good: "core:raw_a", quantity: 2)],
    outputs: [(good: "core:processed_a", quantity: 1)],
)
```

Validation rules:

- Quantities are positive integers.
- All referenced goods exist.
- Primary recipes consume at least one raw resource and produce at least one primary product.
- Secondary recipes consume at least one primary product and at least one additional raw or secondary-stage good.
- Secondary recipes produce at least one secondary product.
- Tertiary recipes have inputs and no trade-good outputs.
- Recipe IDs are unique.

For this prototype, recipe execution is atomic: all inputs are consumed and all outputs produced in one simulation tick, or no inventory changes occur.

The initial recipes are:

#### Primary processing

| Facility/recipe | Inputs | Outputs |
| --- | --- | --- |
| Alloy Smelting | 3 Ferrite Ore | 2 Structural Alloy |
| Composite Firing | 2 Silicate Crystals + 1 Hydrocarbon Feedstock | 2 Ceramic Composite |
| Polymer Synthesis | 2 Cultured Biomass + 1 Hydrocarbon Feedstock | 2 Industrial Biopolymer |

#### Secondary manufacturing

| Facility/recipe | Inputs | Outputs |
| --- | --- | --- |
| Machinery Fabrication | 2 Structural Alloy + 1 Hydrocarbon Feedstock | 1 Industrial Machinery |
| Habitat Fabrication | 1 Ceramic Composite + 1 Industrial Biopolymer + 1 Cultured Biomass | 1 Habitat Module |
| Reactor Fabrication | 1 Structural Alloy + 2 Silicate Crystals | 1 Reactor Assembly |

Each secondary recipe consumes at least one primary product plus an additional raw resource. Habitat Fabrication consumes two primary products as well as its raw input.

#### Tertiary consumption sinks

| Site/recipe | Inputs removed |
| --- | --- |
| Settlement Expansion | 1 Habitat Module + 1 Industrial Machinery |
| Orbital Shipyard | 2 Industrial Machinery + 1 Reactor Assembly |
| Research Enclave | 1 Reactor Assembly + 1 Ceramic Composite |

Tertiary recipes produce no tradable output. Their facility names are explanatory content labels; they do not introduce additional simulation mechanics.

### Economic sites

A system may host one or more economic sites. Each site has:

- An inventory keyed by stable good ID
- A currency balance
- Zero or more production/consumption recipes
- Per-good inventory targets used to derive demand
- Per-tick recipe capacity

Raw-resource source sites replenish configured raw goods each tick. This source behavior is data-defined and exists only to supply the prototype economy.

### Supply, demand, and price

The prototype needs observable market differences, not a final pricing model.

Each market computes a quote from:

- A data-defined base price
- Current local inventory
- A data-defined target inventory

The initial implementation uses integer arithmetic:

```text
scarcity = clamp(target inventory - inventory, -target inventory, target inventory)
adjustment = base price × scarcity / (2 × target inventory)
mid price = max(1, base price + adjustment)
market buy quote = max(1, mid price × 90 / 100)
market sell quote = max(1, mid price × 110 / 100)
```

The market buys from a trader at its buy quote and sells to a trader at its sell quote. Integer division rounds down, and target inventory must be positive. A market that does not target a good offers only 45% of its base price when buying it, preventing unrelated empty markets from competing with actual production demand. The formula is isolated behind a pricing system and covered by tests so it can be replaced without changing the TUI or content model.

### Markets and transactions

Each system has one aggregate market entity. Economic sites deposit outputs into and acquire recipe inputs from that market without transferring currency. Traders buy from and sell to the market rather than transacting directly with facilities.

Raw-source replenishment is an explicit goods source, and tertiary consumption is an explicit goods sink. Transactions transfer goods and currency between a market and a trader without creating either. Distance affects travel time and opportunity scoring but does not charge currency in the prototype.

### Automated trade

Automated traders move goods between markets so the simulation can exercise prices, inventory, currency, and distance.

Each trader:

- Exists as an ECS entity
- Has a current system location
- Owns an inventory, cargo capacity, and currency balance
- May evaluate every market reachable through the system graph
- Buys only when it can afford the transaction and has cargo capacity
- Stores and follows a multi-leg shortest-path route
- Travels for a number of ticks derived from each route leg's distance
- Sells only goods it owns

The deterministic selection algorithm is:

1. Calculate shortest paths from the current system to all reachable markets.
2. Evaluate available local goods against destination market buy quotes.
3. Estimate unit profit as destination buy quote minus local sell quote.
4. Rank positive opportunities by estimated profit per travel tick.
5. Break ties by good ID and then destination system ID.
6. Buy the maximum permitted by stock, cargo capacity, and available currency.
7. Follow the selected route, sell on arrival, and repeat.
8. If the arrival market has no profitable local cargo, reposition empty to the source of the best known trade instead of remaining stranded at a demand sink.

The algorithm has full market information in the prototype. Information limits and imperfect estimates are deferred.

Initial market targets are role-specific: resource systems primarily export, production systems target their recipe inputs, and tertiary systems target the goods they consume. Raw source rates are deliberately below aggregate recipe demand, secondary processors begin with small input buffers, and higher-tier base prices preserve value through transformation. Designers can tune global quote percentages, untargeted demand, raw-source output, and idle trader repositioning in `content/economy_config.ron`; per-system targets and inventories remain in `content/economy.ron`.

The initial content creates nine automated traders. Their count, ID/name prefixes, starting currency, cargo capacity, common speed, and distribution strategy are authored in `content/traders.ron`. `EvenlySpaced` assigns them across the ordered 20-system list using centered intervals, placing them at Systems 02, 04, 06, 08, 11, 13, 15, 17, and 19 rather than clustering them near the player.

### Player trader

The player is a trader entity using the same location, inventory, currency, cargo, travel, market transaction, and route components as automated traders. A marker component identifies which trader is controlled by the current frontend; economic systems do not special-case its transactions.

The player chooses actions rather than using the automated opportunity-selection system:

- Buy a selected quantity of a good at the current market
- Sell a selected quantity of an owned good at the current market
- Select any reachable destination
- Inspect the shortest route, total distance, and estimated travel duration
- Begin travel and advance along the route as simulation ticks pass

The player cannot trade while in transit. The prototype exposes complete current market information for all systems so route and economy behavior can be evaluated without an information-discovery system.

### Player economic status

The application computes player-facing statistics from simulation state:

- Current system, or route and remaining travel ticks
- Currency balance
- Cargo used and total cargo capacity
- Cargo value using current-location quotes, or purchase value while traveling
- Net worth: currency plus quoted cargo value
- Lifetime purchase cost and sales revenue
- Realized trading profit
- Total cargo units moved and completed transactions
- Net-worth rank among all traders
- Share of total trader-owned net worth
- Share of cumulative trader sales volume

Statistics are informational and do not imply an explicit victory condition. Rankings use stable trader IDs as deterministic tie-breakers.

## ECS model

The initial component set is expected to include equivalents of:

```text
SystemMarker
StableId
DisplayName
Position3
EconomicSite
Inventory
CurrencyBalance
RecipeSet
RawResourceSource
Trader
PlayerControlled
CurrentLocation
TravelPlan
TradeLedger
```

Shared resources include equivalents of:

```text
SimulationClock
ContentRegistry
SystemGraph
PricingConfiguration
SeededRandom
```

Components contain state. Systems contain behavior. Data definitions are compiled into typed components and resources before simulation begins.

### Tick schedule

Each logical tick runs synchronously on the simulation-owner task:

```text
accept queued application commands
→ complete arrivals
→ replenish configured raw-resource sources
→ execute primary recipes
→ execute secondary recipes
→ execute tertiary sink recipes
→ update market quotes
→ execute trader decisions and transactions
→ begin or advance travel
→ emit events
→ publish a new immutable view snapshot
```

Exact ordering is declared in `game-core/src/schedule.rs` and tested. No ECS system performs asynchronous work.

## Async application model

A Tokio task exclusively owns the `GameSession` and ECS world.

### Requests

The initial request protocol includes:

```rust
pub enum AppRequest {
    SetRunState(RunState),
    Step,
    SetTickRate(TickRate),
    SelectSystem(ContentId),
    Buy { good: ContentId, quantity: u32 },
    Sell { good: ContentId, quantity: u32 },
    BeginTravel { destination: ContentId },
    Shutdown,
}
```

UI-only navigation does not enter the simulation. `SelectSystem` is application view state and must not mutate ECS components.

### Channels

- Bounded `mpsc` channel: TUI requests to the application task
- `watch` channel: latest immutable application view, including bounded recent event history
- `oneshot` response: request acknowledgement or error when needed
- Cancellation token or explicit shutdown signal: coordinated termination

Continuous mode uses a Tokio interval owned by the application task. Timer expiration requests a synchronous logical tick; time is not read from inside ECS systems.

## Application views

The application publishes immutable, TUI-independent view models:

```text
ApplicationView
├── simulation status and tick number
├── SystemListView[20]
├── selected SystemDetailView
├── selected MarketView
├── PlayerStatusView
├── proposed or active RouteView
└── recent EventView entries
```

The system list includes ID, display name, coordinates, and summary economic data. The selected-system view includes named direct connections as well as a named, leg-by-leg shortest route from the player's current system, total route distance, total travel duration, and active-leg progress. Stable IDs remain available for commands but are never used as player-facing route or location labels. The market view includes inventory, target inventory, and current buy/sell quotes. The player-status view includes named location, cargo, finances, trade-history totals, and economy-wide comparative statistics.

View models must not expose `bevy_ecs::Entity`, ECS queries, Ratatui types, or mutable references.

## TUI

The prototype is menu- and table-oriented. It does not render a spatial ASCII map.

### Layout

```text
┌──────────────── Systems ────────────────┐
│ selectable list of 20 systems           │
├──────────────── Details ────────────────┤
│ coordinates, routes, distance, status   │
├──────────────── Market ─────────────────┤
│ goods, inventory, demand, buy/sell      │
├──────────── Player / Trade ─────────────┤
│ status, cargo, buy/sell, route preview   │
├──────────────── Events ─────────────────┤
│ recent simulation events                │
└──────────────── Controls ───────────────┘
│ paused/running | tick | rate | quit     │
└─────────────────────────────────────────┘
```

The final implementation may use side-by-side panes when terminal width permits.

### Required controls

- Move focus between panes
- Move selection within a list or table
- Pause/resume continuous simulation
- Advance exactly one tick while paused
- Change among a small set of tick rates
- Select a market good and buy or sell a quantity
- Preview and begin travel to any reachable system
- Inspect player finances, cargo, trade totals, and comparative status
- Quit cleanly

Exact key bindings will be documented in the UI and may remain provisional.

### Terminal lifecycle

The executable must restore raw mode, cursor state, and alternate-screen state on normal exit and recoverable error paths. Panic-hook restoration should be installed where practical.

## Content files

Initial content layout:

```text
content/
  systems.ron
  goods.ron
  recipes.ron
  economy.ron
  economy_config.ron
  traders.ron
```

Content is loaded and validated before the simulation task starts. The core receives compiled typed definitions and does not read these files itself.

## Observability

Use `tracing` for diagnostics. Because the TUI owns the terminal, logs should go to a file by default during interactive execution.

Simulation events intended for users are typed application data and are separate from diagnostic traces.

Each market keeps cumulative diagnostic accounting for currency paid to and received from traders, traded units, source generation, recipe inputs/outputs, and tertiary consumption. `cargo run -p game-cli -- --economy-diagnostics <ticks>` reports 50-tick activity windows, conserved currency distribution, final market cash flows, and NPC travel/cargo states. These counters are observational and do not alter simulation decisions.

## Testing

### Content validation

- Exactly 20 systems are present.
- IDs are unique and namespace-qualified.
- Coordinates are finite and non-duplicated.
- Route construction produces a connected graph.
- Distances are not uniform.
- All goods and recipe references resolve.
- Recipes obey their layer constraints.

### Core simulation

- Fixed content and seed produce repeatable results.
- A tick executes systems in the declared order.
- Primary recipes consume raw goods and produce primary goods.
- Secondary recipes enforce both required input classes.
- Tertiary recipes remove inputs and produce no goods.
- Transactions conserve goods and currency between participants, except configured sources and sinks.
- Shortest-path queries return deterministic multi-leg routes.
- Travel duration changes with route distance.
- Automated traders can select profitable non-adjacent destinations.
- Player and automated transactions obey the same market rules.
- Invalid or unaffordable transactions do not partially mutate state.
- Net worth and economy-share statistics are calculated consistently.
- Market diagnostics account for successful trades, source generation, and recipe throughput without recording rejected mutations.

### Application boundary

- Requests are processed in order.
- Single-step advances exactly one tick.
- Paused mode does not advance from timer events.
- Continuous mode publishes updated views.
- Shutdown terminates the simulation task cleanly.

### TUI

- Input maps to the expected request or local focus change.
- `ratatui::backend::TestBackend` can render every pane.
- Rendering does not require access to the ECS world.

## Deliverables

The prototype is complete when the repository contains:

1. A compiling Cargo workspace with core, application, content, TUI, and executable boundaries.
2. RON content defining 20 valid systems and the minimum viable economy.
3. A headless deterministic simulation using `bevy_ecs`.
4. An async Tokio application owner and typed channel protocol.
5. A Ratatui interface showing systems, distances, markets, player trading and status, controls, and events.
6. Automated tests covering content validation, economic layers, multi-hop distance-sensitive travel, player trading, async stepping, and terminal rendering.
7. Formatting, Clippy, tests, and content validation runnable in CI.

## Acceptance scenario

1. Run the executable.
2. The content loader validates and compiles the 20-system map and economy.
3. The TUI opens paused and displays all systems.
4. Selecting a system displays its 3D position, connected routes, route distances, inventory, and prices.
5. Single-step advances exactly one logical tick and refreshes the views.
6. Continuous mode advances ticks asynchronously while input remains responsive.
7. Raw goods are produced, primary and secondary goods are processed, and tertiary consumers remove goods.
8. Automated traders can choose profitable multi-hop destinations and travel over distance-dependent route legs.
9. The player can buy goods, select any reachable destination, travel there, and sell goods under the same market rules.
10. The TUI shows the player's finances, cargo, trading totals, and comparative position in the economy.
11. Quitting shuts down the application task and restores the terminal.
12. The same simulation behavior is exercised by tests without initializing a terminal.
