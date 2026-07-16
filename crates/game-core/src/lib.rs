//! Headless, deterministic simulation core for the physical energy economy.

use bevy_ecs::prelude::*;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::fmt::{Display, Formatter};
use thiserror::Error;

mod energy_logistics;

pub use energy_logistics::{
    BulkEnergyHold, CarrierFeeSchedule, ContractId, ContractRoute, EnergyContract,
    EnergyContractBlocker, EnergyContractEvent, EnergyContractIntent,
    EnergyContractOpportunitySnapshot, EnergyContractSnapshot, EnergyContractState,
    EnergyContractTerminalOutcome, EnergyContracts, EnergyLogisticsDiagnostics,
    EnergyLogisticsPolicy, EnergyMarketLogisticsSnapshot, EnergyStarvationCause, LockedEnergyLot,
    PendingEnergyContractIntents,
};

pub const ENERGY_ID: &str = "core:energy";
/// Content-facing upper bound for population sufficiency history.
///
/// This caps both memory use and per-tick history validation work.
pub const MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS: u32 = 10_000;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        let Some((namespace, name)) = value.split_once(':') else {
            return Err(CoreError::InvalidId(value));
        };
        if namespace.is_empty()
            || name.is_empty()
            || !value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '_'))
        {
            return Err(CoreError::InvalidId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Energy(pub i64);

impl Energy {
    pub const ZERO: Self = Self(0);

    pub fn checked_add(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }

    pub fn checked_mul(self, quantity: u64) -> Result<Self, CoreError> {
        let quantity = i64::try_from(quantity).map_err(|_| CoreError::Overflow)?;
        self.0
            .checked_mul(quantity)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position3 {
    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2))
            .sqrt()
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoodCategory {
    Energy,
    Raw,
    Primary,
    Secondary,
}

#[derive(Clone, Debug)]
pub struct GoodDefinition {
    pub id: ContentId,
    pub name: String,
    pub category: GoodCategory,
    /// Initial embodied-energy cost per unit. `core:energy` must be exactly one.
    pub bootstrap_cost: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeLayer {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Clone, Debug)]
pub struct GoodAmount {
    pub good: ContentId,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct RecipeOutput {
    pub good: ContentId,
    pub quantity: u32,
    pub cost_weight: u32,
}

#[derive(Clone, Debug)]
pub struct RecipeDefinition {
    pub id: ContentId,
    pub name: String,
    pub layer: RecipeLayer,
    pub inputs: Vec<GoodAmount>,
    pub outputs: Vec<RecipeOutput>,
    pub operating_energy: Energy,
    pub margin_percent: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDefinition {
    pub good: ContentId,
    pub quantity_per_tick: u32,
    pub extraction_energy: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PricingMode {
    Scarcity,
    CostAware,
}

/// Ordered severity ladder derived after generation and mandatory life support.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum BrownoutStage {
    #[default]
    Normal,
    Throttled,
    Emergency,
    Starvation,
}

impl BrownoutStage {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Throttled => "Throttled",
            Self::Emergency => "Emergency",
            Self::Starvation => "Starvation",
        }
    }

    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Throttled => 1,
            Self::Emergency => 2,
            Self::Starvation => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrownoutConfig {
    pub throttled_entry_ticks: u32,
    pub emergency_entry_ticks: u32,
    pub starvation_entry_ticks: u32,
    pub throttled_recovery_ticks: u32,
    pub emergency_recovery_ticks: u32,
    pub starvation_recovery_ticks: u32,
    pub minimum_stage_ticks: u32,
    pub throttled_throughput_percent: u32,
    pub emergency_throughput_percent: u32,
    pub starvation_throughput_percent: u32,
    pub survival_goods: BTreeSet<ContentId>,
}

impl Default for BrownoutConfig {
    fn default() -> Self {
        Self {
            throttled_entry_ticks: 12,
            emergency_entry_ticks: 6,
            starvation_entry_ticks: 1,
            throttled_recovery_ticks: 16,
            emergency_recovery_ticks: 8,
            starvation_recovery_ticks: 3,
            minimum_stage_ticks: 1,
            throttled_throughput_percent: 50,
            emergency_throughput_percent: 0,
            starvation_throughput_percent: 0,
            survival_goods: BTreeSet::from([ContentId::new(ENERGY_ID).expect("constant id")]),
        }
    }
}

impl BrownoutConfig {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.starvation_entry_ticks >= self.emergency_entry_ticks
            || self.emergency_entry_ticks >= self.throttled_entry_ticks
            || self.starvation_recovery_ticks <= self.starvation_entry_ticks
            || self.emergency_recovery_ticks <= self.emergency_entry_ticks
            || self.throttled_recovery_ticks <= self.throttled_entry_ticks
            || self.starvation_recovery_ticks >= self.emergency_recovery_ticks
            || self.emergency_recovery_ticks >= self.throttled_recovery_ticks
            || self.minimum_stage_ticks == 0
            || self.throttled_throughput_percent > 100
            || self.emergency_throughput_percent > self.throttled_throughput_percent
            || self.starvation_throughput_percent > self.emergency_throughput_percent
            || !self
                .survival_goods
                .iter()
                .any(|id| id.as_str() == ENERGY_ID)
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
        Ok(())
    }

    #[must_use]
    pub fn throughput_percent(&self, stage: BrownoutStage) -> u32 {
        match stage {
            BrownoutStage::Normal => 100,
            BrownoutStage::Throttled => self.throttled_throughput_percent,
            BrownoutStage::Emergency => self.emergency_throughput_percent,
            BrownoutStage::Starvation => self.starvation_throughput_percent,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrownoutState {
    pub stage: BrownoutStage,
    pub entered_at_tick: u64,
    pub transition_count: u64,
    pub occupancy_ticks: [u64; 4],
    pub ticks_of_burn: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketOperatingProfile {
    pub stage: BrownoutStage,
    pub throughput_percent: u32,
    pub labor_percent: u32,
    pub investment_allowed: bool,
}

impl Default for MarketOperatingProfile {
    fn default() -> Self {
        Self {
            stage: BrownoutStage::Normal,
            throughput_percent: 100,
            labor_percent: 100,
            investment_allowed: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeasonalGenerationState {
    pub base_output: Energy,
    pub amplitude_percent: u32,
    pub period_ticks: u32,
    pub phase_ticks: u32,
    pub current_effective_output: Energy,
}

impl SeasonalGenerationState {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.base_output.0 < 0
            || self.amplitude_percent > 100
            || self.period_ticks < 2
            || (self.amplitude_percent > 0 && !self.period_ticks.is_multiple_of(2))
            || self.phase_ticks >= self.period_ticks
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
        let maximum = i128::from(self.base_output.0)
            .checked_mul(i128::from(100_u32 + self.amplitude_percent))
            .ok_or(CoreError::Overflow)?
            / 100;
        i64::try_from(maximum).map_err(|_| CoreError::Overflow)?;
        Ok(())
    }

    pub fn effective_output_at(&self, tick: u64) -> Result<Energy, CoreError> {
        triangle_wave_output(
            self.base_output,
            self.amplitude_percent,
            self.period_ticks,
            self.phase_ticks,
            tick,
        )
    }

    pub fn phase_at(&self, tick: u64) -> Result<SeasonalPhaseSnapshot, CoreError> {
        seasonal_phase(self.period_ticks, self.phase_ticks, tick)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeasonalTrend {
    Rising,
    Falling,
}

impl SeasonalTrend {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Falling => "falling",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeasonalPhaseSnapshot {
    pub position_ticks: u32,
    pub period_ticks: u32,
    pub trend: SeasonalTrend,
    pub ticks_until_turning_point: u32,
    pub next_turning_point_tick: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopulationConfig {
    pub static_population: bool,
    pub sufficiency_window: u32,
    pub growth_sufficiency_percent: u32,
    pub essential_goods: BTreeSet<ContentId>,
    pub tertiary_demand_per_thousand: BTreeMap<ContentId, u32>,
    pub decline_per_thousand: u32,
    pub growth_per_thousand: u32,
    pub logistic_scale: u32,
    pub minimum_cap: u64,
    pub maximum_cap: u64,
    pub tier_thresholds: Vec<u64>,
}

impl Default for PopulationConfig {
    fn default() -> Self {
        Self {
            static_population: true,
            sufficiency_window: 500,
            growth_sufficiency_percent: 90,
            essential_goods: BTreeSet::from([ContentId::new(ENERGY_ID).expect("constant id")]),
            tertiary_demand_per_thousand: BTreeMap::new(),
            decline_per_thousand: 10,
            growth_per_thousand: 1,
            logistic_scale: 1_000,
            minimum_cap: 0,
            maximum_cap: 1_000_000,
            tier_thresholds: vec![1],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PopulationTrend {
    Growing,
    Declining,
    #[default]
    Stable,
}

impl PopulationTrend {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Growing => "growing",
            Self::Declining => "declining",
            Self::Stable => "stable",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LogisticGrowthCarry {
    /// Fractional numerator retained from the previous growth calculation.
    pub remainder: u64,
    /// Denominator paired with `remainder`. Capacity changes preserve an
    /// exactly compatible denominator and otherwise convert the pair atomically.
    pub denominator: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PopulationState {
    pub current: u64,
    pub reference: u64,
    /// The cap currently supported by the bounded supply history.
    pub carrying_capacity: u64,
    /// The authored/investment-adjusted upper bound from which history derives
    /// the current carrying capacity.
    pub support_capacity: u64,
    /// Per-market percentage bonus applied only to the approved gated logistic
    /// growth rate by population-support investment.
    pub growth_rate_bonus_percent: u32,
    /// Oldest-to-newest fixed-point percentage samples (0..=100). Its length
    /// is bounded by `MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS`.
    pub sufficiency_samples: VecDeque<u32>,
    pub sufficiency_sum: u64,
    pub sufficiency_average_percent: u32,
    pub growth_carry: LogisticGrowthCarry,
    pub decline_remainder: u64,
    pub growth_ticks: u64,
    pub decline_ticks: u64,
    pub settled_changes: u64,
    pub trend: PopulationTrend,
    pub tier: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvestmentKind {
    Collector,
    Storage,
    PopulationSupport,
    RouteSubsidy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvestmentShape {
    pub enabled: bool,
    pub base_cost: Energy,
    pub cost_growth_percent: u32,
    pub maximum_level: u32,
    pub cooldown_ticks: u32,
    pub effect_per_level: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InvestmentPolicy {
    pub allocation_percent: BTreeMap<InvestmentKind, u32>,
}

/// Player-facing investment controls. Core retains ownership of the complete
/// investment policy and merges only these approved allocations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GovernorInvestmentPolicy {
    pub allocation_percent: BTreeMap<InvestmentKind, u32>,
}

impl From<GovernorInvestmentPolicy> for InvestmentPolicy {
    fn from(value: GovernorInvestmentPolicy) -> Self {
        Self {
            allocation_percent: value.allocation_percent,
        }
    }
}

impl InvestmentPolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        let total = self
            .allocation_percent
            .values()
            .try_fold(0_u32, |sum, value| {
                if *value > 100 {
                    return Err(CoreError::InvalidInvestmentPolicy);
                }
                sum.checked_add(*value).ok_or(CoreError::Overflow)
            })?;
        if total > 100 {
            return Err(CoreError::InvalidInvestmentPolicy);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvestmentStatus {
    Disabled,
    DisabledByStage(BrownoutStage),
    Unallocated,
    CoolingDown { until_tick: u64 },
    MaximumLevel,
    InsufficientFunds { available: Energy, cost: Energy },
    Ready { cost: Energy },
    Completed { tick: u64, cost: Energy },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InvestmentState {
    pub levels: BTreeMap<InvestmentKind, u32>,
    pub cooldown_until: BTreeMap<InvestmentKind, u64>,
    pub status: BTreeMap<InvestmentKind, InvestmentStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FleetMode {
    Fixed {
        count: usize,
    },
    Dynamic {
        initial_count: usize,
        opportunity_threshold: u64,
        opportunity_window: u32,
        spawn_cooldown_ticks: u32,
        retirement_window: u32,
        retirement_threshold: i64,
        maximum_count: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FleetArchetype {
    pub id: ContentId,
    pub id_prefix: String,
    pub name_prefix: String,
    pub initial_count: usize,
    pub maximum_count: usize,
    pub starting_tank: Energy,
    pub energy_tank_capacity: Energy,
    pub bulk_energy_capacity: Energy,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel_burn_per_distance: Energy,
    pub refuel_policy: RefuelPolicy,
}

impl FleetArchetype {
    #[must_use]
    pub fn liquidation_capability(&self) -> LiquidationTraderCapability {
        LiquidationTraderCapability {
            cargo_capacity: self.cargo_capacity,
            energy_tank_capacity: self.energy_tank_capacity,
            travel_burn_per_distance: self.travel_burn_per_distance,
        }
    }
}

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FleetDynamics {
    pub mode: Option<FleetMode>,
    /// Stable-ID ordered NPC archetype registry. Total caps live in `mode` and
    /// each archetype contributes its own initial and maximum count.
    pub archetypes: BTreeMap<ContentId, FleetArchetype>,
    /// Canonical profitable request score left after one request per idle NPC,
    /// normalized by system count.
    pub normalized_unserved_opportunity: u64,
    pub opportunity_persistence: u32,
    pub spawn_sequence: u64,
    pub spawn_cooldown_until: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarketAuthority {
    Autonomous,
    Player(ContentId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Governance {
    pub authority: MarketAuthority,
}

impl Default for Governance {
    fn default() -> Self {
        Self {
            authority: MarketAuthority::Autonomous,
        }
    }
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct AggregateDynamicsHistory {
    pub stage_occupancy_ticks: [u64; 4],
    pub stage_transitions: u64,
    pub population_changes: u64,
    pub population_milestones: u64,
    pub fleet_spawns: u64,
    pub fleet_retirements: u64,
    pub investments_completed: u64,
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct MarketPolicy {
    pub pricing_mode: PricingMode,
    pub producer_margin_percent: u32,
    pub operating_reserve_ticks: u32,
    pub import_priorities: BTreeMap<ContentId, u32>,
    pub liquidation_threshold_percent: u32,
    pub liquidation_discount_percent: u32,
    pub default_target: u32,
}

impl Default for MarketPolicy {
    fn default() -> Self {
        Self {
            pricing_mode: PricingMode::CostAware,
            producer_margin_percent: 15,
            operating_reserve_ticks: 3,
            import_priorities: BTreeMap::new(),
            liquidation_threshold_percent: 200,
            liquidation_discount_percent: 50,
            default_target: 10,
        }
    }
}

/// Player-facing market controls. Pricing mode, liquidation policy, and
/// default targets intentionally remain core-owned and are absent here.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GovernorMarketPolicy {
    pub producer_margin_percent: u32,
    pub operating_reserve_ticks: u32,
    pub import_priorities: BTreeMap<ContentId, u32>,
}

impl MarketPolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.producer_margin_percent > 10_000
            || self.liquidation_threshold_percent < 100
            || self.liquidation_discount_percent > 100
            || self.default_target == 0
            || self
                .import_priorities
                .keys()
                .any(|good| good.as_str() == ENERGY_ID)
        {
            return Err(CoreError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SystemDefinition {
    pub id: ContentId,
    pub name: String,
    pub position: Position3,
    /// Unified physical inventory. `core:energy` is the canonical market stock.
    pub inventory: BTreeMap<ContentId, u64>,
    pub targets: BTreeMap<ContentId, u32>,
    pub recipes: Vec<ContentId>,
    pub sources: Vec<SourceDefinition>,
    pub energy_output_per_tick: Energy,
    pub seasonal_generation: SeasonalGenerationState,
    pub energy_storage_cap: Energy,
    pub population: u64,
    pub population_state: PopulationState,
    pub investment_policy: InvestmentPolicy,
    pub governance: Governance,
    pub policy: MarketPolicy,
    /// Fully resolved policy for this market; content overrides are compiled before core.
    pub energy_logistics: EnergyLogisticsPolicy,
    /// Graph/content-compiled anti-strand reserve; never derived from policy knobs.
    pub protected_liquidation_budget: Energy,
    pub bootstrap_risk_acknowledged: bool,
}

/// Player progression capability for access to networked trade reservations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TradeNetworkAccess {
    /// Player reservation-backed trade commitments are unavailable.
    #[default]
    Offline,
    /// Allows player commands to create reservation-backed trade commitments.
    ReservationContracts,
}

/// Canonical constraint that currently caps an immediate local trade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalTradeLimitReason {
    TradingUnavailable,
    MarketQuote,
    MarketStock,
    CargoCapacity,
    TankEnergy,
    MarketEnergyStorage,
    UnitsHeld,
    MarketFunding,
    TankCapacity,
    QuantityType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTradeQuantityLimit {
    pub maximum: u32,
    pub reason: LocalTradeLimitReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTradeLimits {
    pub buy: LocalTradeQuantityLimit,
    pub sell: LocalTradeQuantityLimit,
}

#[derive(Clone, Debug)]
pub struct TraderDefinition {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub archetype: Option<ContentId>,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
    pub bulk_energy_capacity: Energy,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel_burn_per_distance: Energy,
    pub refuel_policy: RefuelPolicy,
    pub player: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefuelPolicy {
    DepositAndWithdraw,
    DepositOnly,
    Disabled,
}

impl RefuelPolicy {
    #[must_use]
    pub fn permits_deposit(self) -> bool {
        matches!(self, Self::DepositAndWithdraw | Self::DepositOnly)
    }

    #[must_use]
    pub fn permits_withdrawal(self) -> bool {
        matches!(self, Self::DepositAndWithdraw)
    }
}

#[derive(Clone, Debug)]
pub struct GameDefinition {
    pub goods: Vec<GoodDefinition>,
    pub recipes: Vec<RecipeDefinition>,
    pub systems: Vec<SystemDefinition>,
    pub traders: Vec<TraderDefinition>,
    /// Authored initial capability for the one player-controlled trader.
    pub player_trade_network_access: TradeNetworkAccess,
    pub fleet: FleetDynamics,
    pub economy: EconomyConfig,
}

#[derive(Resource, Clone, Debug)]
pub struct EconomyConfig {
    pub reservation_ttl: u32,
    pub life_support_burn_per_capita: Energy,
    pub source_output_percent: u32,
    pub idle_trader_repositioning: bool,
    pub brownouts: BrownoutConfig,
    pub energy_logistics: EnergyLogisticsPolicy,
    pub population: PopulationConfig,
    pub investments: BTreeMap<InvestmentKind, InvestmentShape>,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            reservation_ttl: 20,
            life_support_burn_per_capita: Energy(1),
            source_output_percent: 100,
            idle_trader_repositioning: true,
            brownouts: BrownoutConfig::default(),
            energy_logistics: EnergyLogisticsPolicy::default(),
            population: PopulationConfig::default(),
            investments: default_investment_shapes(),
        }
    }
}

fn default_investment_shapes() -> BTreeMap<InvestmentKind, InvestmentShape> {
    [
        InvestmentKind::Collector,
        InvestmentKind::Storage,
        InvestmentKind::PopulationSupport,
        InvestmentKind::RouteSubsidy,
    ]
    .into_iter()
    .map(|kind| {
        (
            kind,
            InvestmentShape {
                enabled: false,
                base_cost: Energy(1_000),
                cost_growth_percent: 150,
                maximum_level: 0,
                cooldown_ticks: 100,
                effect_per_level: 0,
            },
        )
    })
    .collect()
}

/// Applies stage and labor percentages multiplicatively and carries only the
/// final composed remainder. The scale is 100 × 100.
pub fn composed_throughput(
    base: u64,
    stage_percent: u32,
    labor_percent: u32,
    carry: &mut u64,
) -> Result<u64, CoreError> {
    if stage_percent > 100 || labor_percent > 100 || *carry >= 10_000 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let numerator = u128::from(base)
        .checked_mul(u128::from(stage_percent))
        .and_then(|value| value.checked_mul(u128::from(labor_percent)))
        .and_then(|value| value.checked_add(u128::from(*carry)))
        .ok_or(CoreError::Overflow)?;
    let whole = numerator / 10_000;
    *carry = u64::try_from(numerator % 10_000).map_err(|_| CoreError::Overflow)?;
    u64::try_from(whole).map_err(|_| CoreError::Overflow)
}

pub fn triangle_wave_output(
    base: Energy,
    amplitude_percent: u32,
    period_ticks: u32,
    phase_ticks: u32,
    tick: u64,
) -> Result<Energy, CoreError> {
    if base.0 < 0
        || amplitude_percent > 100
        || period_ticks < 2
        || (amplitude_percent > 0 && !period_ticks.is_multiple_of(2))
        || phase_ticks >= period_ticks
    {
        return Err(CoreError::InvalidWorldDynamics);
    }
    if amplitude_percent == 0 || base == Energy::ZERO {
        return Ok(base);
    }
    let period = u64::from(period_ticks);
    let position = (tick % period + u64::from(phase_ticks)) % period;
    // A deterministic triangle in [-period, +period], starting at its trough.
    let doubled = position.checked_mul(2).ok_or(CoreError::Overflow)?;
    let triangle = if doubled <= period {
        i128::from(doubled)
            .checked_mul(2)
            .ok_or(CoreError::Overflow)?
            - i128::from(period)
    } else {
        i128::from(period)
            .checked_mul(3)
            .ok_or(CoreError::Overflow)?
            - i128::from(doubled)
                .checked_mul(2)
                .ok_or(CoreError::Overflow)?
    };
    let adjustment = i128::from(base.0)
        .checked_mul(i128::from(amplitude_percent))
        .and_then(|value| value.checked_mul(triangle))
        .ok_or(CoreError::Overflow)?
        / (100 * i128::from(period));
    let output = i128::from(base.0)
        .checked_add(adjustment)
        .ok_or(CoreError::Overflow)?;
    Ok(Energy(
        i64::try_from(output.max(0)).map_err(|_| CoreError::Overflow)?,
    ))
}

pub fn seasonal_phase(
    period_ticks: u32,
    phase_ticks: u32,
    tick: u64,
) -> Result<SeasonalPhaseSnapshot, CoreError> {
    if period_ticks < 2 || phase_ticks >= period_ticks {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let period = u64::from(period_ticks);
    let position = (tick % period + u64::from(phase_ticks)) % period;
    let crest = u64::from(period_ticks.div_ceil(2));
    let (trend, ticks_until_turning_point) = if position < crest {
        (SeasonalTrend::Rising, crest - position)
    } else {
        (SeasonalTrend::Falling, period - position)
    };
    let ticks_until_turning_point =
        u32::try_from(ticks_until_turning_point).map_err(|_| CoreError::Overflow)?;
    Ok(SeasonalPhaseSnapshot {
        position_ticks: u32::try_from(position).map_err(|_| CoreError::Overflow)?,
        period_ticks,
        trend,
        ticks_until_turning_point,
        next_turning_point_tick: tick.checked_add(u64::from(ticks_until_turning_point)),
    })
}

pub fn classify_brownout(
    state: &BrownoutState,
    config: &BrownoutConfig,
    ticks_of_burn: u32,
    unsupplied_life_support: Energy,
    tick: u64,
) -> Result<BrownoutStage, CoreError> {
    config.validate()?;
    let entry = if unsupplied_life_support.0 > 0 || ticks_of_burn <= config.starvation_entry_ticks {
        BrownoutStage::Starvation
    } else if ticks_of_burn <= config.emergency_entry_ticks {
        BrownoutStage::Emergency
    } else if ticks_of_burn <= config.throttled_entry_ticks {
        BrownoutStage::Throttled
    } else {
        BrownoutStage::Normal
    };
    if entry > state.stage {
        return Ok(entry);
    }
    if tick.saturating_sub(state.entered_at_tick) < u64::from(config.minimum_stage_ticks) {
        return Ok(state.stage);
    }
    Ok(match state.stage {
        BrownoutStage::Starvation if ticks_of_burn >= config.starvation_recovery_ticks => {
            BrownoutStage::Emergency
        }
        BrownoutStage::Emergency if ticks_of_burn >= config.emergency_recovery_ticks => {
            BrownoutStage::Throttled
        }
        BrownoutStage::Throttled if ticks_of_burn >= config.throttled_recovery_ticks => {
            BrownoutStage::Normal
        }
        current => current,
    })
}

const MAX_FLEET_WINDOW_TICKS: u32 = 10_000;

fn dynamic_trader_id(archetype: &FleetArchetype, sequence: u64) -> Result<ContentId, CoreError> {
    ContentId::new(format!("{}_dynamic_{sequence:08}", archetype.id_prefix))
}

fn validate_fleet_definition(
    fleet: &FleetDynamics,
    traders: &[TraderDefinition],
) -> Result<(), CoreError> {
    let Some(mode) = &fleet.mode else {
        return Err(CoreError::InvalidWorldDynamics);
    };
    if matches!(mode, FleetMode::Dynamic { .. }) && fleet.archetypes.is_empty() {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let mut prefixes = BTreeSet::new();
    for (id, archetype) in &fleet.archetypes {
        if id != &archetype.id
            || archetype.initial_count > archetype.maximum_count
            || archetype.maximum_count == 0
            || archetype.starting_tank.0 <= 0
            || archetype.starting_tank > archetype.energy_tank_capacity
            || archetype.energy_tank_capacity.0 <= 0
            || archetype.bulk_energy_capacity.0 < 0
            || archetype.cargo_capacity == 0
            || !archetype.speed.is_finite()
            || archetype.speed <= 0.0
            || archetype.travel_burn_per_distance.0 <= 0
            || dynamic_trader_id(archetype, 1).is_err()
            || archetype.name_prefix.trim().is_empty()
            || !prefixes.insert(archetype.id_prefix.as_str())
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
    }
    if prefixes.iter().any(|left| {
        prefixes.iter().any(|right| {
            left != right
                && (left.starts_with(&format!("{right}_"))
                    || right.starts_with(&format!("{left}_")))
        })
    }) {
        return Err(CoreError::InvalidWorldDynamics);
    }
    if let FleetMode::Dynamic {
        initial_count,
        opportunity_threshold,
        opportunity_window,
        spawn_cooldown_ticks,
        retirement_window,
        maximum_count,
        ..
    } = mode
    {
        let authored_initial = fleet
            .archetypes
            .values()
            .try_fold(0_usize, |sum, value| sum.checked_add(value.initial_count))
            .ok_or(CoreError::Overflow)?;
        let authored_maximum = fleet
            .archetypes
            .values()
            .try_fold(0_usize, |sum, value| sum.checked_add(value.maximum_count))
            .ok_or(CoreError::Overflow)?;
        let mut actual = BTreeMap::<ContentId, usize>::new();
        for trader in traders.iter().filter(|trader| !trader.player) {
            let archetype = trader
                .archetype
                .as_ref()
                .ok_or(CoreError::InvalidWorldDynamics)?;
            if !fleet.archetypes.contains_key(archetype) {
                return Err(CoreError::InvalidWorldDynamics);
            }
            *actual.entry(archetype.clone()).or_default() += 1;
        }
        if *opportunity_threshold == 0
            || *opportunity_window == 0
            || *opportunity_window > MAX_FLEET_WINDOW_TICKS
            || *spawn_cooldown_ticks == 0
            || *retirement_window == 0
            || *retirement_window > MAX_FLEET_WINDOW_TICKS
            || *maximum_count == 0
            || *initial_count > *maximum_count
            || *maximum_count > authored_maximum
            || *initial_count != authored_initial
            || *initial_count != traders.iter().filter(|trader| !trader.player).count()
            || fleet.archetypes.values().any(|archetype| {
                actual.get(&archetype.id).copied().unwrap_or(0) != archetype.initial_count
                    || traders.iter().any(|trader| {
                        trader
                            .id
                            .as_str()
                            .starts_with(&format!("{}_dynamic_", archetype.id_prefix))
                    })
            })
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
    }
    Ok(())
}

pub fn update_opportunity_persistence(
    current: u32,
    opportunity: u64,
    threshold: u64,
) -> Result<u32, CoreError> {
    if threshold == 0 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    if opportunity >= threshold {
        current.checked_add(1).ok_or(CoreError::Overflow)
    } else {
        Ok(0)
    }
}

pub fn investment_cost(shape: &InvestmentShape, level: u32) -> Result<Energy, CoreError> {
    if shape.base_cost.0 <= 0 || shape.cost_growth_percent < 100 || level >= shape.maximum_level {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let mut cost = shape.base_cost;
    for _ in 0..level {
        cost = checked_mul_ratio_ceil(cost, u64::from(shape.cost_growth_percent), 100)?;
    }
    Ok(cost)
}

fn valid_logistic_growth_carry(carry: LogisticGrowthCarry, config: &PopulationConfig) -> bool {
    if carry.denominator == 0 {
        return carry.remainder == 0;
    }
    let denominator_per_capita = u64::from(config.logistic_scale).checked_mul(1_000);
    carry.remainder < carry.denominator
        && denominator_per_capita.is_some_and(|unit| {
            carry.denominator.is_multiple_of(unit) && carry.denominator / unit <= config.maximum_cap
        })
}

pub fn validate_population_config(config: &PopulationConfig) -> Result<(), CoreError> {
    let decline = u64::from(config.decline_per_thousand);
    let growth = u64::from(config.growth_per_thousand);
    if config.sufficiency_window == 0
        || config.sufficiency_window > MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS
        || !(1..=100).contains(&config.growth_sufficiency_percent)
        || config.essential_goods.is_empty()
        || config
            .tertiary_demand_per_thousand
            .values()
            .any(|value| *value == 0)
        || growth == 0
        || decline < growth.checked_mul(5).ok_or(CoreError::Overflow)?
        || decline > growth.checked_mul(10).ok_or(CoreError::Overflow)?
        || config.decline_per_thousand > 1_000
        || config.logistic_scale == 0
        || u128::from(config.maximum_cap)
            .checked_mul(1_000)
            .and_then(|value| value.checked_mul(u128::from(config.logistic_scale)))
            .is_none_or(|denominator| denominator > u128::from(u64::MAX))
        || config.minimum_cap > config.maximum_cap
        || config.tier_thresholds.is_empty()
        || config.tier_thresholds[0] == 0
        || config
            .tier_thresholds
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || config
            .tier_thresholds
            .last()
            .is_some_and(|threshold| *threshold > config.maximum_cap)
    {
        return Err(CoreError::InvalidWorldDynamics);
    }
    Ok(())
}

pub fn validate_investment_shapes(
    shapes: &BTreeMap<InvestmentKind, InvestmentShape>,
    population: &PopulationConfig,
) -> Result<(), CoreError> {
    for kind in [
        InvestmentKind::Collector,
        InvestmentKind::Storage,
        InvestmentKind::PopulationSupport,
        InvestmentKind::RouteSubsidy,
    ] {
        let shape = shapes.get(&kind).ok_or(CoreError::InvalidWorldDynamics)?;
        if shape.base_cost.0 <= 0
            || shape.cost_growth_percent < 100
            || shape.cooldown_ticks == 0
            || shape.maximum_level > 10_000
            || (shape.enabled
                && (shape.maximum_level == 0
                    || shape.effect_per_level == 0
                    || shape
                        .effect_per_level
                        .checked_mul(shape.maximum_level)
                        .is_none()))
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
        for level in 0..shape.maximum_level {
            investment_cost(shape, level)?;
        }
        if !shape.enabled {
            continue;
        }
        let cumulative = shape
            .effect_per_level
            .checked_mul(shape.maximum_level)
            .ok_or(CoreError::InvalidWorldDynamics)?;
        match kind {
            InvestmentKind::RouteSubsidy => {
                100_u32
                    .checked_add(cumulative)
                    .ok_or(CoreError::InvalidWorldDynamics)?;
            }
            InvestmentKind::PopulationSupport => {
                let multiplier = 100_u32
                    .checked_add(cumulative)
                    .ok_or(CoreError::InvalidWorldDynamics)?;
                let effective_growth = u128::from(population.growth_per_thousand)
                    .checked_mul(u128::from(multiplier))
                    .ok_or(CoreError::InvalidWorldDynamics)?
                    / 100;
                let effective_growth =
                    u32::try_from(effective_growth).map_err(|_| CoreError::InvalidWorldDynamics)?;
                population
                    .maximum_cap
                    .checked_add(u64::from(cumulative))
                    .ok_or(CoreError::InvalidWorldDynamics)?;
                let left = population.maximum_cap / 2;
                let right = population.maximum_cap - left;
                let maximum_denominator = u128::from(population.maximum_cap)
                    .checked_mul(1_000)
                    .and_then(|value| value.checked_mul(u128::from(population.logistic_scale)))
                    .ok_or(CoreError::InvalidWorldDynamics)?;
                u128::from(left)
                    .checked_mul(u128::from(right))
                    .and_then(|value| value.checked_mul(u128::from(effective_growth)))
                    .and_then(|value| value.checked_add(maximum_denominator.saturating_sub(1)))
                    .ok_or(CoreError::InvalidWorldDynamics)?;
            }
            InvestmentKind::Collector | InvestmentKind::Storage => {}
        }
    }
    Ok(())
}

/// Validates effects whose safety depends on an authored market's starting
/// generation or storage state.
pub fn validate_market_investment_bounds(
    shapes: &BTreeMap<InvestmentKind, InvestmentShape>,
    seasonal_generation: &SeasonalGenerationState,
    energy_storage_cap: Energy,
) -> Result<(), CoreError> {
    let cumulative = |kind| -> Result<Energy, CoreError> {
        let shape = shapes.get(&kind).ok_or(CoreError::InvalidWorldDynamics)?;
        if !shape.enabled {
            return Ok(Energy::ZERO);
        }
        let amount = u64::from(shape.effect_per_level)
            .checked_mul(u64::from(shape.maximum_level))
            .ok_or(CoreError::InvalidWorldDynamics)?;
        Ok(Energy(
            i64::try_from(amount).map_err(|_| CoreError::InvalidWorldDynamics)?,
        ))
    };
    let collector = cumulative(InvestmentKind::Collector)?;
    let mut maximum_generation = seasonal_generation.clone();
    maximum_generation.base_output = maximum_generation
        .base_output
        .checked_add(collector)
        .map_err(|_| CoreError::InvalidWorldDynamics)?;
    maximum_generation.validate().map_err(|error| match error {
        CoreError::Overflow => CoreError::InvalidWorldDynamics,
        other => other,
    })?;
    energy_storage_cap
        .checked_add(cumulative(InvestmentKind::Storage)?)
        .map_err(|_| CoreError::InvalidWorldDynamics)?;
    Ok(())
}

pub fn population_labor_percent(current: u64, reference: u64) -> Result<u32, CoreError> {
    if reference == 0 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let percent = u128::from(current)
        .checked_mul(100)
        .ok_or(CoreError::Overflow)?
        / u128::from(reference);
    u32::try_from(percent.min(100)).map_err(|_| CoreError::Overflow)
}

pub fn population_tier(population: u64, thresholds: &[u64]) -> usize {
    thresholds
        .iter()
        .take_while(|threshold| population >= **threshold)
        .count()
}

pub fn population_demand_target(
    authored_target: u32,
    population: u64,
    reference: u64,
    units_per_thousand: u32,
) -> Result<u32, CoreError> {
    if reference == 0 || units_per_thousand == 0 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let (numerator, denominator) = if authored_target > 0 {
        (
            u128::from(authored_target)
                .checked_mul(u128::from(population))
                .ok_or(CoreError::Overflow)?,
            u128::from(reference),
        )
    } else {
        (
            u128::from(population)
                .checked_mul(u128::from(units_per_thousand))
                .ok_or(CoreError::Overflow)?,
            1_000,
        )
    };
    if numerator == 0 {
        return Ok(0);
    }
    let rounded = numerator
        .checked_add(denominator - 1)
        .ok_or(CoreError::Overflow)?
        / denominator;
    u32::try_from(rounded).map_err(|_| CoreError::Overflow)
}

pub fn proportional_population_delta(
    population: u64,
    rate_per_thousand: u32,
    remainder: &mut u64,
) -> Result<u64, CoreError> {
    if rate_per_thousand == 0 || rate_per_thousand > 1_000 || *remainder >= 1_000 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let numerator = u128::from(population)
        .checked_mul(u128::from(rate_per_thousand))
        .and_then(|value| value.checked_add(u128::from(*remainder)))
        .ok_or(CoreError::Overflow)?;
    let next_remainder = u64::try_from(numerator % 1_000).map_err(|_| CoreError::Overflow)?;
    let delta = u64::try_from(numerator / 1_000)
        .map_err(|_| CoreError::Overflow)?
        .min(population);
    *remainder = next_remainder;
    Ok(delta)
}

fn rebase_fraction_half_even(
    remainder: u64,
    old_denominator: u64,
    new_denominator: u64,
) -> Result<u64, CoreError> {
    let scaled = u128::from(remainder)
        .checked_mul(u128::from(new_denominator))
        .ok_or(CoreError::Overflow)?;
    let old_denominator = u128::from(old_denominator);
    let quotient = scaled / old_denominator;
    let conversion_remainder = scaled % old_denominator;
    let twice_remainder = conversion_remainder
        .checked_mul(2)
        .ok_or(CoreError::Overflow)?;
    let rounds_up = twice_remainder > old_denominator
        || (twice_remainder == old_denominator && !quotient.is_multiple_of(2));
    let rounded = quotient
        .checked_add(u128::from(rounds_up))
        .ok_or(CoreError::Overflow)?;
    // A carry is strictly fractional. At the upper representable boundary,
    // retain the nearest valid carry rather than creating an invalid pair.
    u64::try_from(rounded)
        .map_err(|_| CoreError::Overflow)
        .map(|value| value.min(new_denominator - 1))
}

pub fn logistic_population_delta(
    population: u64,
    carrying_capacity: u64,
    rate_per_thousand: u32,
    scale: u32,
    carry: &mut LogisticGrowthCarry,
) -> Result<u64, CoreError> {
    if scale == 0
        || carrying_capacity == 0
        || (carry.denominator == 0 && carry.remainder != 0)
        || (carry.denominator != 0 && carry.remainder >= carry.denominator)
    {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let active_denominator = u128::from(carrying_capacity)
        .checked_mul(1_000)
        .and_then(|value| value.checked_mul(u128::from(scale)))
        .ok_or(CoreError::Overflow)?;
    let active_denominator = u64::try_from(active_denominator).map_err(|_| CoreError::Overflow)?;

    // Preserve the finer denominator whenever either denominator exactly
    // represents the other. This avoids any directional rounding under
    // alternating compatible capacities. Incompatible pairs use checked
    // round-half-to-even conversion rather than biased half-up conversion.
    let (denominator, rebased_remainder, growth_multiplier) = if carry.denominator == 0 {
        (active_denominator, 0, 1)
    } else if carry.denominator.is_multiple_of(active_denominator) {
        (
            carry.denominator,
            carry.remainder,
            carry.denominator / active_denominator,
        )
    } else if active_denominator.is_multiple_of(carry.denominator) {
        let multiplier = active_denominator / carry.denominator;
        let remainder = u128::from(carry.remainder)
            .checked_mul(u128::from(multiplier))
            .ok_or(CoreError::Overflow)?;
        (
            active_denominator,
            u64::try_from(remainder).map_err(|_| CoreError::Overflow)?,
            1,
        )
    } else {
        (
            active_denominator,
            rebase_fraction_half_even(carry.remainder, carry.denominator, active_denominator)?,
            1,
        )
    };
    if population >= carrying_capacity {
        *carry = LogisticGrowthCarry {
            remainder: rebased_remainder,
            denominator,
        };
        return Ok(0);
    }
    let numerator = u128::from(population)
        .checked_mul(u128::from(carrying_capacity - population))
        .and_then(|value| value.checked_mul(u128::from(rate_per_thousand)))
        .and_then(|value| value.checked_mul(u128::from(growth_multiplier)))
        .and_then(|value| value.checked_add(u128::from(rebased_remainder)))
        .ok_or(CoreError::Overflow)?;
    let next_carry = LogisticGrowthCarry {
        remainder: u64::try_from(numerator % u128::from(denominator))
            .map_err(|_| CoreError::Overflow)?,
        denominator,
    };
    let delta = u64::try_from(numerator / u128::from(denominator))
        .map_err(|_| CoreError::Overflow)?
        .min(carrying_capacity - population);
    *carry = next_carry;
    Ok(delta)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CostBasis {
    pub stock_quantity: u64,
    pub total_embodied_energy: Energy,
}

impl CostBasis {
    pub fn unit_cost_ceil(self) -> Result<Energy, CoreError> {
        if self.stock_quantity == 0 {
            return Ok(Energy(0));
        }
        checked_ceil_div(self.total_embodied_energy.0, self.stock_quantity)
    }

    pub fn add(&mut self, quantity: u64, embodied: Energy) -> Result<(), CoreError> {
        let next_quantity = self
            .stock_quantity
            .checked_add(quantity)
            .ok_or(CoreError::Overflow)?;
        let next_total = self.total_embodied_energy.checked_add(embodied)?;
        self.stock_quantity = next_quantity;
        self.total_embodied_energy = next_total;
        Ok(())
    }

    /// Removes average cost using floor division; the final unit receives the remainder.
    pub fn removal_cost(self, quantity: u64) -> Result<Energy, CoreError> {
        if quantity > self.stock_quantity {
            return Err(CoreError::InsufficientStock);
        }
        if quantity == self.stock_quantity {
            return Ok(self.total_embodied_energy);
        }
        let quantity = i64::try_from(quantity).map_err(|_| CoreError::Overflow)?;
        let stock = i64::try_from(self.stock_quantity).map_err(|_| CoreError::Overflow)?;
        self.total_embodied_energy
            .0
            .checked_mul(quantity)
            .map(|v| Energy(v / stock))
            .ok_or(CoreError::Overflow)
    }

    pub fn remove(&mut self, quantity: u64) -> Result<Energy, CoreError> {
        let cost = self.removal_cost(quantity)?;
        let next_quantity = self.stock_quantity - quantity;
        let next_total = self.total_embodied_energy.checked_sub(cost)?;
        self.stock_quantity = next_quantity;
        self.total_embodied_energy = next_total;
        Ok(cost)
    }
}

#[derive(Component, Clone, Debug)]
pub struct StableId(pub ContentId);
#[derive(Component, Clone, Debug)]
pub struct DisplayName(pub String);
#[derive(Component, Clone, Copy, Debug)]
pub struct SystemMarker;
#[derive(Component, Clone, Copy, Debug)]
pub struct SpatialPosition(pub Position3);
#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerControlled;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MarketLedger {
    pub energy_paid_to_traders: Energy,
    pub energy_received_from_traders: Energy,
    pub units_bought_from_traders: u64,
    pub units_sold_to_traders: u64,
    pub source_units_generated: u64,
    pub recipe_input_units_consumed: u64,
    pub recipe_output_units_produced: u64,
    pub processor_input_cost: Energy,
    pub processor_operating_energy: Energy,
    pub processor_output_revenue: Energy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnergyFlowLedger {
    pub external_inflow: Energy,
    pub generated: Energy,
    pub life_support_burned: Energy,
    pub life_support_unsupplied: Energy,
    pub source_burned: Energy,
    pub production_burned: Energy,
    pub investment_burned: Energy,
    pub travel_burned: Energy,
    pub curtailed: Energy,
    pub market_to_tank: Energy,
    pub tank_to_market: Energy,
    pub contract_source_loaded: Energy,
    pub contract_destination_delivered: Energy,
    pub contract_allocation_converted: Energy,
    pub owned_bulk_deposited: Energy,
    pub contract_recovery_returned: Energy,
    pub contract_recovery_curtailed: Energy,
}

impl EnergyFlowLedger {
    pub fn validate_contract_channels(self) -> Result<(), CoreError> {
        let delivered_to_markets = self
            .contract_destination_delivered
            .checked_add(self.owned_bulk_deposited)?
            .checked_add(self.contract_recovery_returned)?;
        if delivered_to_markets.0 < 0
            || self.contract_source_loaded.0 < 0
            || self.contract_destination_delivered.0 < 0
            || self.contract_allocation_converted.0 < 0
            || self.owned_bulk_deposited.0 < 0
            || self.contract_recovery_returned.0 < 0
            || self.contract_recovery_curtailed.0 < 0
            || self.contract_recovery_curtailed > self.curtailed
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        Ok(())
    }

    pub fn net_external_delta(self) -> Result<Energy, CoreError> {
        self.external_inflow
            .checked_add(self.generated)?
            .checked_sub(self.life_support_burned)?
            .checked_sub(self.source_burned)?
            .checked_sub(self.production_burned)?
            .checked_sub(self.investment_burned)?
            .checked_sub(self.travel_burned)?
            .checked_sub(self.curtailed)
    }
}

/// Wider exact aggregate used for cross-market reporting. Per-market ledgers
/// remain checked `Energy`; aggregation must never clamp to a plausible value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlobalEnergyFlowLedger {
    pub external_inflow: WideEnergy,
    pub generated: WideEnergy,
    pub life_support_burned: WideEnergy,
    pub life_support_unsupplied: WideEnergy,
    pub source_burned: WideEnergy,
    pub production_burned: WideEnergy,
    pub investment_burned: WideEnergy,
    pub travel_burned: WideEnergy,
    pub curtailed: WideEnergy,
    pub market_to_tank: WideEnergy,
    pub tank_to_market: WideEnergy,
    pub contract_source_loaded: WideEnergy,
    pub contract_destination_delivered: WideEnergy,
    pub contract_allocation_converted: WideEnergy,
    pub owned_bulk_deposited: WideEnergy,
    pub contract_recovery_returned: WideEnergy,
    pub contract_recovery_curtailed: WideEnergy,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct WideEnergy(pub WideAmount);

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct WideAmount(i128);

impl From<WideAmount> for i128 {
    fn from(value: WideAmount) -> Self {
        value.0
    }
}

impl GlobalEnergyFlowLedger {
    #[must_use]
    pub fn net_external_delta(self) -> WideEnergy {
        WideEnergy(WideAmount(
            i128::from(self.external_inflow.0) + i128::from(self.generated.0)
                - i128::from(self.life_support_burned.0)
                - i128::from(self.source_burned.0)
                - i128::from(self.production_burned.0)
                - i128::from(self.investment_burned.0)
                - i128::from(self.travel_burned.0)
                - i128::from(self.curtailed.0),
        ))
    }

    fn add_market(&mut self, flow: EnergyFlowLedger) {
        macro_rules! add {
            ($field:ident) => {
                self.$field.0.0 += i128::from(flow.$field.0);
            };
        }
        add!(external_inflow);
        add!(generated);
        add!(life_support_burned);
        add!(life_support_unsupplied);
        add!(source_burned);
        add!(production_burned);
        add!(investment_burned);
        add!(travel_burned);
        add!(curtailed);
        add!(market_to_tank);
        add!(tank_to_market);
        add!(contract_source_loaded);
        add!(contract_destination_delivered);
        add!(contract_allocation_converted);
        add!(owned_bulk_deposited);
        add!(contract_recovery_returned);
        add!(contract_recovery_curtailed);
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ThroughputScheduleKey {
    Source(ContentId),
    Recipe(ContentId),
}

#[derive(Component, Clone, Debug)]
pub struct Market {
    pub inventory: BTreeMap<ContentId, u64>,
    /// Effective demand targets for the current population.
    pub targets: BTreeMap<ContentId, u32>,
    /// Immutable authored targets used when population-mapped demand is
    /// recomputed for the next tick.
    pub authored_targets: BTreeMap<ContentId, u32>,
    pub recipes: Vec<ContentId>,
    pub sources: Vec<SourceDefinition>,
    pub cost_basis: BTreeMap<ContentId, CostBasis>,
    pub energy_output_per_tick: Energy,
    pub seasonal_generation: SeasonalGenerationState,
    pub energy_storage_cap: Energy,
    pub population: u64,
    pub population_state: PopulationState,
    pub brownout: BrownoutState,
    pub operating_profile: MarketOperatingProfile,
    pub investment_policy: InvestmentPolicy,
    pub investment_state: InvestmentState,
    pub governance: Governance,
    pub energy_logistics: EnergyLogisticsPolicy,
    pub throughput_carry: BTreeMap<ThroughputScheduleKey, u64>,
    pub source_output_percent: u32,
    pub recipe_operating_energy: BTreeMap<ContentId, Energy>,
    pub reserved_energy: Energy,
    pub protected_liquidation_budget: Energy,
    pub bootstrap_risk_acknowledged: bool,
    pub ledger: MarketLedger,
    pub energy_flow: EnergyFlowLedger,
    pub last_life_support_unsupplied: Energy,
}

impl Market {
    pub fn energy_stock(&self) -> Result<Energy, CoreError> {
        let id = ContentId::new(ENERGY_ID).expect("constant id");
        let value = self.inventory.get(&id).copied().unwrap_or(0);
        Ok(Energy(
            i64::try_from(value).map_err(|_| CoreError::Overflow)?,
        ))
    }

    fn set_energy_stock(&mut self, value: Energy) -> Result<(), CoreError> {
        if value.0 < 0 {
            return Err(CoreError::InsufficientEnergy);
        }
        let value = u64::try_from(value.0).map_err(|_| CoreError::Overflow)?;
        self.inventory
            .insert(ContentId::new(ENERGY_ID).expect("constant id"), value);
        let basis = self
            .cost_basis
            .entry(ContentId::new(ENERGY_ID).expect("constant id"))
            .or_default();
        basis.stock_quantity = value;
        basis.total_embodied_energy =
            Energy(i64::try_from(value).map_err(|_| CoreError::Overflow)?);
        Ok(())
    }

    pub fn operating_reserve(
        &self,
        policy: &MarketPolicy,
        life_per_capita: Energy,
    ) -> Result<Energy, CoreError> {
        let life = life_per_capita.checked_mul(self.population)?;
        let mut carry = self.throughput_carry.clone();
        let mut reserve = Energy::ZERO;
        for _ in 0..policy.operating_reserve_ticks {
            reserve = reserve.checked_add(life)?;
            for source in &self.sources {
                let base_output =
                    scaled_source_output(source.quantity_per_tick, self.source_output_percent)?;
                let output = composed_throughput(
                    u64::from(base_output),
                    self.operating_profile.throughput_percent,
                    self.operating_profile.labor_percent,
                    carry
                        .entry(ThroughputScheduleKey::Source(source.good.clone()))
                        .or_insert(0),
                )?;
                reserve = reserve.checked_add(source.extraction_energy.checked_mul(output)?)?;
            }
            for (recipe, operating_energy) in &self.recipe_operating_energy {
                let executions = composed_throughput(
                    1,
                    self.operating_profile.throughput_percent,
                    self.operating_profile.labor_percent,
                    carry
                        .entry(ThroughputScheduleKey::Recipe(recipe.clone()))
                        .or_insert(0),
                )?;
                reserve = reserve.checked_add(operating_energy.checked_mul(executions)?)?;
            }
        }
        Ok(reserve)
    }

    pub fn protected_discretionary_energy(&self) -> Result<Energy, CoreError> {
        let stock = self.energy_stock()?.0;
        let protected = self
            .reserved_energy
            .checked_add(self.protected_liquidation_budget)?
            .0;
        Ok(Energy(stock.saturating_sub(protected).max(0)))
    }

    fn purchasing_protection(
        &self,
        policy: &MarketPolicy,
        life_per_capita: Energy,
        released_claim: Energy,
    ) -> Result<(Energy, Energy, Energy), CoreError> {
        Ok((
            self.reserved_energy.checked_sub(released_claim)?,
            self.operating_reserve(policy, life_per_capita)?,
            self.protected_liquidation_budget,
        ))
    }

    pub fn funded_quantity_for_purchases(
        &self,
        policy: &MarketPolicy,
        life_per_capita: Energy,
        requested: u32,
        bid: Energy,
        released_claim: Energy,
    ) -> Result<u32, CoreError> {
        let (claims, operating, liquidation) =
            self.purchasing_protection(policy, life_per_capita, released_claim)?;
        funded_quantity(
            requested,
            self.energy_stock()?,
            claims,
            operating,
            liquidation,
            bid,
        )
    }

    pub fn unreserved_energy_for_purchases(
        &self,
        policy: &MarketPolicy,
        life_per_capita: Energy,
    ) -> Result<Energy, CoreError> {
        let (claims, operating, liquidation) =
            self.purchasing_protection(policy, life_per_capita, Energy::ZERO)?;
        let protected = claims.checked_add(operating)?.checked_add(liquidation)?;
        Ok(Energy(
            self.energy_stock()?.0.saturating_sub(protected.0).max(0),
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TravelPlan {
    pub destination: ContentId,
    pub route: Vec<ContentId>,
    pub next_leg: usize,
    pub remaining_ticks: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TradeLedger {
    pub purchase_cost: Energy,
    pub sales_revenue: Energy,
    /// Contract allocation converted solely to reimburse loaded/recovery burn.
    pub contract_reimbursement: Energy,
    pub travel_cost: Energy,
    /// Travel cost covered by contract reimbursement.
    pub reimbursed_travel_cost: Energy,
    pub cargo_units_moved: u64,
    pub completed_transactions: u64,
}

impl TradeLedger {
    fn validate_contract_subsets(&self) -> Result<(), CoreError> {
        if self.sales_revenue.0 < 0
            || self.contract_reimbursement.0 < 0
            || self.contract_reimbursement > self.sales_revenue
            || self.travel_cost.0 < 0
            || self.reimbursed_travel_cost.0 < 0
            || self.reimbursed_travel_cost > self.travel_cost
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraderRetirementState {
    Active,
    CleaningUp,
}

#[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
struct TraderLifecycle {
    profitability: Vec<i64>,
    observed_purchase_cost: Energy,
    observed_sales_revenue: Energy,
    observed_contract_reimbursement: Energy,
    observed_travel_cost: Energy,
    observed_reimbursed_travel_cost: Energy,
    failed_liquidation_ticks: u32,
    last_failed_tick: Option<u64>,
    retirement: Option<TraderRetirementState>,
}

/// Mutable player-only progression state for networked trade capabilities.
#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerTradeNetworkAccess {
    pub access: TradeNetworkAccess,
}

#[derive(Component, Clone, Debug)]
pub struct Trader {
    pub system: ContentId,
    pub archetype: Option<ContentId>,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
    pub bulk_energy_capacity: Energy,
    pub bulk_energy: BulkEnergyHold,
    pub cargo: BTreeMap<ContentId, u64>,
    pub cargo_cost_basis: BTreeMap<ContentId, CostBasis>,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel_burn_per_distance: Energy,
    pub refuel_policy: RefuelPolicy,
    pub travel: Option<TravelPlan>,
    pub reservation: Option<u64>,
    pub ledger: TradeLedger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationStatus {
    Active,
    Fulfilled,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeReservation {
    pub id: u64,
    pub trader: ContentId,
    pub destination: ContentId,
    pub good: ContentId,
    pub quantity: u32,
    pub remaining_quantity: u32,
    pub reserved_energy: Energy,
    pub floor_unit_price: Energy,
    pub expires_at_tick: u64,
    pub status: ReservationStatus,
}

#[derive(Clone, Copy, Debug)]
struct SaleTerms {
    unit_price: Energy,
    reserved_release: Energy,
    partial: bool,
}

#[derive(Clone, Copy, Debug)]
struct FundingProtection {
    released_ordinary_claim: Energy,
    protect_liquidation_budget: bool,
}

#[derive(Resource, Clone, Debug, Default)]
struct Reservations {
    next_id: u64,
    entries: BTreeMap<u64, TradeReservation>,
}

#[derive(Clone, Debug)]
struct PreparedTradeCommitment {
    trader_entity: Entity,
    origin_entity: Entity,
    destination_entity: Entity,
    trader: Trader,
    origin_market: Market,
    destination_market: Market,
    reservations: Reservations,
    events: Vec<GameEvent>,
}

#[derive(Clone, Debug)]
struct PendingTradeRequest {
    score: i128,
    trader_id: ContentId,
    trader: Entity,
    destination: ContentId,
    good: ContentId,
    quantity: u32,
    buy_at_origin: bool,
    command_driven: bool,
}

#[derive(Resource, Clone, Debug, Default)]
struct PendingTradeRequests(Vec<PendingTradeRequest>);

#[derive(Clone, Debug)]
struct OrdinaryNpcOpportunity {
    score: i128,
    source: ContentId,
    destination: ContentId,
    good: ContentId,
    quantity: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DynamicOpportunityKind {
    EnergyContract,
    OrdinaryTrade,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DynamicOpportunityKey {
    kind: DynamicOpportunityKind,
    source: ContentId,
    destination: ContentId,
    good: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DynamicSpawnCandidate {
    score: i128,
    opportunity: DynamicOpportunityKey,
    archetype: ContentId,
}

impl DynamicSpawnCandidate {
    fn selection_order(left: &Self, right: &Self) -> Ordering {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.opportunity.kind.cmp(&right.opportunity.kind))
            .then_with(|| left.opportunity.source.cmp(&right.opportunity.source))
            .then_with(|| {
                left.opportunity
                    .destination
                    .cmp(&right.opportunity.destination)
            })
            .then_with(|| left.opportunity.good.cmp(&right.opportunity.good))
            .then_with(|| left.archetype.cmp(&right.archetype))
    }

    fn is_better_than(&self, other: &Self) -> bool {
        Self::selection_order(self, other) == Ordering::Less
    }
}

fn retain_best_dynamic_candidate(
    candidates: &mut BTreeMap<DynamicOpportunityKey, DynamicSpawnCandidate>,
    candidate: DynamicSpawnCandidate,
) {
    match candidates.get_mut(&candidate.opportunity) {
        Some(current) if candidate.is_better_than(current) => *current = candidate,
        Some(_) => {}
        None => {
            candidates.insert(candidate.opportunity.clone(), candidate);
        }
    }
}

/// Phase-10-only spawn choice. It is deliberately absent from snapshots and
/// revalidated rather than replaced during phase 13.
#[derive(Resource, Clone, Debug, Default)]
struct DynamicFleetOpportunityState {
    captured_tick: Option<u64>,
    candidate: Option<DynamicSpawnCandidate>,
}

#[derive(Resource, Clone, Debug)]
pub struct Catalog {
    pub goods: BTreeMap<ContentId, GoodDefinition>,
    pub recipes: BTreeMap<ContentId, RecipeDefinition>,
}

#[derive(Resource, Clone, Debug)]
pub struct SystemGraph {
    positions: BTreeMap<ContentId, Position3>,
    edges: BTreeMap<ContentId, Vec<(ContentId, f64)>>,
}

impl SystemGraph {
    pub fn build(systems: &[SystemDefinition]) -> Result<Self, CoreError> {
        if systems.is_empty() {
            return Err(CoreError::EmptyGraph);
        }
        let positions: BTreeMap<_, _> =
            systems.iter().map(|s| (s.id.clone(), s.position)).collect();
        let mut undirected = BTreeSet::new();
        for system in systems {
            let mut neighbors: Vec<_> = systems
                .iter()
                .filter(|o| o.id != system.id)
                .map(|o| (system.position.distance(o.position), o.id.clone()))
                .collect();
            neighbors.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            for (_, neighbor) in neighbors.into_iter().take(3) {
                undirected.insert(if system.id < neighbor {
                    (system.id.clone(), neighbor)
                } else {
                    (neighbor, system.id.clone())
                });
            }
        }
        let mut edges: BTreeMap<_, Vec<_>> = positions
            .keys()
            .cloned()
            .map(|id| (id, Vec::new()))
            .collect();
        for (a, b) in undirected {
            let d = positions[&a].distance(positions[&b]);
            edges.get_mut(&a).unwrap().push((b.clone(), d));
            edges.get_mut(&b).unwrap().push((a, d));
        }
        for values in edges.values_mut() {
            values.sort_by(|a, b| a.0.cmp(&b.0));
        }
        let graph = Self { positions, edges };
        if graph.reachable_count(systems[0].id.clone()) != systems.len() {
            return Err(CoreError::DisconnectedGraph);
        }
        Ok(graph)
    }
    fn reachable_count(&self, start: ContentId) -> usize {
        let mut seen = BTreeSet::from([start.clone()]);
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            for (next, _) in self.neighbors(&n) {
                if seen.insert(next.clone()) {
                    stack.push(next.clone())
                }
            }
        }
        seen.len()
    }
    #[must_use]
    pub fn neighbors(&self, id: &ContentId) -> &[(ContentId, f64)] {
        self.edges.get(id).map_or(&[], Vec::as_slice)
    }
    #[must_use]
    pub fn position(&self, id: &ContentId) -> Option<Position3> {
        self.positions.get(id).copied()
    }
    pub fn shortest_path(
        &self,
        start: &ContentId,
        goal: &ContentId,
    ) -> Option<(Vec<ContentId>, f64)> {
        if start == goal {
            return Some((vec![start.clone()], 0.0));
        }
        #[derive(Clone)]
        struct State {
            cost: f64,
            id: ContentId,
        }
        impl Eq for State {}
        impl PartialEq for State {
            fn eq(&self, o: &Self) -> bool {
                self.cost.total_cmp(&o.cost) == Ordering::Equal && self.id == o.id
            }
        }
        impl Ord for State {
            fn cmp(&self, o: &Self) -> Ordering {
                o.cost
                    .total_cmp(&self.cost)
                    .then_with(|| o.id.cmp(&self.id))
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        let mut distances = BTreeMap::from([(start.clone(), 0.0)]);
        let mut previous = BTreeMap::<ContentId, ContentId>::new();
        let mut heap = BinaryHeap::from([State {
            cost: 0.0,
            id: start.clone(),
        }]);
        while let Some(State { cost, id }) = heap.pop() {
            if id == *goal {
                let mut path = vec![goal.clone()];
                let mut cursor = goal;
                while let Some(parent) = previous.get(cursor) {
                    path.push(parent.clone());
                    cursor = parent;
                }
                path.reverse();
                return Some((path, cost));
            }
            if cost > *distances.get(&id).unwrap_or(&f64::INFINITY) {
                continue;
            }
            for (next, edge) in self.neighbors(&id) {
                let candidate = cost + edge;
                let current = distances.get(next).copied().unwrap_or(f64::INFINITY);
                if candidate < current
                    || (candidate.total_cmp(&current) == Ordering::Equal
                        && previous.get(next).is_none_or(|old| id < *old))
                {
                    distances.insert(next.clone(), candidate);
                    previous.insert(next.clone(), id.clone());
                    heap.push(State {
                        cost: candidate,
                        id: next.clone(),
                    });
                }
            }
        }
        None
    }
    #[must_use]
    pub fn route_distance(&self, route: &[ContentId]) -> f64 {
        route
            .windows(2)
            .filter_map(|p| {
                self.neighbors(&p[0])
                    .iter()
                    .find(|(id, _)| id == &p[1])
                    .map(|(_, d)| *d)
            })
            .sum()
    }
}

#[derive(Resource, Default)]
struct EventBuffer(Vec<GameEvent>);
#[derive(Resource, Default)]
struct Clock(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GovernorRejectionReason {
    Unauthorized,
    InvalidPolicy,
    InvalidInvestmentAllocation,
    UnknownMarket,
    UnknownGood,
    Arithmetic,
}

impl GovernorRejectionReason {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unauthorized => "not authorized for this market",
            Self::InvalidPolicy => "invalid market policy",
            Self::InvalidInvestmentAllocation => "invalid investment allocation",
            Self::UnknownMarket => "unknown market",
            Self::UnknownGood => "unknown good",
            Self::Arithmetic => "policy arithmetic failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    TickAdvanced(u64),
    EnergyLogistics(EnergyContractEvent),
    EnergyGenerated {
        system: ContentId,
        amount: Energy,
        curtailed: Energy,
    },
    LifeSupport {
        system: ContentId,
        burned: Energy,
        unsupplied: Energy,
    },
    ExternalDeliveryRecorded {
        system: ContentId,
        good: ContentId,
        quantity: u64,
        energy_inflow: Energy,
        tick: u64,
    },
    BrownoutTransition {
        system: ContentId,
        from: BrownoutStage,
        to: BrownoutStage,
        ticks_of_burn: u32,
        tick: u64,
    },
    PopulationChanged {
        system: ContentId,
        from: u64,
        to: u64,
    },
    PopulationTierChanged {
        system: ContentId,
        from: usize,
        to: usize,
        population: u64,
    },
    TraderSpawned {
        trader: ContentId,
        system: ContentId,
    },
    TraderRetired {
        trader: ContentId,
        system: ContentId,
    },
    InvestmentCompleted {
        system: ContentId,
        kind: InvestmentKind,
        level: u32,
        cost: Energy,
    },
    InvestmentDeferred {
        system: ContentId,
        kind: InvestmentKind,
        reason: String,
    },
    GovernorPolicyRejected {
        system: ContentId,
        reason: GovernorRejectionReason,
    },
    Produced {
        system: ContentId,
        recipe: ContentId,
    },
    Bought {
        trader: ContentId,
        good: ContentId,
        quantity: u32,
        total: Energy,
    },
    Sold {
        trader: ContentId,
        good: ContentId,
        quantity: u32,
        total: Energy,
        partial: bool,
    },
    ReservationCreated {
        reservation: u64,
        trader: ContentId,
        destination: ContentId,
        good: ContentId,
        quantity: u32,
        reserved_energy: Energy,
    },
    ReservationReleased {
        reservation: u64,
        status: ReservationStatus,
        released_energy: Energy,
    },
    SaleDeferred {
        trader: ContentId,
        good: ContentId,
        reason: String,
    },
    Departed {
        trader: ContentId,
        destination: ContentId,
        travel_burn: Energy,
    },
    Arrived {
        trader: ContentId,
        system: ContentId,
    },
    PolicyChanged {
        system: ContentId,
    },
    MarketTargetChanged {
        system: ContentId,
        good: ContentId,
        target: u32,
    },
    Rejected(String),
}

#[derive(Clone, Debug)]
pub enum GameCommand {
    Buy {
        good: ContentId,
        quantity: u32,
    },
    Sell {
        good: ContentId,
        quantity: u32,
    },
    BeginTravel {
        destination: ContentId,
    },
    CommitTrade {
        origin: ContentId,
        destination: ContentId,
        good: ContentId,
        quantity: u32,
    },
    DepositTank {
        amount: Energy,
    },
    WithdrawTank {
        amount: Energy,
    },
    TransferOwnedBulkToTank {
        amount: Energy,
    },
    DepositOwnedBulkEnergy {
        amount: Energy,
    },
    AcceptEnergyContract {
        source: ContentId,
        destination: ContentId,
        gross_payload: Energy,
    },
    CancelEnergyContract {
        contract_id: ContractId,
    },
    SetMarketPolicy {
        system: ContentId,
        policy: MarketPolicy,
    },
    SetInvestmentPolicy {
        system: ContentId,
        policy: InvestmentPolicy,
    },
    SetGovernorMarketPolicy {
        system: ContentId,
        policy: GovernorMarketPolicy,
    },
    SetGovernorInvestmentPolicy {
        system: ContentId,
        policy: GovernorInvestmentPolicy,
    },
    SetGovernorMarketTarget {
        system: ContentId,
        good: ContentId,
        target: u32,
    },
    /// A core-owned, auditable boundary for diagnostics and future adapters.
    /// This is deliberately not exposed through the player application API.
    RecordExternalDelivery {
        system: ContentId,
        good: ContentId,
        quantity: u64,
    },
    CancelReservation,
}

#[derive(Error, Debug, Clone, Eq, PartialEq)]
pub enum CoreError {
    #[error("invalid content id: {0}")]
    InvalidId(String),
    #[error("graph has no systems")]
    EmptyGraph,
    #[error("system graph is disconnected")]
    DisconnectedGraph,
    #[error("unknown {kind}: {id}")]
    Unknown { kind: &'static str, id: String },
    #[error("quantity must be positive")]
    ZeroQuantity,
    #[error("trader is in transit")]
    InTransit,
    #[error("trader is not located at this market")]
    WrongLocation,
    #[error("insufficient stock")]
    InsufficientStock,
    #[error("insufficient energy")]
    InsufficientEnergy,
    #[error("insufficient cargo capacity")]
    InsufficientCapacity,
    #[error("insufficient tank capacity")]
    InsufficientTankCapacity,
    #[error("destination is current system")]
    AlreadyThere,
    #[error("no route to destination")]
    NoRoute,
    #[error("arithmetic overflow")]
    Overflow,
    #[error("definition must contain exactly one player")]
    InvalidPlayerCount,
    #[error("core:energy definition missing or invalid")]
    InvalidEnergyGood,
    #[error("core:energy is not available through ordinary trade")]
    EnergyNotTradable,
    #[error("locked contract energy cannot be moved or spent")]
    LockedEnergy,
    #[error("an energy contract request is already pending")]
    PendingEnergyContractIntent,
    #[error("an active energy contract blocks this action")]
    ActiveEnergyContract,
    #[error("invalid market policy")]
    InvalidPolicy,
    #[error("player is not authorized to govern this market")]
    UnauthorizedMarketPolicy,
    #[error("market target must be greater than zero")]
    InvalidMarketTarget,
    #[error("invalid investment allocation")]
    InvalidInvestmentPolicy,
    #[error("invalid physical definition")]
    InvalidPhysicalDefinition,
    #[error("refuel policy forbids this transfer")]
    RefuelForbidden,
    #[error("requested quantity {requested} exceeds current maximum {maximum}")]
    ExactQuantityUnavailable { requested: u32, maximum: u32 },
    #[error("no funded quantity available")]
    Unfunded,
    #[error("reservation not found")]
    ReservationNotFound,
    #[error("player trade-network access does not permit reservation contracts")]
    TradeNetworkAccessDenied,
    #[error("invalid world-dynamics configuration")]
    InvalidWorldDynamics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MarketDemandSnapshot {
    pub advertised: u32,
    pub funded: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarketSnapshot {
    pub system_id: ContentId,
    pub name: String,
    pub position: Position3,
    pub inventory: BTreeMap<ContentId, u64>,
    pub targets: BTreeMap<ContentId, u32>,
    pub authored_targets: BTreeMap<ContentId, u32>,
    pub recipes: Vec<ContentId>,
    pub sources: Vec<SourceDefinition>,
    pub energy_stock: Energy,
    pub energy_storage_cap: Energy,
    pub reserved_energy: Energy,
    pub operating_reserve: Energy,
    pub protected_liquidation_budget: Energy,
    pub unreserved_energy_for_purchases: Energy,
    pub demand: BTreeMap<ContentId, MarketDemandSnapshot>,
    pub population: u64,
    pub brownout: BrownoutState,
    pub operating_profile: MarketOperatingProfile,
    pub seasonal_generation: SeasonalGenerationState,
    pub seasonal_phase: SeasonalPhaseSnapshot,
    pub population_state: PopulationState,
    pub investment_policy: InvestmentPolicy,
    pub investment_state: InvestmentState,
    pub governance: Governance,
    pub bootstrap_risk_acknowledged: bool,
    pub policy: MarketPolicy,
    pub energy_logistics: EnergyLogisticsPolicy,
    pub cost_basis: BTreeMap<ContentId, CostBasis>,
    pub ledger: MarketLedger,
    pub energy_flow: EnergyFlowLedger,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TraderSnapshot {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub archetype: Option<ContentId>,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
    pub bulk_energy_capacity: Energy,
    pub bulk_energy: BulkEnergyHold,
    pub cargo: BTreeMap<ContentId, u64>,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel_burn_per_distance: Energy,
    pub refuel_policy: RefuelPolicy,
    pub travel: Option<TravelPlan>,
    pub reservation: Option<u64>,
    pub ledger: TradeLedger,
    pub profitability_window: Vec<i64>,
    pub retirement: Option<TraderRetirementState>,
    pub failed_liquidation_ticks: u32,
    pub player: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CoreSnapshot {
    pub tick: u64,
    pub markets: Vec<MarketSnapshot>,
    pub energy_markets: Vec<EnergyMarketLogisticsSnapshot>,
    pub energy_opportunities: Vec<EnergyContractOpportunitySnapshot>,
    pub energy_contracts: Vec<EnergyContractSnapshot>,
    pub energy_logistics: EnergyLogisticsDiagnostics,
    pub energy_starvation: BTreeMap<ContentId, EnergyStarvationCause>,
    pub investment_shapes: BTreeMap<InvestmentKind, InvestmentShape>,
    pub player_trade_network_access: TradeNetworkAccess,
    pub traders: Vec<TraderSnapshot>,
    pub reservations: Vec<TradeReservation>,
    pub energy_flow: GlobalEnergyFlowLedger,
    pub dynamics_history: AggregateDynamicsHistory,
    pub fleet: FleetDynamics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessorSolvency {
    pub system: ContentId,
    pub recipe: ContentId,
    pub expected_input_bids: Energy,
    pub operating_energy: Energy,
    pub expected_output_asks: Energy,
    pub required_margin_percent: u32,
    pub solvent: bool,
}

pub fn checked_ceil_div(value: i64, divisor: u64) -> Result<Energy, CoreError> {
    if value < 0 || divisor == 0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let d = i64::try_from(divisor).map_err(|_| CoreError::Overflow)?;
    let adjusted = value.checked_add(d - 1).ok_or(CoreError::Overflow)?;
    Ok(Energy(adjusted / d))
}

pub fn checked_mul_ratio_ceil(
    value: Energy,
    numerator: u64,
    denominator: u64,
) -> Result<Energy, CoreError> {
    if value.0 < 0 || denominator == 0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let product = i128::from(value.0)
        .checked_mul(i128::from(numerator))
        .ok_or(CoreError::Overflow)?;
    let denominator = i128::from(denominator);
    let rounded = product
        .checked_add(denominator - 1)
        .ok_or(CoreError::Overflow)?
        / denominator;
    Ok(Energy(
        i64::try_from(rounded).map_err(|_| CoreError::Overflow)?,
    ))
}
pub fn funded_quantity(
    requested: u32,
    stock: Energy,
    reserved: Energy,
    operating: Energy,
    protected: Energy,
    unit_price: Energy,
) -> Result<u32, CoreError> {
    if unit_price.0 <= 0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let available = stock
        .checked_sub(reserved)?
        .checked_sub(operating)?
        .checked_sub(protected)?
        .0
        .max(0);
    Ok(requested.min(u32::try_from(available / unit_price.0).unwrap_or(u32::MAX)))
}
pub fn scaled_source_output(quantity: u32, percent: u32) -> Result<u32, CoreError> {
    quantity
        .checked_mul(percent)
        .ok_or(CoreError::Overflow)
        .map(|value| value / 100)
}

pub fn travel_energy(distance: f64, burn_per_distance: Energy) -> Result<Energy, CoreError> {
    if !distance.is_finite() || distance < 0.0 || burn_per_distance.0 < 0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let value = distance * burn_per_distance.0 as f64;
    if !value.is_finite() || value > i64::MAX as f64 {
        return Err(CoreError::Overflow);
    }
    Ok(Energy(value.ceil() as i64))
}

pub fn route_travel_energy(
    graph: &SystemGraph,
    route: &[ContentId],
    burn_per_distance: Energy,
) -> Result<Energy, CoreError> {
    route.windows(2).try_fold(Energy::ZERO, |total, leg| {
        let distance = graph
            .neighbors(&leg[0])
            .iter()
            .find(|(id, _)| id == &leg[1])
            .map(|(_, distance)| *distance)
            .ok_or(CoreError::NoRoute)?;
        total.checked_add(travel_energy(distance, burn_per_distance)?)
    })
}

pub fn liquidation_unit_price(
    reference_price: Energy,
    liquidation_discount_percent: u32,
) -> Result<Energy, CoreError> {
    if reference_price.0 <= 0 || liquidation_discount_percent > 100 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    reference_price
        .0
        .checked_mul(i64::from(liquidation_discount_percent))
        .map(|value| Energy((value / 100).max(1)))
        .ok_or(CoreError::Overflow)
}

pub fn liquidation_target_energy(
    adjacent_jump: Energy,
    liquidation_threshold_percent: u32,
) -> Result<Energy, CoreError> {
    if adjacent_jump.0 < 0 || liquidation_threshold_percent < 100 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let numerator = adjacent_jump
        .0
        .checked_mul(i64::from(liquidation_threshold_percent))
        .and_then(|value| value.checked_add(99))
        .ok_or(CoreError::Overflow)?;
    Ok(Energy(numerator / 100))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidationTraderCapability {
    pub cargo_capacity: u32,
    pub energy_tank_capacity: Energy,
    pub travel_burn_per_distance: Energy,
}

/// Computes the anti-strand budget from the same graph, bootstrap-price, policy,
/// and trader-capability contract used by content compilation and runtime policy
/// replacement. Operating reserve is deliberately not an input.
pub fn compute_protected_liquidation_budget(
    graph: &SystemGraph,
    system: &ContentId,
    policy: &MarketPolicy,
    eligible_bootstrap_costs: &[Energy],
    trader_capabilities: &[LiquidationTraderCapability],
) -> Result<Energy, CoreError> {
    policy.validate()?;
    if eligible_bootstrap_costs.is_empty() || trader_capabilities.is_empty() {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let adjacent_distance = graph
        .neighbors(system)
        .iter()
        .map(|(_, distance)| *distance)
        .min_by(f64::total_cmp)
        .ok_or(CoreError::NoRoute)?;
    let mut budget = Energy::ZERO;
    for capability in trader_capabilities {
        let target = liquidation_target_energy(
            travel_energy(adjacent_distance, capability.travel_burn_per_distance)?,
            policy.liquidation_threshold_percent,
        )?;
        for reference in eligible_bootstrap_costs {
            let price = liquidation_unit_price(*reference, policy.liquidation_discount_percent)?;
            let quantity = target
                .0
                .checked_add(price.0.checked_sub(1).ok_or(CoreError::Overflow)?)
                .ok_or(CoreError::Overflow)?
                / price.0;
            if quantity > i64::from(capability.cargo_capacity) {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let payout = price
                .0
                .checked_mul(quantity)
                .map(Energy)
                .ok_or(CoreError::Overflow)?;
            if payout > capability.energy_tank_capacity {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            budget = budget.max(payout);
        }
    }
    Ok(budget)
}

pub fn allocate_embodied_energy(
    total: Energy,
    outputs: &[(ContentId, u32, u32)],
) -> Result<Vec<(ContentId, Energy)>, CoreError> {
    if total.0 < 0 || outputs.is_empty() || outputs.iter().any(|(_, q, w)| *q == 0 || *w == 0) {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let weight_sum = outputs.iter().try_fold(0_u64, |s, (_, _, w)| {
        s.checked_add(u64::from(*w)).ok_or(CoreError::Overflow)
    })?;
    let denominator = i64::try_from(weight_sum).map_err(|_| CoreError::Overflow)?;
    let mut ordered = outputs.to_vec();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let mut allocated = 0_i64;
    let mut result = Vec::with_capacity(ordered.len());
    for (id, _, weight) in ordered {
        let amount = total
            .0
            .checked_mul(i64::from(weight))
            .ok_or(CoreError::Overflow)?
            / denominator;
        allocated = allocated.checked_add(amount).ok_or(CoreError::Overflow)?;
        result.push((id, Energy(amount)));
    }
    let remainder = usize::try_from(total.0.checked_sub(allocated).ok_or(CoreError::Overflow)?)
        .map_err(|_| CoreError::Overflow)?;
    for (_, amount) in result.iter_mut().take(remainder) {
        *amount = amount.checked_add(Energy(1))?;
    }
    Ok(result)
}
#[must_use]
pub fn ticks_for_distance(distance: f64, speed: f64) -> u32 {
    (distance / speed.max(f64::EPSILON)).ceil().max(1.0) as u32
}

fn profitable_opportunity_score(
    bid: Energy,
    ask: Energy,
    quantity: u32,
    travel_burn: Energy,
    distance: f64,
    speed: f64,
) -> Result<Option<i128>, CoreError> {
    if quantity == 0 || bid <= ask {
        return Ok(None);
    }
    let gross = Energy(bid.0.checked_sub(ask.0).ok_or(CoreError::Overflow)?)
        .checked_mul(u64::from(quantity))?;
    if gross <= travel_burn {
        return Ok(None);
    }
    let net = gross.checked_sub(travel_burn)?;
    Ok(Some(
        i128::from(net.0)
            .checked_mul(1_000_000)
            .ok_or(CoreError::Overflow)?
            / i128::from(ticks_for_distance(distance, speed)),
    ))
}

pub struct GameSession {
    world: World,
}

impl GameSession {
    pub fn new(definition: GameDefinition) -> Result<Self, CoreError> {
        if definition.traders.iter().filter(|t| t.player).count() != 1 {
            return Err(CoreError::InvalidPlayerCount);
        }
        let energy_id = ContentId::new(ENERGY_ID).expect("constant id");
        let energy_good = definition
            .goods
            .iter()
            .find(|g| g.id == energy_id)
            .ok_or(CoreError::InvalidEnergyGood)?;
        if energy_good.category != GoodCategory::Energy || energy_good.bootstrap_cost != Energy(1) {
            return Err(CoreError::InvalidEnergyGood);
        }
        definition
            .economy
            .life_support_burn_per_capita
            .checked_mul(1)?;
        definition.economy.brownouts.validate()?;
        definition.economy.energy_logistics.validate()?;
        validate_population_config(&definition.economy.population)?;
        validate_investment_shapes(
            &definition.economy.investments,
            &definition.economy.population,
        )?;
        validate_fleet_definition(&definition.fleet, &definition.traders)?;
        let graph = SystemGraph::build(&definition.systems)?;
        let catalog = Catalog {
            goods: definition
                .goods
                .into_iter()
                .map(|g| (g.id.clone(), g))
                .collect(),
            recipes: definition
                .recipes
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect(),
        };
        let player_trade_network_access = definition.player_trade_network_access;
        let mut world = World::new();
        world.insert_resource(graph);
        world.insert_resource(catalog.clone());
        world.insert_resource(definition.economy);
        world.insert_resource(definition.fleet);
        world.insert_resource(AggregateDynamicsHistory::default());
        world.insert_resource(Clock::default());
        world.insert_resource(EventBuffer::default());
        world.insert_resource(Reservations::default());
        world.insert_resource(PendingTradeRequests::default());
        world.insert_resource(DynamicFleetOpportunityState::default());
        world.insert_resource(EnergyContracts::default());
        world.insert_resource(PendingEnergyContractIntents::default());
        world.insert_resource(energy_logistics::EnergyLogisticsTickCapture::default());
        for mut system in definition.systems {
            if system
                .policy
                .import_priorities
                .keys()
                .any(|good| good.as_str() == ENERGY_ID)
            {
                return Err(CoreError::InvalidWorldDynamics);
            }
            system.policy.validate()?;
            system.energy_logistics.validate()?;
            system.investment_policy.validate()?;
            // Additive Slice-2 defaults keep older fixed-output/static-population
            // fixtures source-compatible when they adjust the legacy fields.
            if system.seasonal_generation.amplitude_percent == 0 {
                system.seasonal_generation.base_output = system.energy_output_per_tick;
                system.seasonal_generation.current_effective_output = system.energy_output_per_tick;
            }
            validate_market_investment_bounds(
                &world.resource::<EconomyConfig>().investments,
                &system.seasonal_generation,
                system.energy_storage_cap,
            )?;
            let population_config = world.resource::<EconomyConfig>().population.clone();
            if population_config.static_population {
                system.population_state.current = system.population;
                system.population_state.reference = system.population.max(1);
                system.population_state.carrying_capacity = system
                    .population_state
                    .carrying_capacity
                    .max(system.population);
            }
            if system.population_state.support_capacity == 0 {
                system.population_state.support_capacity = system
                    .population_state
                    .carrying_capacity
                    .max(system.population);
            }
            system.population_state.tier =
                population_tier(system.population, &population_config.tier_thresholds);
            system.seasonal_generation.validate()?;
            let initial_effective_output = system.seasonal_generation.effective_output_at(0)?;
            system.seasonal_generation.current_effective_output = initial_effective_output;
            if system.energy_output_per_tick.0 < 0
                || system.energy_storage_cap.0 <= 0
                || system.protected_liquidation_budget.0 < 0
                || system.seasonal_generation.base_output != system.energy_output_per_tick
                || system.population_state.current != system.population
                || system.population_state.reference == 0
                || system.population_state.support_capacity < population_config.minimum_cap
                || system.population_state.support_capacity > population_config.maximum_cap
                || system.population_state.carrying_capacity
                    > system.population_state.support_capacity
                || system.population_state.sufficiency_samples.len()
                    > usize::try_from(population_config.sufficiency_window)
                        .map_err(|_| CoreError::Overflow)?
                || system
                    .population_state
                    .sufficiency_samples
                    .iter()
                    .any(|sample| *sample > 100)
                || system.population_state.sufficiency_sum
                    != system
                        .population_state
                        .sufficiency_samples
                        .iter()
                        .map(|sample| u64::from(*sample))
                        .sum::<u64>()
                || system.population_state.sufficiency_average_percent
                    != if system.population_state.sufficiency_samples.is_empty() {
                        0
                    } else {
                        u32::try_from(
                            system.population_state.sufficiency_sum
                                / u64::try_from(system.population_state.sufficiency_samples.len())
                                    .map_err(|_| CoreError::Overflow)?,
                        )
                        .map_err(|_| CoreError::Overflow)?
                    }
                || system.population_state.decline_remainder >= 1_000
                || !valid_logistic_growth_carry(
                    system.population_state.growth_carry,
                    &population_config,
                )
            {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let stock = Energy(
                i64::try_from(system.inventory.get(&energy_id).copied().unwrap_or(0))
                    .map_err(|_| CoreError::Overflow)?,
            );
            if stock > system.energy_storage_cap {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let source_percent = world.resource::<EconomyConfig>().source_output_percent;
            let source_goods = system
                .sources
                .iter()
                .map(|source| source.good.clone())
                .collect::<BTreeSet<_>>();
            let recipe_ids = system.recipes.iter().cloned().collect::<BTreeSet<_>>();
            if source_goods.len() != system.sources.len()
                || recipe_ids.len() != system.recipes.len()
            {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let recipe_operating_energy = system
                .recipes
                .iter()
                .map(|recipe_id| {
                    let recipe =
                        catalog
                            .recipes
                            .get(recipe_id)
                            .ok_or_else(|| CoreError::Unknown {
                                kind: "recipe",
                                id: recipe_id.to_string(),
                            })?;
                    Ok((recipe_id.clone(), recipe.operating_energy))
                })
                .collect::<Result<BTreeMap<_, _>, CoreError>>()?;
            let mut bases = BTreeMap::new();
            for (good, quantity) in &system.inventory {
                let unit = catalog
                    .goods
                    .get(good)
                    .ok_or_else(|| CoreError::Unknown {
                        kind: "good",
                        id: good.to_string(),
                    })?
                    .bootstrap_cost;
                bases.insert(
                    good.clone(),
                    CostBasis {
                        stock_quantity: *quantity,
                        total_embodied_energy: unit.checked_mul(*quantity)?,
                    },
                );
            }
            let authored_targets = system.targets;
            let mut effective_targets = authored_targets.clone();
            for (good, units_per_thousand) in &population_config.tertiary_demand_per_thousand {
                effective_targets.insert(
                    good.clone(),
                    population_demand_target(
                        authored_targets.get(good).copied().unwrap_or(0),
                        system.population_state.current,
                        system.population_state.reference,
                        *units_per_thousand,
                    )?,
                );
            }
            world.spawn((
                StableId(system.id),
                DisplayName(system.name),
                SystemMarker,
                SpatialPosition(system.position),
                system.policy,
                Market {
                    inventory: system.inventory,
                    authored_targets,
                    targets: effective_targets,
                    recipes: system.recipes,
                    sources: system.sources,
                    cost_basis: bases,
                    energy_output_per_tick: system.energy_output_per_tick,
                    seasonal_generation: system.seasonal_generation,
                    energy_storage_cap: system.energy_storage_cap,
                    population: system.population,
                    population_state: system.population_state,
                    brownout: BrownoutState::default(),
                    operating_profile: MarketOperatingProfile::default(),
                    investment_policy: system.investment_policy,
                    investment_state: InvestmentState::default(),
                    governance: system.governance,
                    energy_logistics: system.energy_logistics,
                    throughput_carry: BTreeMap::new(),
                    source_output_percent: source_percent,
                    recipe_operating_energy,
                    reserved_energy: Energy(0),
                    protected_liquidation_budget: system.protected_liquidation_budget,
                    bootstrap_risk_acknowledged: system.bootstrap_risk_acknowledged,
                    ledger: MarketLedger::default(),
                    energy_flow: EnergyFlowLedger::default(),
                    last_life_support_unsupplied: Energy::ZERO,
                },
            ));
        }
        for trader in definition.traders {
            if trader.energy_tank.0 < 0
                || trader.energy_tank > trader.energy_tank_capacity
                || trader.energy_tank_capacity.0 <= 0
                || trader.bulk_energy_capacity.0 < 0
                || trader.cargo_capacity == 0
                || !trader.speed.is_finite()
                || trader.speed <= 0.0
                || trader.travel_burn_per_distance.0 < 0
            {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let player = trader.player;
            let mut e = world.spawn((
                StableId(trader.id),
                DisplayName(trader.name),
                Trader {
                    system: trader.system,
                    archetype: trader.archetype,
                    energy_tank: trader.energy_tank,
                    energy_tank_capacity: trader.energy_tank_capacity,
                    bulk_energy_capacity: trader.bulk_energy_capacity,
                    bulk_energy: BulkEnergyHold::default(),
                    cargo: BTreeMap::new(),
                    cargo_cost_basis: BTreeMap::new(),
                    cargo_capacity: trader.cargo_capacity,
                    speed: trader.speed,
                    travel_burn_per_distance: trader.travel_burn_per_distance,
                    refuel_policy: trader.refuel_policy,
                    travel: None,
                    reservation: None,
                    ledger: TradeLedger::default(),
                },
            ));
            if player {
                e.insert((
                    PlayerControlled,
                    PlayerTradeNetworkAccess {
                        access: player_trade_network_access,
                    },
                ));
            } else {
                e.insert(TraderLifecycle::default());
            }
        }
        Ok(Self { world })
    }
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.world.resource::<Clock>().0
    }
    #[must_use]
    pub fn graph(&self) -> &SystemGraph {
        self.world.resource::<SystemGraph>()
    }
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        self.world.resource::<Catalog>()
    }
    pub fn shortest_path(
        &self,
        start: &ContentId,
        destination: &ContentId,
    ) -> Option<(Vec<ContentId>, f64)> {
        self.graph().shortest_path(start, destination)
    }
    fn player_entity(&mut self) -> Result<Entity, CoreError> {
        self.world
            .query_filtered::<Entity, (With<Trader>, With<PlayerControlled>)>()
            .iter(&self.world)
            .next()
            .ok_or(CoreError::InvalidPlayerCount)
    }
    fn market_entity(&mut self, id: &ContentId) -> Result<Entity, CoreError> {
        self.world
            .query_filtered::<(Entity, &StableId), With<Market>>()
            .iter(&self.world)
            .find(|(_, v)| v.0 == *id)
            .map(|v| v.0)
            .ok_or_else(|| CoreError::Unknown {
                kind: "system",
                id: id.to_string(),
            })
    }
    pub fn submit(&mut self, command: GameCommand) -> Result<(), CoreError> {
        let governor_system = match &command {
            GameCommand::SetMarketPolicy { system, .. }
            | GameCommand::SetInvestmentPolicy { system, .. }
            | GameCommand::SetGovernorMarketPolicy { system, .. }
            | GameCommand::SetGovernorInvestmentPolicy { system, .. }
            | GameCommand::SetGovernorMarketTarget { system, .. } => Some(system.clone()),
            _ => None,
        };
        let investment_command = matches!(
            &command,
            GameCommand::SetInvestmentPolicy { .. }
                | GameCommand::SetGovernorInvestmentPolicy { .. }
        );
        let result = match command {
            GameCommand::Buy { good, quantity } => {
                let e = self.player_entity()?;
                self.local_buy(e, &good, quantity)
            }
            GameCommand::Sell { good, quantity } => {
                if good.as_str() == ENERGY_ID {
                    Err(CoreError::EnergyNotTradable)
                } else {
                    let maximum = self.player_local_trade_limits(&good)?.sell.maximum;
                    if quantity > maximum {
                        Err(CoreError::ExactQuantityUnavailable {
                            requested: quantity,
                            maximum,
                        })
                    } else {
                        let e = self.player_entity()?;
                        self.local_sell(e, &good, quantity, false).map(|_| ())
                    }
                }
            }
            GameCommand::BeginTravel { destination } => {
                let e = self.player_entity()?;
                self.begin_travel(e, &destination)
            }
            GameCommand::CommitTrade {
                origin,
                destination,
                good,
                quantity,
            } => {
                if good.as_str() == ENERGY_ID {
                    Err(CoreError::EnergyNotTradable)
                } else {
                    let e = self.player_entity()?;
                    let trader = self.world.get::<Trader>(e).unwrap();
                    let access = self
                        .world
                        .get::<PlayerTradeNetworkAccess>(e)
                        .ok_or(CoreError::InvalidPlayerCount)?
                        .access;
                    let system = trader.system.clone();
                    if access != TradeNetworkAccess::ReservationContracts {
                        Err(CoreError::TradeNetworkAccessDenied)
                    } else if system != origin {
                        Err(CoreError::WrongLocation)
                    } else {
                        self.enqueue_commit_request(e, &destination, &good, quantity, true, true)
                    }
                }
            }
            GameCommand::DepositTank { amount } => {
                let e = self.player_entity()?;
                self.transfer_tank(e, amount, true)
            }
            GameCommand::WithdrawTank { amount } => {
                let e = self.player_entity()?;
                self.transfer_tank(e, amount, false)
            }
            GameCommand::TransferOwnedBulkToTank { amount } => {
                let e = self.player_entity()?;
                self.transfer_owned_bulk(e, amount, false)
            }
            GameCommand::DepositOwnedBulkEnergy { amount } => {
                let e = self.player_entity()?;
                self.transfer_owned_bulk(e, amount, true)
            }
            GameCommand::AcceptEnergyContract {
                source,
                destination,
                gross_payload,
            } => self.enqueue_player_energy_contract(source, destination, gross_payload),
            GameCommand::CancelEnergyContract { contract_id } => {
                self.cancel_player_energy_contract(contract_id)
            }
            GameCommand::SetMarketPolicy { system, policy } => {
                self.set_player_policy(&system, policy)
            }
            GameCommand::SetInvestmentPolicy { system, policy } => {
                self.set_player_investment_policy(&system, policy)
            }
            GameCommand::SetGovernorMarketPolicy { system, policy } => {
                self.set_player_governor_policy(&system, policy)
            }
            GameCommand::SetGovernorInvestmentPolicy { system, policy } => {
                self.set_player_investment_policy(&system, policy.into())
            }
            GameCommand::SetGovernorMarketTarget {
                system,
                good,
                target,
            } => self.set_player_market_target(&system, &good, target),
            GameCommand::RecordExternalDelivery {
                system,
                good,
                quantity,
            } => self.record_external_delivery(&system, &good, quantity),
            GameCommand::CancelReservation => {
                let e = self.player_entity()?;
                self.cancel_trader_reservation(e, ReservationStatus::Cancelled)
            }
        };
        if let Err(error) = &result {
            let event = governor_system.map_or_else(
                || GameEvent::Rejected(error.to_string()),
                |system| GameEvent::GovernorPolicyRejected {
                    system,
                    reason: match error {
                        CoreError::UnauthorizedMarketPolicy => {
                            GovernorRejectionReason::Unauthorized
                        }
                        CoreError::InvalidInvestmentPolicy if investment_command => {
                            GovernorRejectionReason::InvalidInvestmentAllocation
                        }
                        CoreError::Unknown { kind: "good", .. } => {
                            GovernorRejectionReason::UnknownGood
                        }
                        CoreError::Unknown { .. } => GovernorRejectionReason::UnknownMarket,
                        CoreError::Overflow => GovernorRejectionReason::Arithmetic,
                        _ => GovernorRejectionReason::InvalidPolicy,
                    },
                },
            );
            self.world.resource_mut::<EventBuffer>().0.push(event);
        }
        result
    }
    fn record_external_delivery(
        &mut self,
        system: &ContentId,
        good: &ContentId,
        quantity: u64,
    ) -> Result<(), CoreError> {
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let good_definition = self
            .world
            .resource::<Catalog>()
            .goods
            .get(good)
            .cloned()
            .ok_or_else(|| CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            })?;
        let entity = self.market_entity(system)?;
        let mut next = self.world.get::<Market>(entity).unwrap().clone();
        let energy_inflow = if good.as_str() == ENERGY_ID {
            let quantity = i64::try_from(quantity).map_err(|_| CoreError::Overflow)?;
            let stock = next.energy_stock()?.checked_add(Energy(quantity))?;
            if stock > next.energy_storage_cap {
                return Err(CoreError::InsufficientCapacity);
            }
            next.set_energy_stock(stock)?;
            next.energy_flow.external_inflow = next
                .energy_flow
                .external_inflow
                .checked_add(Energy(quantity))?;
            Energy(quantity)
        } else {
            let stock = next.inventory.get(good).copied().unwrap_or(0);
            let next_stock = stock.checked_add(quantity).ok_or(CoreError::Overflow)?;
            let embodied = good_definition.bootstrap_cost.checked_mul(quantity)?;
            let mut basis = next.cost_basis.get(good).copied().unwrap_or_default();
            basis.add(quantity, embodied)?;
            next.inventory.insert(good.clone(), next_stock);
            next.cost_basis.insert(good.clone(), basis);
            Energy::ZERO
        };
        *self.world.get_mut::<Market>(entity).unwrap() = next;
        let tick = self.tick();
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::ExternalDeliveryRecorded {
                system: system.clone(),
                good: good.clone(),
                quantity,
                energy_inflow,
                tick,
            });
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), CoreError> {
        self.advance_travel()?;
        self.mark_energy_contract_arrivals()?;
        self.refresh_enroute_reservations()?;
        self.expire_reservations()?;
        self.generate_and_life_support()?;
        self.capture_unsupplied_energy_destinations();
        self.classify_brownouts()?;
        self.execute_sources_and_recipes()?;
        self.maintain_preload_energy_contracts()?;
        self.settle_energy_contracts()?;
        self.settle_idle_laden()?;
        self.rebalance_idle_npc_tanks()?;
        self.execute_autonomous_investments()?;
        self.capture_phase10_energy_logistics()?;
        self.collect_automated_trader_requests()?;
        self.resolve_pending_energy_contract_intents()?;
        self.finalize_energy_starvation_attribution()?;
        self.resolve_pending_trade_requests()?;
        self.evaluate_dynamic_fleet()?;
        self.update_populations()?;
        self.world.resource_mut::<Clock>().0 =
            self.tick().checked_add(1).ok_or(CoreError::Overflow)?;
        let tick = self.tick();
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::TickAdvanced(tick));
        Ok(())
    }
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.world.resource_mut::<EventBuffer>().0)
    }
    pub fn quotes(
        &mut self,
        system: &ContentId,
        good: &ContentId,
    ) -> Result<(Energy, Energy), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let e = self.market_entity(system)?;
        let market = self.world.get::<Market>(e).unwrap();
        let policy = self.world.get::<MarketPolicy>(e).unwrap();
        Ok((
            self.bid_quote(market, policy, good)?,
            self.ask_quote(market, policy, good)?,
        ))
    }

    pub fn player_local_trade_limits(
        &mut self,
        good: &ContentId,
    ) -> Result<LocalTradeLimits, CoreError> {
        if good.as_str() == ENERGY_ID {
            let unavailable = LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::TradingUnavailable,
            };
            return Ok(LocalTradeLimits {
                buy: unavailable,
                sell: unavailable,
            });
        }
        let trader_entity = self.player_entity()?;
        let trader = self.world.get::<Trader>(trader_entity).unwrap();
        if trader.travel.is_some() {
            let unavailable = LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::TradingUnavailable,
            };
            return Ok(LocalTradeLimits {
                buy: unavailable,
                sell: unavailable,
            });
        }
        let system = trader.system.clone();
        let tank = trader.energy_tank;
        let tank_capacity = trader.energy_tank_capacity;
        let cargo_used = Self::cargo_used(trader)?;
        let cargo_capacity = u64::from(trader.cargo_capacity);
        let held = trader.cargo.get(good).copied().unwrap_or(0);
        let market_entity = self.market_entity(&system)?;
        let market = self.world.get::<Market>(market_entity).unwrap();
        let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
        let ask = self.ask_quote(market, policy, good)?;
        let bid = self.bid_quote(market, policy, good)?;

        let buy = if ask.0 <= 0 {
            LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::MarketQuote,
            }
        } else {
            let mut limit = LocalTradeQuantityLimit {
                maximum: u32::MAX,
                reason: LocalTradeLimitReason::QuantityType,
            };
            let mut apply = |candidate: u64, reason| {
                let candidate = u32::try_from(candidate).unwrap_or(u32::MAX);
                if candidate < limit.maximum {
                    limit = LocalTradeQuantityLimit {
                        maximum: candidate,
                        reason,
                    };
                }
            };
            apply(
                market.inventory.get(good).copied().unwrap_or(0),
                LocalTradeLimitReason::MarketStock,
            );
            apply(
                cargo_capacity.saturating_sub(cargo_used),
                LocalTradeLimitReason::CargoCapacity,
            );
            apply(
                u64::try_from(tank.0 / ask.0).unwrap_or(0),
                LocalTradeLimitReason::TankEnergy,
            );
            let headroom = market
                .energy_storage_cap
                .0
                .saturating_sub(market.energy_stock()?.0);
            apply(
                u64::try_from(headroom / ask.0).unwrap_or(0),
                LocalTradeLimitReason::MarketEnergyStorage,
            );
            limit
        };

        let sell = if bid.0 <= 0 {
            LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::MarketQuote,
            }
        } else {
            let funded = self.funded_quantity_with_preload_claims(
                &system,
                market,
                policy,
                u32::MAX,
                bid,
                FundingProtection {
                    released_ordinary_claim: Energy::ZERO,
                    protect_liquidation_budget: true,
                },
            )?;
            let mut limit = LocalTradeQuantityLimit {
                maximum: u32::MAX,
                reason: LocalTradeLimitReason::QuantityType,
            };
            let mut apply = |candidate: u64, reason| {
                let candidate = u32::try_from(candidate).unwrap_or(u32::MAX);
                if candidate < limit.maximum {
                    limit = LocalTradeQuantityLimit {
                        maximum: candidate,
                        reason,
                    };
                }
            };
            apply(held, LocalTradeLimitReason::UnitsHeld);
            apply(u64::from(funded), LocalTradeLimitReason::MarketFunding);
            let tank_headroom = tank_capacity.checked_sub(tank)?;
            apply(
                u64::try_from(tank_headroom.0 / bid.0).unwrap_or(0),
                LocalTradeLimitReason::TankCapacity,
            );
            limit
        };

        Ok(LocalTradeLimits { buy, sell })
    }

    pub fn market_demand(
        &mut self,
        system: &ContentId,
        good: &ContentId,
    ) -> Result<MarketDemandSnapshot, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let entity = self.market_entity(system)?;
        let market = self.world.get::<Market>(entity).unwrap();
        let policy = self.world.get::<MarketPolicy>(entity).unwrap();
        if !self.demand_allowed(market, good) {
            return Ok(MarketDemandSnapshot::default());
        }
        let advertised_u64 = u64::from(market.targets.get(good).copied().unwrap_or(0))
            .saturating_sub(market.inventory.get(good).copied().unwrap_or(0));
        let advertised = u32::try_from(advertised_u64).unwrap_or(u32::MAX);
        if advertised == 0 {
            return Ok(MarketDemandSnapshot::default());
        }
        let bid = self.bid_quote(market, policy, good)?;
        if bid.0 <= 0 {
            return Ok(MarketDemandSnapshot::default());
        }
        let funded = self.funded_quantity_with_preload_claims(
            system,
            market,
            policy,
            advertised,
            bid,
            FundingProtection {
                released_ordinary_claim: Energy::ZERO,
                protect_liquidation_budget: true,
            },
        )?;
        Ok(MarketDemandSnapshot { advertised, funded })
    }
    pub fn processor_solvency(&mut self) -> Result<Vec<ProcessorSolvency>, CoreError> {
        let markets = self
            .world
            .query_filtered::<(&StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, market, policy)| (id.0.clone(), market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        let recipes = self.world.resource::<Catalog>().recipes.clone();
        let mut rows = Vec::new();
        for (system, market, policy) in markets {
            for recipe_id in &market.recipes {
                let recipe = recipes.get(recipe_id).ok_or_else(|| CoreError::Unknown {
                    kind: "recipe",
                    id: recipe_id.to_string(),
                })?;
                if recipe.outputs.is_empty() {
                    continue;
                }
                let input_bids = recipe.inputs.iter().try_fold(Energy::ZERO, |sum, input| {
                    sum.checked_add(
                        self.bid_quote(&market, &policy, &input.good)?
                            .checked_mul(u64::from(input.quantity))?,
                    )
                })?;
                let output_asks = recipe
                    .outputs
                    .iter()
                    .try_fold(Energy::ZERO, |sum, output| {
                        sum.checked_add(
                            self.ask_quote(&market, &policy, &output.good)?
                                .checked_mul(u64::from(output.quantity))?,
                        )
                    })?;
                let margin = recipe
                    .margin_percent
                    .unwrap_or(policy.producer_margin_percent);
                let total_cost = input_bids.checked_add(recipe.operating_energy)?;
                let solvent = i128::from(output_asks.0)
                    .checked_mul(100)
                    .ok_or(CoreError::Overflow)?
                    >= i128::from(total_cost.0)
                        .checked_mul(i128::from(
                            100_u32.checked_add(margin).ok_or(CoreError::Overflow)?,
                        ))
                        .ok_or(CoreError::Overflow)?;
                rows.push(ProcessorSolvency {
                    system: system.clone(),
                    recipe: recipe.id.clone(),
                    expected_input_bids: input_bids,
                    operating_energy: recipe.operating_energy,
                    expected_output_asks: output_asks,
                    required_margin_percent: margin,
                    solvent,
                });
            }
        }
        rows.sort_by(|a, b| {
            a.system
                .cmp(&b.system)
                .then_with(|| a.recipe.cmp(&b.recipe))
        });
        Ok(rows)
    }

    pub fn try_snapshot(&mut self) -> Result<CoreSnapshot, CoreError> {
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let phase_tick = self.tick().saturating_sub(u64::from(self.tick() > 0));
        let mut markets = self
            .world
            .query_filtered::<(
                &StableId,
                &DisplayName,
                &SpatialPosition,
                &Market,
                &MarketPolicy,
            ), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, name, pos, m, p)| MarketSnapshot {
                system_id: id.0.clone(),
                name: name.0.clone(),
                position: pos.0,
                inventory: m.inventory.clone(),
                targets: m.targets.clone(),
                authored_targets: m.authored_targets.clone(),
                recipes: m.recipes.clone(),
                sources: m.sources.clone(),
                energy_stock: m.energy_stock().unwrap_or(Energy(0)),
                energy_storage_cap: m.energy_storage_cap,
                reserved_energy: m.reserved_energy,
                operating_reserve: m.operating_reserve(p, life).unwrap_or(Energy(i64::MAX)),
                protected_liquidation_budget: m.protected_liquidation_budget,
                unreserved_energy_for_purchases: m
                    .unreserved_energy_for_purchases(p, life)
                    .unwrap_or(Energy(0)),
                demand: BTreeMap::new(),
                population: m.population,
                brownout: m.brownout.clone(),
                operating_profile: m.operating_profile.clone(),
                seasonal_generation: m.seasonal_generation.clone(),
                seasonal_phase: m
                    .seasonal_generation
                    .phase_at(phase_tick)
                    .expect("validated seasonal phase"),
                population_state: m.population_state.clone(),
                investment_policy: m.investment_policy.clone(),
                investment_state: m.investment_state.clone(),
                governance: m.governance.clone(),
                bootstrap_risk_acknowledged: m.bootstrap_risk_acknowledged,
                policy: p.clone(),
                energy_logistics: m.energy_logistics,
                cost_basis: m.cost_basis.clone(),
                ledger: m.ledger,
                energy_flow: m.energy_flow,
            })
            .collect::<Vec<_>>();
        markets.sort_by(|a, b| a.system_id.cmp(&b.system_id));
        let goods = self
            .world
            .resource::<Catalog>()
            .goods
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for market in &mut markets {
            market.demand = goods
                .iter()
                .filter(|good| good.as_str() != ENERGY_ID)
                .map(|good| {
                    (
                        good.clone(),
                        self.market_demand(&market.system_id, good)
                            .unwrap_or_default(),
                    )
                })
                .collect();
        }
        let mut traders = self
            .world
            .query::<(
                &StableId,
                &DisplayName,
                &Trader,
                Option<&TraderLifecycle>,
                Option<&PlayerControlled>,
            )>()
            .iter(&self.world)
            .map(|(id, n, t, lifecycle, p)| TraderSnapshot {
                id: id.0.clone(),
                name: n.0.clone(),
                system: t.system.clone(),
                archetype: t.archetype.clone(),
                energy_tank: t.energy_tank,
                energy_tank_capacity: t.energy_tank_capacity,
                bulk_energy_capacity: t.bulk_energy_capacity,
                bulk_energy: t.bulk_energy,
                cargo: t.cargo.clone(),
                cargo_capacity: t.cargo_capacity,
                speed: t.speed,
                travel_burn_per_distance: t.travel_burn_per_distance,
                refuel_policy: t.refuel_policy,
                travel: t.travel.clone(),
                reservation: t.reservation,
                ledger: t.ledger.clone(),
                profitability_window: lifecycle
                    .map(|state| state.profitability.clone())
                    .unwrap_or_default(),
                retirement: lifecycle.and_then(|state| state.retirement),
                failed_liquidation_ticks: lifecycle
                    .map_or(0, |state| state.failed_liquidation_ticks),
                player: p.is_some(),
            })
            .collect::<Vec<_>>();
        traders.sort_by(|a, b| a.id.cmp(&b.id));
        let reservations = self
            .world
            .resource::<Reservations>()
            .entries
            .values()
            .cloned()
            .collect();
        let energy_flow = markets.iter().fold(
            GlobalEnergyFlowLedger::default(),
            |mut aggregate, market| {
                aggregate.add_market(market.energy_flow);
                aggregate
            },
        );
        let player_trade_network_access = self
            .world
            .query_filtered::<&PlayerTradeNetworkAccess, With<PlayerControlled>>()
            .iter(&self.world)
            .next()
            .expect("validated player access component")
            .access;
        let energy_logistics = self.energy_logistics_projection()?;
        Ok(CoreSnapshot {
            tick: self.tick(),
            markets,
            energy_markets: energy_logistics.markets,
            energy_opportunities: energy_logistics.opportunities,
            energy_contracts: energy_logistics.contracts,
            energy_logistics: energy_logistics.diagnostics,
            energy_starvation: energy_logistics.starvation,
            investment_shapes: self.world.resource::<EconomyConfig>().investments.clone(),
            player_trade_network_access,
            traders,
            reservations,
            energy_flow,
            dynamics_history: self.world.resource::<AggregateDynamicsHistory>().clone(),
            fleet: self.world.resource::<FleetDynamics>().clone(),
        })
    }

    pub fn snapshot(&mut self) -> CoreSnapshot {
        self.try_snapshot()
            .expect("validated simulation state must produce an immutable snapshot")
    }

    fn good_cost_basis(&self, market: &Market, good: &ContentId) -> Result<Energy, CoreError> {
        let bootstrap = self
            .world
            .resource::<Catalog>()
            .goods
            .get(good)
            .ok_or_else(|| CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            })?
            .bootstrap_cost;
        Ok(Energy(
            market
                .cost_basis
                .get(good)
                .copied()
                .unwrap_or_default()
                .unit_cost_ceil()?
                .0
                .max(bootstrap.0),
        ))
    }

    fn producer_margin(&self, market: &Market, policy: &MarketPolicy, good: &ContentId) -> u32 {
        market
            .recipes
            .iter()
            .filter_map(|recipe_id| self.world.resource::<Catalog>().recipes.get(recipe_id))
            .filter(|recipe| recipe.outputs.iter().any(|output| &output.good == good))
            .filter_map(|recipe| recipe.margin_percent)
            .max()
            .unwrap_or(policy.producer_margin_percent)
    }

    fn scarcity_multiplier(
        &self,
        market: &Market,
        policy: &MarketPolicy,
        good: &ContentId,
    ) -> Result<u64, CoreError> {
        const SCALE: u64 = 1_000;
        const MAX_BONUS: u64 = 500;
        let target = u64::from(
            market
                .targets
                .get(good)
                .copied()
                .unwrap_or(policy.default_target),
        );
        if target == 0 {
            return Ok(SCALE);
        }
        let stock = market.inventory.get(good).copied().unwrap_or(0);
        let shortage = target.saturating_sub(stock).min(target);
        let bonus = u64::try_from(
            i128::from(MAX_BONUS)
                .checked_mul(i128::from(shortage))
                .ok_or(CoreError::Overflow)?
                .checked_add(i128::from(target - 1))
                .ok_or(CoreError::Overflow)?
                / i128::from(target),
        )
        .map_err(|_| CoreError::Overflow)?;
        SCALE.checked_add(bonus).ok_or(CoreError::Overflow)
    }

    fn ask_quote(
        &self,
        market: &Market,
        policy: &MarketPolicy,
        good: &ContentId,
    ) -> Result<Energy, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let basis = self.good_cost_basis(market, good)?;
        let scarcity = self.scarcity_multiplier(market, policy, good)?;
        let price = match policy.pricing_mode {
            PricingMode::Scarcity => checked_mul_ratio_ceil(basis, scarcity, 1_000)?,
            PricingMode::CostAware => {
                let margin = self.producer_margin(market, policy, good);
                let sustainable = checked_mul_ratio_ceil(
                    basis,
                    u64::from(100_u32.checked_add(margin).ok_or(CoreError::Overflow)?),
                    100,
                )?;
                checked_mul_ratio_ceil(sustainable, scarcity, 1_000)?
            }
        };
        Ok(Energy(price.0.max(1)))
    }

    fn processor_input_bid_ceiling(
        &self,
        market: &Market,
        policy: &MarketPolicy,
        good: &ContentId,
    ) -> Result<Option<Energy>, CoreError> {
        let catalog = self.world.resource::<Catalog>();
        let mut ceilings = Vec::new();
        for recipe_id in &market.recipes {
            let recipe = catalog
                .recipes
                .get(recipe_id)
                .ok_or_else(|| CoreError::Unknown {
                    kind: "recipe",
                    id: recipe_id.to_string(),
                })?;
            let Some(input) = recipe.inputs.iter().find(|input| &input.good == good) else {
                continue;
            };
            let output_revenue = recipe
                .outputs
                .iter()
                .try_fold(Energy::ZERO, |sum, output| {
                    sum.checked_add(
                        self.ask_quote(market, policy, &output.good)?
                            .checked_mul(u64::from(output.quantity))?,
                    )
                })?;
            let margin = recipe
                .margin_percent
                .unwrap_or(policy.producer_margin_percent);
            let maximum_total_cost = Energy(
                i64::try_from(
                    i128::from(output_revenue.0)
                        .checked_mul(100)
                        .ok_or(CoreError::Overflow)?
                        / i128::from(100_u32.checked_add(margin).ok_or(CoreError::Overflow)?),
                )
                .map_err(|_| CoreError::Overflow)?,
            );
            let maximum_input_budget = maximum_total_cost.checked_sub(recipe.operating_energy)?;
            let grounded_inputs = recipe.inputs.iter().try_fold(Energy::ZERO, |sum, item| {
                sum.checked_add(
                    self.good_cost_basis(market, &item.good)?
                        .checked_mul(u64::from(item.quantity))?,
                )
            })?;
            if grounded_inputs.0 <= 0 || input.quantity == 0 {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let target_grounded = self
                .good_cost_basis(market, good)?
                .checked_mul(u64::from(input.quantity))?;
            let allocated = i128::from(maximum_input_budget.0.max(0))
                .checked_mul(i128::from(target_grounded.0))
                .ok_or(CoreError::Overflow)?
                / i128::from(grounded_inputs.0);
            ceilings.push(Energy(
                i64::try_from(allocated).map_err(|_| CoreError::Overflow)?
                    / i64::from(input.quantity),
            ));
        }
        Ok(ceilings.into_iter().min())
    }

    fn demand_allowed(&self, market: &Market, good: &ContentId) -> bool {
        market.operating_profile.stage < BrownoutStage::Emergency
            || self
                .world
                .resource::<EconomyConfig>()
                .brownouts
                .survival_goods
                .contains(good)
    }

    fn normal_bid_quote(
        &self,
        market: &Market,
        policy: &MarketPolicy,
        good: &ContentId,
    ) -> Result<Energy, CoreError> {
        let ask = self.ask_quote(market, policy, good)?;
        let priority = u64::from(policy.import_priorities.get(good).copied().unwrap_or(100));
        let dynamic = checked_mul_ratio_ceil(ask, priority, 100)?;
        let bid = if policy.pricing_mode == PricingMode::CostAware {
            self.processor_input_bid_ceiling(market, policy, good)?
                .map_or(dynamic, |ceiling| dynamic.min(ceiling))
        } else {
            dynamic
        };
        Ok(Energy(bid.0.max(1)))
    }

    fn bid_quote(
        &self,
        market: &Market,
        policy: &MarketPolicy,
        good: &ContentId,
    ) -> Result<Energy, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        if !self.demand_allowed(market, good) {
            return Ok(Energy::ZERO);
        }
        let mut bid = self.normal_bid_quote(market, policy, good)?;
        let survival_good = self
            .world
            .resource::<EconomyConfig>()
            .brownouts
            .survival_goods
            .contains(good);
        if !survival_good {
            let level = market
                .investment_state
                .levels
                .get(&InvestmentKind::RouteSubsidy)
                .copied()
                .unwrap_or(0);
            if level > 0 {
                let effect = self
                    .world
                    .resource::<EconomyConfig>()
                    .investments
                    .get(&InvestmentKind::RouteSubsidy)
                    .ok_or(CoreError::InvalidWorldDynamics)?
                    .effect_per_level;
                let premium = effect.checked_mul(level).ok_or(CoreError::Overflow)?;
                bid = checked_mul_ratio_ceil(
                    bid,
                    u64::from(100_u32.checked_add(premium).ok_or(CoreError::Overflow)?),
                    100,
                )?;
                if policy.pricing_mode == PricingMode::CostAware
                    && let Some(ceiling) = self.processor_input_bid_ceiling(market, policy, good)?
                {
                    // A route subsidy is paid by this market, not by an
                    // external treasury. Preserve the processor-solvency cap
                    // after adding the premium so the investment cannot bid a
                    // recipe above its sustainable input budget.
                    bid = bid.min(Energy(ceiling.0.max(1)));
                }
            }
        }
        Ok(bid)
    }
    fn cargo_used(trader: &Trader) -> Result<u64, CoreError> {
        trader
            .cargo
            .values()
            .try_fold(0_u64, |a, v| a.checked_add(*v).ok_or(CoreError::Overflow))
    }

    fn local_buy(
        &mut self,
        trader_entity: Entity,
        good: &ContentId,
        quantity: u32,
    ) -> Result<(), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let (system, tank, travel, used, cap) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
            let trader_id = self.world.get::<StableId>(trader_entity).unwrap();
            self.ensure_carrier_contract_free(&trader_id.0, t)?;
            (
                t.system.clone(),
                t.energy_tank,
                t.travel.is_some(),
                Self::cargo_used(t)?,
                u64::from(t.cargo_capacity),
            )
        };
        if travel {
            return Err(CoreError::InTransit);
        }
        let market_entity = self.market_entity(&system)?;
        let (price, stock, cost) = {
            let m = self.world.get::<Market>(market_entity).unwrap();
            let p = self.world.get::<MarketPolicy>(market_entity).unwrap();
            (
                self.ask_quote(m, p, good)?,
                m.inventory.get(good).copied().unwrap_or(0),
                m.cost_basis
                    .get(good)
                    .copied()
                    .unwrap_or_default()
                    .removal_cost(u64::from(quantity))?,
            )
        };
        let total = price.checked_mul(u64::from(quantity))?;
        if stock < u64::from(quantity) {
            return Err(CoreError::InsufficientStock);
        }
        if tank < total {
            return Err(CoreError::InsufficientEnergy);
        }
        if used
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?
            > cap
        {
            return Err(CoreError::InsufficientCapacity);
        }
        let initial_market_energy = self
            .world
            .get::<Market>(market_entity)
            .unwrap()
            .energy_stock()?;
        let market_energy = initial_market_energy.checked_add(total)?;
        if market_energy
            > self
                .world
                .get::<Market>(market_entity)
                .unwrap()
                .energy_storage_cap
        {
            return Err(CoreError::InsufficientCapacity);
        }
        let cargo_next = self
            .world
            .get::<Trader>(trader_entity)
            .unwrap()
            .cargo
            .get(good)
            .copied()
            .unwrap_or(0)
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        *market
            .inventory
            .get_mut(good)
            .ok_or(CoreError::InsufficientStock)? -= u64::from(quantity);
        market
            .cost_basis
            .get_mut(good)
            .ok_or(CoreError::InsufficientStock)?
            .remove(u64::from(quantity))?;
        market.set_energy_stock(market_energy)?;
        market.ledger.energy_received_from_traders = market
            .ledger
            .energy_received_from_traders
            .checked_add(total)?;
        market.ledger.units_sold_to_traders = market
            .ledger
            .units_sold_to_traders
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        if market.recipes.iter().any(|recipe_id| {
            self.world
                .resource::<Catalog>()
                .recipes
                .get(recipe_id)
                .is_some_and(|recipe| recipe.outputs.iter().any(|output| output.good == *good))
        }) {
            market.ledger.processor_output_revenue =
                market.ledger.processor_output_revenue.checked_add(total)?;
        }
        market.energy_flow.tank_to_market = market.energy_flow.tank_to_market.checked_add(total)?;
        trader.energy_tank = trader.energy_tank.checked_sub(total)?;
        trader.cargo.insert(good.clone(), cargo_next);
        trader
            .cargo_cost_basis
            .entry(good.clone())
            .or_default()
            .add(u64::from(quantity), cost)?;
        trader.ledger.purchase_cost = trader.ledger.purchase_cost.checked_add(total)?;
        trader.ledger.completed_transactions = trader
            .ledger
            .completed_transactions
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        *self.world.get_mut::<Market>(market_entity).unwrap() = market;
        *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Bought {
                trader: trader_id,
                good: good.clone(),
                quantity,
                total,
            });
        Ok(())
    }

    fn local_sell(
        &mut self,
        trader_entity: Entity,
        good: &ContentId,
        requested: u32,
        liquidation: bool,
    ) -> Result<u32, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        if requested == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let (system, cargo, tank, cap, travel) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
            let trader_id = self.world.get::<StableId>(trader_entity).unwrap();
            self.ensure_carrier_contract_free(&trader_id.0, t)?;
            (
                t.system.clone(),
                t.cargo.get(good).copied().unwrap_or(0),
                t.energy_tank,
                t.energy_tank_capacity,
                t.travel.is_some(),
            )
        };
        if travel {
            return Err(CoreError::InTransit);
        }
        if cargo == 0 {
            return Err(CoreError::InsufficientStock);
        }
        let market_entity = self.market_entity(&system)?;
        let (bid, mut quantity) = {
            let m = self.world.get::<Market>(market_entity).unwrap();
            let p = self.world.get::<MarketPolicy>(market_entity).unwrap();
            let normal = self.bid_quote(m, p, good)?;
            let bid = if liquidation {
                let reference = self
                    .world
                    .resource::<Catalog>()
                    .goods
                    .get(good)
                    .ok_or_else(|| CoreError::Unknown {
                        kind: "good",
                        id: good.to_string(),
                    })?
                    .bootstrap_cost;
                liquidation_unit_price(reference, p.liquidation_discount_percent)?
            } else {
                normal
            };
            if bid == Energy::ZERO {
                return Err(CoreError::Unfunded);
            }
            let requested = requested.min(u32::try_from(cargo).unwrap_or(u32::MAX));
            let quantity = self.funded_quantity_with_preload_claims(
                &system,
                m,
                p,
                requested,
                bid,
                FundingProtection {
                    released_ordinary_claim: Energy::ZERO,
                    protect_liquidation_budget: !liquidation,
                },
            )?;
            (bid, quantity)
        };
        let headroom = cap.checked_sub(tank)?;
        quantity = quantity.min(u32::try_from(headroom.0 / bid.0).unwrap_or(u32::MAX));
        if quantity == 0 {
            return Err(CoreError::Unfunded);
        }
        self.execute_funded_sale(
            trader_entity,
            market_entity,
            good,
            quantity,
            SaleTerms {
                unit_price: bid,
                reserved_release: Energy::ZERO,
                partial: quantity < requested,
            },
        )?;
        Ok(quantity)
    }

    /// Shared validate-before-mutate settlement used by ordinary reservations,
    /// immediate funded sales, and liquidation. Energy rejects at this boundary.
    fn execute_funded_sale(
        &mut self,
        trader_entity: Entity,
        market_entity: Entity,
        good: &ContentId,
        quantity: u32,
        terms: SaleTerms,
    ) -> Result<(), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let total = terms.unit_price.checked_mul(u64::from(quantity))?;
        if total != terms.reserved_release && terms.reserved_release != Energy::ZERO {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        let cargo = trader.cargo.get(good).copied().unwrap_or(0);
        if cargo < u64::from(quantity) {
            return Err(CoreError::InsufficientStock);
        }
        let cargo_cost = trader
            .cargo_cost_basis
            .get(good)
            .copied()
            .unwrap_or_default()
            .removal_cost(u64::from(quantity))?;
        let next_tank = trader.energy_tank.checked_add(total)?;
        if next_tank > trader.energy_tank_capacity {
            return Err(CoreError::InsufficientTankCapacity);
        }
        let after_payment = market.energy_stock()?.checked_sub(total)?;
        let next_stock = market
            .inventory
            .get(good)
            .copied()
            .unwrap_or(0)
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        market.set_energy_stock(after_payment)?;
        market.inventory.insert(good.clone(), next_stock);
        market
            .cost_basis
            .entry(good.clone())
            .or_default()
            .add(u64::from(quantity), cargo_cost)?;
        market.reserved_energy = market.reserved_energy.checked_sub(terms.reserved_release)?;
        market.ledger.energy_paid_to_traders =
            market.ledger.energy_paid_to_traders.checked_add(total)?;
        market.ledger.units_bought_from_traders = market
            .ledger
            .units_bought_from_traders
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        market.energy_flow.market_to_tank = market.energy_flow.market_to_tank.checked_add(total)?;

        trader.energy_tank = next_tank;
        let left = cargo - u64::from(quantity);
        if left == 0 {
            trader.cargo.remove(good);
            trader.cargo_cost_basis.remove(good);
        } else {
            trader.cargo.insert(good.clone(), left);
            trader
                .cargo_cost_basis
                .get_mut(good)
                .ok_or(CoreError::InsufficientStock)?
                .remove(u64::from(quantity))?;
        }
        trader.ledger.sales_revenue = trader.ledger.sales_revenue.checked_add(total)?;
        trader.ledger.cargo_units_moved = trader
            .ledger
            .cargo_units_moved
            .checked_add(u64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        trader.ledger.completed_transactions = trader
            .ledger
            .completed_transactions
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;

        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        *self.world.get_mut::<Market>(market_entity).unwrap() = market;
        *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Sold {
                trader: trader_id,
                good: good.clone(),
                quantity,
                total,
                partial: terms.partial,
            });
        Ok(())
    }

    fn preload_export_claims_for_source(&self, source: &ContentId) -> Result<Energy, CoreError> {
        self.world
            .resource::<EnergyContracts>()
            .active
            .values()
            .filter(|contract| &contract.source == source)
            .try_fold(Energy::ZERO, |claims, contract| match contract.state {
                EnergyContractState::DeadheadingToSource { source_claim, .. } => {
                    claims.checked_add(source_claim)
                }
                _ => Ok(claims),
            })
    }

    fn market_exportable_energy_for_state(
        &self,
        market_entity: Entity,
        market: &Market,
    ) -> Result<Energy, CoreError> {
        let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
        let source = &self.world.get::<StableId>(market_entity).unwrap().0;
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        energy_logistics::exportable_energy(
            market.energy_stock()?,
            market.reserved_energy,
            self.preload_export_claims_for_source(source)?,
            market.operating_reserve(policy, life)?,
            market.protected_liquidation_budget,
            market.energy_logistics.export_reserve,
        )
    }

    fn market_exportable_energy(&self, market_entity: Entity) -> Result<Energy, CoreError> {
        self.market_exportable_energy_for_state(
            market_entity,
            self.world.get::<Market>(market_entity).unwrap(),
        )
    }

    fn projected_market_after_exportable_withdrawal(
        &self,
        market_entity: Entity,
        amount: Energy,
    ) -> Result<Option<Market>, CoreError> {
        if amount.0 < 0 {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
        if self.market_exportable_energy_for_state(market_entity, &market)? < amount {
            return Ok(None);
        }
        market.set_energy_stock(market.energy_stock()?.checked_sub(amount)?)?;
        Ok(Some(market))
    }

    fn funded_quantity_with_preload_claims(
        &self,
        system: &ContentId,
        market: &Market,
        policy: &MarketPolicy,
        requested: u32,
        bid: Energy,
        protection: FundingProtection,
    ) -> Result<u32, CoreError> {
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let ordinary_claims = market
            .reserved_energy
            .checked_sub(protection.released_ordinary_claim)?;
        let claims = ordinary_claims.checked_add(self.preload_export_claims_for_source(system)?)?;
        funded_quantity(
            requested,
            market.energy_stock()?,
            claims,
            market.operating_reserve(policy, life)?,
            if protection.protect_liquidation_budget {
                market.protected_liquidation_budget
            } else {
                Energy::ZERO
            },
            bid,
        )
    }

    fn transfer_owned_bulk(
        &mut self,
        trader_entity: Entity,
        amount: Energy,
        to_market: bool,
    ) -> Result<(), CoreError> {
        if amount.0 <= 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        if self.carrier_has_active_energy_contract(&trader_id) {
            return Err(CoreError::ActiveEnergyContract);
        }
        if trader.travel.is_some() {
            return Err(CoreError::InTransit);
        }
        if trader.bulk_energy.owned < amount {
            return Err(CoreError::InsufficientStock);
        }
        if trader.bulk_energy.locked.is_some() {
            return Err(CoreError::LockedEnergy);
        }
        let next_owned = trader.bulk_energy.owned.checked_sub(amount)?;
        let event = if to_market {
            let market_entity = self.market_entity(&trader.system)?;
            let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
            let next_stock = market.energy_stock()?.checked_add(amount)?;
            if next_stock > market.energy_storage_cap {
                return Err(CoreError::InsufficientCapacity);
            }
            market.set_energy_stock(next_stock)?;
            market.energy_flow.owned_bulk_deposited = market
                .energy_flow
                .owned_bulk_deposited
                .checked_add(amount)?;
            trader.bulk_energy.owned = next_owned;
            let system = trader.system.clone();
            *self.world.get_mut::<Market>(market_entity).unwrap() = market;
            *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
            EnergyContractEvent::OwnedBulkDepositedToMarket {
                trader: trader_id,
                system,
                amount,
            }
        } else {
            let next_tank = trader.energy_tank.checked_add(amount)?;
            if next_tank > trader.energy_tank_capacity {
                return Err(CoreError::InsufficientTankCapacity);
            }
            trader.energy_tank = next_tank;
            trader.bulk_energy.owned = next_owned;
            *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
            EnergyContractEvent::OwnedBulkTransferredToTank {
                trader: trader_id,
                amount,
            }
        };
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::EnergyLogistics(event));
        Ok(())
    }

    fn transfer_tank(
        &mut self,
        trader_entity: Entity,
        amount: Energy,
        deposit: bool,
    ) -> Result<(), CoreError> {
        if amount.0 <= 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let (system, tank, cap, travel, refuel_policy) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
            let trader_id = self.world.get::<StableId>(trader_entity).unwrap();
            self.ensure_carrier_contract_free(&trader_id.0, t)?;
            (
                t.system.clone(),
                t.energy_tank,
                t.energy_tank_capacity,
                t.travel.is_some(),
                t.refuel_policy,
            )
        };
        if travel {
            return Err(CoreError::InTransit);
        }
        if (deposit && !refuel_policy.permits_deposit())
            || (!deposit && !refuel_policy.permits_withdrawal())
        {
            return Err(CoreError::RefuelForbidden);
        }
        let market_entity = self.market_entity(&system)?;
        let stock = self
            .world
            .get::<Market>(market_entity)
            .unwrap()
            .energy_stock()?;
        let (next_stock, next_tank) = if deposit {
            if tank < amount {
                return Err(CoreError::InsufficientEnergy);
            }
            let next_stock = stock.checked_add(amount)?;
            if next_stock
                > self
                    .world
                    .get::<Market>(market_entity)
                    .unwrap()
                    .energy_storage_cap
            {
                return Err(CoreError::InsufficientCapacity);
            }
            (next_stock, tank.checked_sub(amount)?)
        } else {
            if self.market_exportable_energy(market_entity)? < amount {
                return Err(CoreError::InsufficientEnergy);
            }
            let next = tank.checked_add(amount)?;
            if next > cap {
                return Err(CoreError::InsufficientTankCapacity);
            }
            (stock.checked_sub(amount)?, next)
        };
        let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        market.set_energy_stock(next_stock)?;
        if deposit {
            market.energy_flow.tank_to_market =
                market.energy_flow.tank_to_market.checked_add(amount)?;
        } else {
            market.energy_flow.market_to_tank =
                market.energy_flow.market_to_tank.checked_add(amount)?;
        }
        trader.energy_tank = next_tank;
        *self.world.get_mut::<Market>(market_entity).unwrap() = market;
        *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
        Ok(())
    }
    fn rebalance_idle_npc_tanks(&mut self) -> Result<(), CoreError> {
        let mut traders = self
            .world
            .query_filtered::<(Entity, &StableId, &Trader), Without<PlayerControlled>>()
            .iter(&self.world)
            .filter(|(entity, id, trader)| {
                trader.travel.is_none()
                    && trader.cargo.is_empty()
                    && trader.bulk_energy.locked.is_none()
                    && !self.carrier_has_active_energy_contract(&id.0)
                    && self
                        .world
                        .get::<TraderLifecycle>(*entity)
                        .is_none_or(|state| state.retirement.is_none())
            })
            .map(|(entity, id, trader)| {
                (
                    id.0.clone(),
                    entity,
                    trader.system.clone(),
                    trader.energy_tank,
                    trader.energy_tank_capacity,
                )
            })
            .collect::<Vec<_>>();
        traders.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, entity, system, tank, capacity) in traders {
            let target = Energy(capacity.0 / 2);
            let market_entity = self.market_entity(&system)?;
            if tank > target {
                let market = self.world.get::<Market>(market_entity).unwrap();
                let storage_headroom = market
                    .energy_storage_cap
                    .checked_sub(market.energy_stock()?)?;
                let amount = Energy((tank.0 - target.0).min(storage_headroom.0));
                if amount.0 > 0 {
                    self.transfer_tank(entity, amount, true)?;
                }
            } else if tank < target {
                let available = self.market_exportable_energy(market_entity)?;
                let amount = Energy((target.0 - tank.0).min(available.0));
                if amount.0 > 0 {
                    self.transfer_tank(entity, amount, false)?;
                }
            }
        }
        Ok(())
    }

    fn player_governs(&mut self, system: &ContentId) -> Result<Entity, CoreError> {
        let player_entity = self.player_entity()?;
        let player = self.world.get::<StableId>(player_entity).unwrap().0.clone();
        let market_entity = self.market_entity(system)?;
        let authorized = matches!(
            &self.world.get::<Market>(market_entity).unwrap().governance.authority,
            MarketAuthority::Player(governor) if governor == &player
        );
        if !authorized {
            return Err(CoreError::UnauthorizedMarketPolicy);
        }
        Ok(market_entity)
    }

    fn set_player_policy(
        &mut self,
        system: &ContentId,
        policy: MarketPolicy,
    ) -> Result<(), CoreError> {
        let market_entity = self.player_governs(system)?;
        self.apply_market_policy(system, market_entity, policy)
    }

    fn set_player_governor_policy(
        &mut self,
        system: &ContentId,
        policy: GovernorMarketPolicy,
    ) -> Result<(), CoreError> {
        let market_entity = self.player_governs(system)?;
        let mut merged = self
            .world
            .get::<MarketPolicy>(market_entity)
            .unwrap()
            .clone();
        merged.producer_margin_percent = policy.producer_margin_percent;
        merged.operating_reserve_ticks = policy.operating_reserve_ticks;
        merged.import_priorities = policy.import_priorities;
        self.apply_market_policy(system, market_entity, merged)
    }

    fn set_player_market_target(
        &mut self,
        system: &ContentId,
        good: &ContentId,
        target: u32,
    ) -> Result<(), CoreError> {
        let market_entity = self.player_governs(system)?;
        if target == 0 {
            return Err(CoreError::InvalidMarketTarget);
        }
        if !self.world.resource::<Catalog>().goods.contains_key(good) {
            return Err(CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            });
        }
        let market = self.world.get::<Market>(market_entity).unwrap();
        let effective = self
            .world
            .resource::<EconomyConfig>()
            .population
            .tertiary_demand_per_thousand
            .get(good)
            .map_or(Ok(target), |units_per_thousand| {
                population_demand_target(
                    target,
                    market.population_state.current,
                    market.population_state.reference,
                    *units_per_thousand,
                )
            })?;
        let mut market = self.world.get_mut::<Market>(market_entity).unwrap();
        market.authored_targets.insert(good.clone(), target);
        market.targets.insert(good.clone(), effective);
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::MarketTargetChanged {
                system: system.clone(),
                good: good.clone(),
                target: effective,
            });
        Ok(())
    }

    fn set_player_investment_policy(
        &mut self,
        system: &ContentId,
        policy: InvestmentPolicy,
    ) -> Result<(), CoreError> {
        let market_entity = self.player_governs(system)?;
        policy.validate()?;
        self.world
            .get_mut::<Market>(market_entity)
            .unwrap()
            .investment_policy = policy;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::PolicyChanged {
                system: system.clone(),
            });
        Ok(())
    }

    // Internal autonomous governance can call this only after its own authority
    // selection; public/player commands must always use `set_player_policy`.
    fn apply_market_policy(
        &mut self,
        system: &ContentId,
        market_entity: Entity,
        policy: MarketPolicy,
    ) -> Result<(), CoreError> {
        policy.validate()?;
        if policy
            .import_priorities
            .keys()
            .any(|good| !self.world.resource::<Catalog>().goods.contains_key(good))
        {
            return Err(CoreError::InvalidPolicy);
        }
        let bootstrap_costs = self
            .world
            .resource::<Catalog>()
            .goods
            .values()
            .map(|good| good.bootstrap_cost)
            .collect::<Vec<_>>();
        let mut capabilities = self
            .world
            .query_filtered::<&Trader, With<PlayerControlled>>()
            .iter(&self.world)
            .map(|trader| LiquidationTraderCapability {
                cargo_capacity: trader.cargo_capacity,
                energy_tank_capacity: trader.energy_tank_capacity,
                travel_burn_per_distance: trader.travel_burn_per_distance,
            })
            .collect::<Vec<_>>();
        capabilities.extend(
            self.world
                .resource::<FleetDynamics>()
                .archetypes
                .values()
                .map(FleetArchetype::liquidation_capability),
        );
        let protected_liquidation_budget = compute_protected_liquidation_budget(
            self.graph(),
            system,
            &policy,
            &bootstrap_costs,
            &capabilities,
        )?;

        *self.world.get_mut::<MarketPolicy>(market_entity).unwrap() = policy;
        self.world
            .get_mut::<Market>(market_entity)
            .unwrap()
            .protected_liquidation_budget = protected_liquidation_budget;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::PolicyChanged {
                system: system.clone(),
            });
        Ok(())
    }

    fn execute_autonomous_investments(&mut self) -> Result<(), CoreError> {
        let tick = self.tick();
        let (life, shapes, population_maximum) = {
            let config = self.world.resource::<EconomyConfig>();
            (
                config.life_support_burn_per_capita,
                config.investments.clone(),
                config.population.maximum_cap,
            )
        };
        let mut markets = self
            .world
            .query_filtered::<(Entity, &StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(entity, id, market, policy)| {
                (entity, id.0.clone(), market.clone(), policy.clone())
            })
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| left.1.cmp(&right.1));

        let mut prepared = Vec::new();
        let mut events = Vec::new();
        for (entity, system, mut market, policy) in markets {
            market.investment_policy.validate()?;
            let available = market.unreserved_energy_for_purchases(&policy, life)?;
            let mut ranked = Vec::new();
            for kind in [
                InvestmentKind::Collector,
                InvestmentKind::Storage,
                InvestmentKind::PopulationSupport,
                InvestmentKind::RouteSubsidy,
            ] {
                let shape = shapes.get(&kind).ok_or(CoreError::InvalidWorldDynamics)?;
                let level = market
                    .investment_state
                    .levels
                    .get(&kind)
                    .copied()
                    .unwrap_or(0);
                let allocation = market
                    .investment_policy
                    .allocation_percent
                    .get(&kind)
                    .copied()
                    .unwrap_or(0);
                let status = if !shape.enabled {
                    InvestmentStatus::Disabled
                } else if !market.operating_profile.investment_allowed {
                    InvestmentStatus::DisabledByStage(market.operating_profile.stage)
                } else if level >= shape.maximum_level {
                    InvestmentStatus::MaximumLevel
                } else if market
                    .investment_state
                    .cooldown_until
                    .get(&kind)
                    .copied()
                    .unwrap_or(0)
                    > tick
                {
                    InvestmentStatus::CoolingDown {
                        until_tick: market.investment_state.cooldown_until[&kind],
                    }
                } else if allocation == 0 {
                    InvestmentStatus::Unallocated
                } else {
                    let cost = investment_cost(shape, level)?;
                    if available < cost {
                        InvestmentStatus::InsufficientFunds { available, cost }
                    } else {
                        ranked.push((std::cmp::Reverse(allocation), kind, cost));
                        InvestmentStatus::Ready { cost }
                    }
                };
                market.investment_state.status.insert(kind, status);
            }
            ranked.sort();
            if let Some((_, kind, cost)) = ranked.into_iter().next() {
                let shape = shapes.get(&kind).ok_or(CoreError::InvalidWorldDynamics)?;
                let level = market
                    .investment_state
                    .levels
                    .get(&kind)
                    .copied()
                    .unwrap_or(0);
                let next_level = level.checked_add(1).ok_or(CoreError::Overflow)?;
                let next_stock = market.energy_stock()?.checked_sub(cost)?;
                let cooldown_until = tick
                    .checked_add(u64::from(shape.cooldown_ticks))
                    .ok_or(CoreError::Overflow)?;
                match kind {
                    InvestmentKind::Collector => {
                        let effect = Energy(i64::from(shape.effect_per_level));
                        let next_base =
                            market.seasonal_generation.base_output.checked_add(effect)?;
                        market.energy_output_per_tick = next_base;
                        market.seasonal_generation.base_output = next_base;
                    }
                    InvestmentKind::Storage => {
                        market.energy_storage_cap = market
                            .energy_storage_cap
                            .checked_add(Energy(i64::from(shape.effect_per_level)))?;
                    }
                    InvestmentKind::PopulationSupport => {
                        market.population_state.support_capacity = market
                            .population_state
                            .support_capacity
                            .checked_add(u64::from(shape.effect_per_level))
                            .ok_or(CoreError::Overflow)?
                            .min(population_maximum);
                        market.population_state.growth_rate_bonus_percent = market
                            .population_state
                            .growth_rate_bonus_percent
                            .checked_add(shape.effect_per_level)
                            .ok_or(CoreError::Overflow)?;
                    }
                    InvestmentKind::RouteSubsidy => {}
                }
                market.set_energy_stock(next_stock)?;
                market.energy_flow.investment_burned =
                    market.energy_flow.investment_burned.checked_add(cost)?;
                market.investment_state.levels.insert(kind, next_level);
                market
                    .investment_state
                    .cooldown_until
                    .insert(kind, cooldown_until);
                market
                    .investment_state
                    .status
                    .insert(kind, InvestmentStatus::Completed { tick, cost });
                let remaining = market.unreserved_energy_for_purchases(&policy, life)?;
                for (other_kind, status) in &mut market.investment_state.status {
                    if *other_kind == kind {
                        continue;
                    }
                    if let InvestmentStatus::Ready { cost: other_cost } = *status
                        && remaining < other_cost
                    {
                        *status = InvestmentStatus::InsufficientFunds {
                            available: remaining,
                            cost: other_cost,
                        };
                    }
                }
                events.push(GameEvent::InvestmentCompleted {
                    system: system.clone(),
                    kind,
                    level: next_level,
                    cost,
                });
            }
            prepared.push((entity, market));
        }
        let completed = u64::try_from(events.len()).map_err(|_| CoreError::Overflow)?;
        let next_completed = self
            .world
            .resource::<AggregateDynamicsHistory>()
            .investments_completed
            .checked_add(completed)
            .ok_or(CoreError::Overflow)?;
        for (entity, market) in prepared {
            *self.world.get_mut::<Market>(entity).unwrap() = market;
        }
        self.world
            .resource_mut::<AggregateDynamicsHistory>()
            .investments_completed = next_completed;
        self.world.resource_mut::<EventBuffer>().0.extend(events);
        Ok(())
    }

    fn begin_travel(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
    ) -> Result<(), CoreError> {
        let (start, speed, burn, tank) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
            let trader_id = self.world.get::<StableId>(trader_entity).unwrap();
            self.ensure_carrier_contract_free(&trader_id.0, t)?;
            if t.travel.is_some() {
                return Err(CoreError::InTransit);
            }
            (
                t.system.clone(),
                t.speed,
                t.travel_burn_per_distance,
                t.energy_tank,
            )
        };
        if &start == destination {
            return Err(CoreError::AlreadyThere);
        }
        let (route, _) = self
            .graph()
            .shortest_path(&start, destination)
            .ok_or(CoreError::NoRoute)?;
        let travel_burn = route_travel_energy(self.graph(), &route, burn)?;
        if tank < travel_burn {
            return Err(CoreError::InsufficientEnergy);
        }
        let remaining_ticks = ticks_for_distance(self.graph().route_distance(&route[..2]), speed);
        let id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        let origin = self.market_entity(&start)?;
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        let mut market = self.world.get::<Market>(origin).unwrap().clone();
        trader.energy_tank = trader.energy_tank.checked_sub(travel_burn)?;
        trader.ledger.travel_cost = trader.ledger.travel_cost.checked_add(travel_burn)?;
        trader.travel = Some(TravelPlan {
            destination: destination.clone(),
            route,
            next_leg: 1,
            remaining_ticks,
        });
        market.energy_flow.travel_burned =
            market.energy_flow.travel_burned.checked_add(travel_burn)?;
        *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
        *self.world.get_mut::<Market>(origin).unwrap() = market;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Departed {
                trader: id,
                destination: destination.clone(),
                travel_burn,
            });
        Ok(())
    }
    fn advance_travel(&mut self) -> Result<(), CoreError> {
        let graph = self.graph().clone();
        let mut arrivals = Vec::new();
        let mut query = self.world.query::<(Entity, &StableId, &mut Trader)>();
        for (e, id, mut t) in query.iter_mut(&mut self.world) {
            let speed = t.speed;
            let Some(mut p) = t.travel.take() else {
                continue;
            };
            p.remaining_ticks = p.remaining_ticks.saturating_sub(1);
            if p.remaining_ticks > 0 {
                t.travel = Some(p);
                continue;
            }
            t.system = p.route[p.next_leg].clone();
            p.next_leg += 1;
            if p.next_leg >= p.route.len() {
                arrivals.push((e, id.0.clone(), t.system.clone()));
            } else {
                let d = graph.route_distance(&p.route[p.next_leg - 1..=p.next_leg]);
                p.remaining_ticks = ticks_for_distance(d, speed);
                t.travel = Some(p);
            }
        }
        for (e, id, system) in arrivals {
            if let Some(r) = self.world.get::<Trader>(e).unwrap().reservation {
                self.refresh_reservation(r)?;
            }
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::Arrived { trader: id, system });
        }
        Ok(())
    }

    fn generate_and_life_support(&mut self) -> Result<(), CoreError> {
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let updates = self
            .world
            .query::<(Entity, &StableId, &Market)>()
            .iter(&self.world)
            .map(|(e, id, m)| {
                let stock = m.energy_stock()?;
                let generated = m.seasonal_generation.effective_output_at(self.tick())?;
                let gross = stock.checked_add(generated)?;
                let stored = Energy(gross.0.min(m.energy_storage_cap.0));
                let curtailed = Energy(gross.0 - m.energy_storage_cap.0.min(gross.0));
                let obligation = life.checked_mul(m.population)?;
                let burned = Energy(stored.0.min(obligation.0));
                let unsupplied = Energy(obligation.0 - burned.0);
                let final_stock = stored.checked_sub(burned)?;
                let mut flow = m.energy_flow;
                flow.generated = flow.generated.checked_add(generated)?;
                flow.curtailed = flow.curtailed.checked_add(curtailed)?;
                flow.life_support_burned = flow.life_support_burned.checked_add(burned)?;
                flow.life_support_unsupplied =
                    flow.life_support_unsupplied.checked_add(unsupplied)?;
                Ok((
                    e,
                    id.0.clone(),
                    final_stock,
                    flow,
                    generated,
                    curtailed,
                    burned,
                    unsupplied,
                ))
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        for (e, id, stock, flow, generated, curtailed, burned, unsupplied) in updates {
            let mut m = self.world.get_mut::<Market>(e).unwrap();
            m.set_energy_stock(stock)?;
            m.energy_flow = flow;
            m.seasonal_generation.current_effective_output = generated;
            m.last_life_support_unsupplied = unsupplied;
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::EnergyGenerated {
                    system: id.clone(),
                    amount: generated,
                    curtailed,
                });
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::LifeSupport {
                    system: id,
                    burned,
                    unsupplied,
                });
        }
        Ok(())
    }

    fn classify_brownouts(&mut self) -> Result<(), CoreError> {
        let tick = self.tick();
        let (life, config) = {
            let economy = self.world.resource::<EconomyConfig>();
            (
                economy.life_support_burn_per_capita,
                economy.brownouts.clone(),
            )
        };
        let mut updates = self
            .world
            .query::<(Entity, &StableId, &Market)>()
            .iter(&self.world)
            .map(|(entity, id, market)| {
                let obligation = life.checked_mul(market.population)?;
                let ticks_of_burn = if obligation.0 == 0 {
                    u32::MAX
                } else {
                    u32::try_from(market.energy_stock()?.0 / obligation.0).unwrap_or(u32::MAX)
                };
                let next_stage = classify_brownout(
                    &market.brownout,
                    &config,
                    ticks_of_burn,
                    market.last_life_support_unsupplied,
                    tick,
                )?;
                let mut next = market.clone();
                let from = next.brownout.stage;
                next.brownout.ticks_of_burn = ticks_of_burn;
                if next_stage != from {
                    next.brownout.stage = next_stage;
                    next.brownout.entered_at_tick = tick;
                    next.brownout.transition_count = next
                        .brownout
                        .transition_count
                        .checked_add(1)
                        .ok_or(CoreError::Overflow)?;
                }
                next.brownout.occupancy_ticks[next_stage.index()] = next.brownout.occupancy_ticks
                    [next_stage.index()]
                .checked_add(1)
                .ok_or(CoreError::Overflow)?;
                next.operating_profile = MarketOperatingProfile {
                    stage: next_stage,
                    throughput_percent: config.throughput_percent(next_stage),
                    labor_percent: population_labor_percent(
                        next.population_state.current,
                        next.population_state.reference,
                    )?,
                    investment_allowed: next_stage < BrownoutStage::Emergency,
                };
                Ok((entity, id.0.clone(), from, next_stage, ticks_of_burn, next))
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        updates.sort_by(|a, b| a.1.cmp(&b.1));
        let mut transition_events = Vec::new();
        let mut occupancy = [0_u64; 4];
        let mut transitions = 0_u64;
        for (_, system, from, to, ticks_of_burn, _) in &updates {
            occupancy[to.index()] = occupancy[to.index()]
                .checked_add(1)
                .ok_or(CoreError::Overflow)?;
            if from != to {
                transitions = transitions.checked_add(1).ok_or(CoreError::Overflow)?;
                transition_events.push(GameEvent::BrownoutTransition {
                    system: system.clone(),
                    from: *from,
                    to: *to,
                    ticks_of_burn: *ticks_of_burn,
                    tick,
                });
            }
        }
        let mut next_history = self.world.resource::<AggregateDynamicsHistory>().clone();
        for (target, value) in next_history.stage_occupancy_ticks.iter_mut().zip(occupancy) {
            *target = target.checked_add(value).ok_or(CoreError::Overflow)?;
        }
        next_history.stage_transitions = next_history
            .stage_transitions
            .checked_add(transitions)
            .ok_or(CoreError::Overflow)?;

        for (entity, _, _, _, _, next) in updates {
            *self.world.get_mut::<Market>(entity).unwrap() = next;
        }
        *self.world.resource_mut::<AggregateDynamicsHistory>() = next_history;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .extend(transition_events);
        Ok(())
    }

    fn update_populations(&mut self) -> Result<(), CoreError> {
        let config = self.world.resource::<EconomyConfig>().population.clone();
        if config.static_population {
            return Ok(());
        }
        let life_per_capita = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let energy_id = ContentId::new(ENERGY_ID).expect("constant energy id");
        let window = usize::try_from(config.sufficiency_window).map_err(|_| CoreError::Overflow)?;
        let mut updates = self
            .world
            .query::<(Entity, &StableId, &Market)>()
            .iter(&self.world)
            .map(|(entity, id, market)| {
                let obligation = life_per_capita.checked_mul(market.population)?;
                let energy_sufficiency = if obligation.0 == 0 {
                    100
                } else {
                    let supplied = obligation.checked_sub(market.last_life_support_unsupplied)?;
                    u32::try_from(
                        i128::from(supplied.0)
                            .checked_mul(100)
                            .ok_or(CoreError::Overflow)?
                            / i128::from(obligation.0),
                    )
                    .map_err(|_| CoreError::Overflow)?
                    .min(100)
                };
                let mut sample = energy_sufficiency;
                let mut sampled_goods = config.essential_goods.clone();
                sampled_goods.extend(config.tertiary_demand_per_thousand.keys().cloned());
                sampled_goods.remove(&energy_id);
                for good in sampled_goods {
                    let authored = market.authored_targets.get(&good).copied().unwrap_or(0);
                    let target = if let Some(rate) = config.tertiary_demand_per_thousand.get(&good)
                    {
                        population_demand_target(
                            authored,
                            market.population,
                            market.population_state.reference,
                            *rate,
                        )?
                    } else {
                        authored
                    };
                    if target == 0 {
                        continue;
                    }
                    let stock = market.inventory.get(&good).copied().unwrap_or(0);
                    let percent = u32::try_from(
                        u128::from(stock)
                            .checked_mul(100)
                            .ok_or(CoreError::Overflow)?
                            / u128::from(target),
                    )
                    .unwrap_or(u32::MAX)
                    .min(100);
                    sample = sample.min(percent);
                }

                let mut next = market.clone();
                next.population_state.sufficiency_samples.push_back(sample);
                next.population_state.sufficiency_sum = next
                    .population_state
                    .sufficiency_sum
                    .checked_add(u64::from(sample))
                    .ok_or(CoreError::Overflow)?;
                if next.population_state.sufficiency_samples.len() > window {
                    let evicted = next
                        .population_state
                        .sufficiency_samples
                        .pop_front()
                        .expect("window overflow guarantees an oldest sample");
                    next.population_state.sufficiency_sum = next
                        .population_state
                        .sufficiency_sum
                        .checked_sub(u64::from(evicted))
                        .ok_or(CoreError::Overflow)?;
                }
                let sample_count = u64::try_from(next.population_state.sufficiency_samples.len())
                    .map_err(|_| CoreError::Overflow)?;
                next.population_state.sufficiency_average_percent =
                    u32::try_from(next.population_state.sufficiency_sum / sample_count)
                        .map_err(|_| CoreError::Overflow)?;
                let history_cap = u128::from(next.population_state.support_capacity)
                    .checked_mul(u128::from(
                        next.population_state.sufficiency_average_percent,
                    ))
                    .ok_or(CoreError::Overflow)?
                    / 100;
                next.population_state.carrying_capacity = u64::try_from(history_cap)
                    .map_err(|_| CoreError::Overflow)?
                    .clamp(config.minimum_cap, config.maximum_cap)
                    .min(next.population_state.support_capacity);

                let from_population = next.population;
                let from_tier = next.population_state.tier;
                next.population_state.trend = PopulationTrend::Stable;
                if next.brownout.stage == BrownoutStage::Starvation && next.population > 0 {
                    let decline = proportional_population_delta(
                        next.population,
                        config.decline_per_thousand,
                        &mut next.population_state.decline_remainder,
                    )?;
                    next.population_state.decline_ticks = next
                        .population_state
                        .decline_ticks
                        .checked_add(1)
                        .ok_or(CoreError::Overflow)?;
                    if decline > 0 {
                        next.population = next
                            .population
                            .checked_sub(decline)
                            .ok_or(CoreError::Overflow)?;
                        next.population_state.trend = PopulationTrend::Declining;
                    }
                } else if next.population > 0
                    && next.brownout.stage == BrownoutStage::Normal
                    && next.population_state.sufficiency_samples.len() == window
                    && next.population_state.sufficiency_average_percent
                        >= config.growth_sufficiency_percent
                    && next.population < next.population_state.carrying_capacity
                {
                    let effective_growth_rate = u32::try_from(
                        u128::from(config.growth_per_thousand)
                            .checked_mul(u128::from(
                                100_u32
                                    .checked_add(next.population_state.growth_rate_bonus_percent)
                                    .ok_or(CoreError::Overflow)?,
                            ))
                            .ok_or(CoreError::Overflow)?
                            / 100,
                    )
                    .map_err(|_| CoreError::Overflow)?;
                    let growth = logistic_population_delta(
                        next.population,
                        next.population_state.carrying_capacity,
                        effective_growth_rate,
                        config.logistic_scale,
                        &mut next.population_state.growth_carry,
                    )?;
                    next.population_state.growth_ticks = next
                        .population_state
                        .growth_ticks
                        .checked_add(1)
                        .ok_or(CoreError::Overflow)?;
                    if growth > 0 {
                        next.population = next
                            .population
                            .checked_add(growth)
                            .ok_or(CoreError::Overflow)?
                            .min(next.population_state.carrying_capacity);
                        next.population_state.trend = PopulationTrend::Growing;
                    }
                }
                next.population_state.current = next.population;
                let changed = next.population != from_population;
                if changed {
                    next.population_state.settled_changes = next
                        .population_state
                        .settled_changes
                        .checked_add(1)
                        .ok_or(CoreError::Overflow)?;
                }
                let to_tier = population_tier(next.population, &config.tier_thresholds);
                next.population_state.tier = to_tier;
                next.operating_profile.labor_percent =
                    population_labor_percent(next.population, next.population_state.reference)?;
                for (good, rate) in &config.tertiary_demand_per_thousand {
                    let authored = next.authored_targets.get(good).copied().unwrap_or(0);
                    let target = population_demand_target(
                        authored,
                        next.population,
                        next.population_state.reference,
                        *rate,
                    )?;
                    next.targets.insert(good.clone(), target);
                }
                Ok((
                    id.0.clone(),
                    entity,
                    next,
                    from_population,
                    from_tier,
                    to_tier,
                    changed,
                ))
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        updates.sort_by(|left, right| left.0.cmp(&right.0));

        let population_changes = u64::try_from(updates.iter().filter(|row| row.6).count())
            .map_err(|_| CoreError::Overflow)?;
        let milestones = u64::try_from(updates.iter().filter(|row| row.4 != row.5).count())
            .map_err(|_| CoreError::Overflow)?;
        let mut history = self.world.resource::<AggregateDynamicsHistory>().clone();
        history.population_changes = history
            .population_changes
            .checked_add(population_changes)
            .ok_or(CoreError::Overflow)?;
        history.population_milestones = history
            .population_milestones
            .checked_add(milestones)
            .ok_or(CoreError::Overflow)?;
        let mut events = Vec::new();
        for (system, entity, next, from, from_tier, to_tier, changed) in updates {
            let population = next.population;
            *self.world.get_mut::<Market>(entity).unwrap() = next;
            if changed {
                events.push(GameEvent::PopulationChanged {
                    system: system.clone(),
                    from,
                    to: population,
                });
            }
            if from_tier != to_tier {
                events.push(GameEvent::PopulationTierChanged {
                    system,
                    from: from_tier,
                    to: to_tier,
                    population,
                });
            }
        }
        *self.world.resource_mut::<AggregateDynamicsHistory>() = history;
        self.world.resource_mut::<EventBuffer>().0.extend(events);
        Ok(())
    }

    fn execute_sources_and_recipes(&mut self) -> Result<(), CoreError> {
        let percent = self.world.resource::<EconomyConfig>().source_output_percent;
        let recipes = self.world.resource::<Catalog>().recipes.clone();
        let mut events = Vec::new();
        let mut updates = Vec::new();
        for (e, id, m) in self
            .world
            .query::<(Entity, &StableId, &Market)>()
            .iter(&self.world)
        {
            let mut next = m.clone();
            for source in &m.sources {
                let base_output = scaled_source_output(source.quantity_per_tick, percent)?;
                let carry = next
                    .throughput_carry
                    .entry(ThroughputScheduleKey::Source(source.good.clone()))
                    .or_insert(0);
                let output = u32::try_from(composed_throughput(
                    u64::from(base_output),
                    next.operating_profile.throughput_percent,
                    next.operating_profile.labor_percent,
                    carry,
                )?)
                .map_err(|_| CoreError::Overflow)?;
                let burn = source.extraction_energy.checked_mul(u64::from(output))?;
                if output == 0 || next.protected_discretionary_energy()? < burn {
                    continue;
                }
                let energy = next.energy_stock()?.checked_sub(burn)?;
                next.set_energy_stock(energy)?;
                let q = next
                    .inventory
                    .get(&source.good)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(u64::from(output))
                    .ok_or(CoreError::Overflow)?;
                next.inventory.insert(source.good.clone(), q);
                next.cost_basis
                    .entry(source.good.clone())
                    .or_default()
                    .add(u64::from(output), burn)?;
                next.energy_flow.source_burned =
                    next.energy_flow.source_burned.checked_add(burn)?;
                next.ledger.source_units_generated = next
                    .ledger
                    .source_units_generated
                    .checked_add(u64::from(output))
                    .ok_or(CoreError::Overflow)?;
            }
            for layer in [
                RecipeLayer::Primary,
                RecipeLayer::Secondary,
                RecipeLayer::Tertiary,
            ] {
                for recipe_id in &m.recipes {
                    let r = recipes.get(recipe_id).ok_or_else(|| CoreError::Unknown {
                        kind: "recipe",
                        id: recipe_id.to_string(),
                    })?;
                    if r.layer != layer {
                        continue;
                    }
                    let carry = next
                        .throughput_carry
                        .entry(ThroughputScheduleKey::Recipe(recipe_id.clone()))
                        .or_insert(0);
                    let executions = composed_throughput(
                        1,
                        next.operating_profile.throughput_percent,
                        next.operating_profile.labor_percent,
                        carry,
                    )?;
                    if executions == 0
                        || !r.inputs.iter().all(|i| {
                            next.inventory.get(&i.good).copied().unwrap_or(0)
                                >= u64::from(i.quantity)
                        })
                        || next.protected_discretionary_energy()? < r.operating_energy
                    {
                        continue;
                    }
                    let mut input_cost = Energy::ZERO;
                    let mut inv = next.inventory.clone();
                    let mut bases = next.cost_basis.clone();
                    for input in &r.inputs {
                        *inv.get_mut(&input.good).unwrap() -= u64::from(input.quantity);
                        input_cost = input_cost.checked_add(
                            bases
                                .get_mut(&input.good)
                                .unwrap()
                                .remove(u64::from(input.quantity))?,
                        )?;
                    }
                    let embodied = input_cost.checked_add(r.operating_energy)?;
                    for output in &r.outputs {
                        let q = inv
                            .get(&output.good)
                            .copied()
                            .unwrap_or(0)
                            .checked_add(u64::from(output.quantity))
                            .ok_or(CoreError::Overflow)?;
                        inv.insert(output.good.clone(), q);
                    }
                    let allocations = if r.outputs.is_empty() {
                        Vec::new()
                    } else {
                        allocate_embodied_energy(
                            embodied,
                            &r.outputs
                                .iter()
                                .map(|o| (o.good.clone(), o.quantity, o.cost_weight))
                                .collect::<Vec<_>>(),
                        )?
                    };
                    for (output, cost) in allocations {
                        let quantity = r
                            .outputs
                            .iter()
                            .find(|o| o.good == output)
                            .unwrap()
                            .quantity;
                        bases
                            .entry(output)
                            .or_default()
                            .add(u64::from(quantity), cost)?;
                    }
                    let inventory_energy = Energy(
                        i64::try_from(
                            inv.get(&ContentId::new(ENERGY_ID).expect("constant id"))
                                .copied()
                                .unwrap_or(0),
                        )
                        .map_err(|_| CoreError::Overflow)?,
                    );
                    let energy = inventory_energy.checked_sub(r.operating_energy)?;
                    if energy.0
                        < next
                            .reserved_energy
                            .checked_add(next.protected_liquidation_budget)?
                            .0
                    {
                        continue;
                    }
                    next.inventory = inv;
                    next.cost_basis = bases;
                    next.set_energy_stock(energy)?;
                    next.energy_flow.production_burned = next
                        .energy_flow
                        .production_burned
                        .checked_add(r.operating_energy)?;
                    next.ledger.recipe_input_units_consumed = next
                        .ledger
                        .recipe_input_units_consumed
                        .checked_add(r.inputs.iter().try_fold(0_u64, |sum, input| {
                            sum.checked_add(u64::from(input.quantity))
                                .ok_or(CoreError::Overflow)
                        })?)
                        .ok_or(CoreError::Overflow)?;
                    next.ledger.recipe_output_units_produced = next
                        .ledger
                        .recipe_output_units_produced
                        .checked_add(r.outputs.iter().try_fold(0_u64, |sum, output| {
                            sum.checked_add(u64::from(output.quantity))
                                .ok_or(CoreError::Overflow)
                        })?)
                        .ok_or(CoreError::Overflow)?;
                    if !r.outputs.is_empty() {
                        next.ledger.processor_input_cost =
                            next.ledger.processor_input_cost.checked_add(input_cost)?;
                        next.ledger.processor_operating_energy = next
                            .ledger
                            .processor_operating_energy
                            .checked_add(r.operating_energy)?;
                    }
                    events.push(GameEvent::Produced {
                        system: id.0.clone(),
                        recipe: r.id.clone(),
                    });
                }
            }
            updates.push((e, next));
        }
        for (e, m) in updates {
            *self.world.get_mut::<Market>(e).unwrap() = m;
        }
        self.world.resource_mut::<EventBuffer>().0.extend(events);
        Ok(())
    }

    fn create_reservation(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
        good: &ContentId,
        requested: u32,
    ) -> Result<u32, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        let trader = self.world.get::<Trader>(trader_entity).unwrap();
        self.ensure_carrier_contract_free(&trader_id, trader)?;
        let market_entity = self.market_entity(destination)?;
        let (price, quantity, total) = {
            let m = self.world.get::<Market>(market_entity).unwrap();
            let p = self.world.get::<MarketPolicy>(market_entity).unwrap();
            let price = self.bid_quote(m, p, good)?;
            if price == Energy::ZERO {
                return Err(CoreError::Unfunded);
            }
            let q = self.funded_quantity_with_preload_claims(
                destination,
                m,
                p,
                requested,
                price,
                FundingProtection {
                    released_ordinary_claim: Energy::ZERO,
                    protect_liquidation_budget: true,
                },
            )?;
            if q == 0 {
                return Err(CoreError::Unfunded);
            }
            let total = price.checked_mul(u64::from(q))?;
            (price, q, total)
        };
        let ttl = u64::from(self.world.resource::<EconomyConfig>().reservation_ttl);
        let expires = self.tick().checked_add(ttl).ok_or(CoreError::Overflow)?;
        let mut reservations = self.world.resource::<Reservations>().clone();
        let id = reservations.next_id;
        reservations.next_id = id.checked_add(1).ok_or(CoreError::Overflow)?;
        reservations.entries.insert(
            id,
            TradeReservation {
                id,
                trader: trader_id.clone(),
                destination: destination.clone(),
                good: good.clone(),
                quantity,
                remaining_quantity: quantity,
                reserved_energy: total,
                floor_unit_price: price,
                expires_at_tick: expires,
                status: ReservationStatus::Active,
            },
        );
        let mut market = self.world.get::<Market>(market_entity).unwrap().clone();
        let mut trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        market.reserved_energy = market.reserved_energy.checked_add(total)?;
        trader.reservation = Some(id);
        *self.world.resource_mut::<Reservations>() = reservations;
        *self.world.get_mut::<Market>(market_entity).unwrap() = market;
        *self.world.get_mut::<Trader>(trader_entity).unwrap() = trader;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::ReservationCreated {
                reservation: id,
                trader: trader_id,
                destination: destination.clone(),
                good: good.clone(),
                quantity,
                reserved_energy: total,
            });
        Ok(quantity)
    }
    fn release_reservation(&mut self, id: u64, status: ReservationStatus) -> Result<(), CoreError> {
        let reservation = self
            .world
            .resource::<Reservations>()
            .entries
            .get(&id)
            .cloned()
            .ok_or(CoreError::ReservationNotFound)?;
        if reservation.status != ReservationStatus::Active {
            return Ok(());
        }
        let market = self.market_entity(&reservation.destination)?;
        let released = reservation.reserved_energy;
        let next_reserved = self
            .world
            .get::<Market>(market)
            .unwrap()
            .reserved_energy
            .checked_sub(released)?;
        self.world
            .get_mut::<Market>(market)
            .unwrap()
            .reserved_energy = next_reserved;
        {
            let mut reservations = self.world.resource_mut::<Reservations>();
            let entry = reservations.entries.get_mut(&id).unwrap();
            entry.status = status;
            entry.reserved_energy = Energy::ZERO;
        }
        for mut trader in self.world.query::<&mut Trader>().iter_mut(&mut self.world) {
            if trader.reservation == Some(id) {
                trader.reservation = None;
            }
        }
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::ReservationReleased {
                reservation: id,
                status,
                released_energy: released,
            });
        Ok(())
    }
    fn cancel_trader_reservation(
        &mut self,
        e: Entity,
        status: ReservationStatus,
    ) -> Result<(), CoreError> {
        let id = self
            .world
            .get::<Trader>(e)
            .unwrap()
            .reservation
            .ok_or(CoreError::ReservationNotFound)?;
        self.release_reservation(id, status)
    }
    fn refresh_reservation(&mut self, id: u64) -> Result<(), CoreError> {
        let expires = self
            .tick()
            .checked_add(u64::from(
                self.world.resource::<EconomyConfig>().reservation_ttl,
            ))
            .ok_or(CoreError::Overflow)?;
        let mut reservations = self.world.resource_mut::<Reservations>();
        let r = reservations
            .entries
            .get_mut(&id)
            .ok_or(CoreError::ReservationNotFound)?;
        if r.status == ReservationStatus::Active {
            r.expires_at_tick = expires;
        }
        Ok(())
    }
    fn refresh_enroute_reservations(&mut self) -> Result<(), CoreError> {
        let mut ids = self
            .world
            .query::<&Trader>()
            .iter(&self.world)
            .filter(|trader| trader.travel.is_some())
            .filter_map(|trader| trader.reservation)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            self.refresh_reservation(id)?;
        }
        Ok(())
    }

    fn expire_reservations(&mut self) -> Result<(), CoreError> {
        let tick = self.tick();
        let ids = self
            .world
            .resource::<Reservations>()
            .entries
            .values()
            .filter(|r| r.status == ReservationStatus::Active && r.expires_at_tick <= tick)
            .map(|r| r.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.release_reservation(id, ReservationStatus::Expired)?;
        }
        Ok(())
    }
    fn settle_reservation(&mut self, e: Entity, id: u64) -> Result<(), CoreError> {
        let r = self
            .world
            .resource::<Reservations>()
            .entries
            .get(&id)
            .cloned()
            .ok_or(CoreError::ReservationNotFound)?;
        if r.status != ReservationStatus::Active {
            return Ok(());
        }
        if r.good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let system = self.world.get::<Trader>(e).unwrap().system.clone();
        if system != r.destination {
            return Ok(());
        }
        if r.floor_unit_price.0 <= 0 {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let trader = self.world.get::<Trader>(e).unwrap();
        let trader_id = self.world.get::<StableId>(e).unwrap();
        self.ensure_carrier_contract_free(&trader_id.0, trader)?;
        let cargo = trader.cargo.get(&r.good).copied().unwrap_or(0);
        let headroom = trader
            .energy_tank_capacity
            .checked_sub(trader.energy_tank)?;
        let market_entity = self.market_entity(&system)?;
        let market = self.world.get::<Market>(market_entity).unwrap();
        let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
        let quantity = self
            .funded_quantity_with_preload_claims(
                &system,
                market,
                policy,
                r.remaining_quantity,
                r.floor_unit_price,
                FundingProtection {
                    released_ordinary_claim: r.reserved_energy,
                    protect_liquidation_budget: true,
                },
            )?
            .min(u32::try_from(cargo).unwrap_or(u32::MAX))
            .min(u32::try_from(headroom.0 / r.floor_unit_price.0).unwrap_or(u32::MAX));
        if quantity > 0 {
            let total = r.floor_unit_price.checked_mul(u64::from(quantity))?;
            self.execute_funded_sale(
                e,
                market_entity,
                &r.good,
                quantity,
                SaleTerms {
                    unit_price: r.floor_unit_price,
                    reserved_release: total,
                    partial: quantity < r.remaining_quantity,
                },
            )?;
            let mut reservations = self.world.resource_mut::<Reservations>();
            let entry = reservations.entries.get_mut(&id).unwrap();
            entry.remaining_quantity -= quantity;
            entry.reserved_energy = entry.reserved_energy.checked_sub(total)?;
        }
        self.release_reservation(id, ReservationStatus::Fulfilled)
    }

    fn enqueue_commit_request(
        &mut self,
        trader: Entity,
        destination: &ContentId,
        good: &ContentId,
        quantity: u32,
        buy_at_origin: bool,
        command_driven: bool,
    ) -> Result<(), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let state = self.world.get::<Trader>(trader).unwrap();
        let trader_id = self.world.get::<StableId>(trader).unwrap().0.clone();
        self.ensure_carrier_contract_free(&trader_id, state)?;
        if state.travel.is_some() {
            return Err(CoreError::InTransit);
        }
        if &state.system == destination {
            return Err(CoreError::AlreadyThere);
        }
        if !self.world.resource::<Catalog>().goods.contains_key(good) {
            return Err(CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            });
        }
        let origin = state.system.clone();
        let burn_per_distance = state.travel_burn_per_distance;
        let speed = state.speed;
        let (route, distance) = self
            .graph()
            .shortest_path(&origin, destination)
            .ok_or(CoreError::NoRoute)?;
        let burn = route_travel_energy(self.graph(), &route, burn_per_distance)?;
        let origin_entity = self.market_entity(&origin)?;
        let destination_entity = self.market_entity(destination)?;
        let ask = if buy_at_origin {
            self.ask_quote(
                self.world.get::<Market>(origin_entity).unwrap(),
                self.world.get::<MarketPolicy>(origin_entity).unwrap(),
                good,
            )?
        } else {
            Energy::ZERO
        };
        let bid = self.bid_quote(
            self.world.get::<Market>(destination_entity).unwrap(),
            self.world.get::<MarketPolicy>(destination_entity).unwrap(),
            good,
        )?;
        let gross = Energy(bid.0.checked_sub(ask.0).ok_or(CoreError::Overflow)?)
            .checked_mul(u64::from(quantity))?;
        let net = gross.checked_sub(burn)?;
        let ticks = i128::from(ticks_for_distance(distance, speed));
        let score = i128::from(net.0)
            .checked_mul(1_000_000)
            .ok_or(CoreError::Overflow)?
            / ticks;
        self.world
            .resource_mut::<PendingTradeRequests>()
            .0
            .push(PendingTradeRequest {
                score,
                trader_id,
                trader,
                destination: destination.clone(),
                good: good.clone(),
                quantity,
                buy_at_origin,
                command_driven,
            });
        Ok(())
    }

    fn resolve_pending_trade_requests(&mut self) -> Result<(), CoreError> {
        let mut requests = std::mem::take(&mut self.world.resource_mut::<PendingTradeRequests>().0);
        requests.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.trader_id.cmp(&b.trader_id))
                .then_with(|| a.good.cmp(&b.good))
                .then_with(|| a.destination.cmp(&b.destination))
                .then_with(|| b.buy_at_origin.cmp(&a.buy_at_origin))
        });
        let mut handled = BTreeSet::new();
        for request in requests {
            if handled.contains(&request.trader_id) {
                continue;
            }
            let result = if request.buy_at_origin {
                self.commit_and_depart(
                    request.trader,
                    &request.destination,
                    &request.good,
                    request.quantity,
                )
            } else {
                self.create_reservation(
                    request.trader,
                    &request.destination,
                    &request.good,
                    request.quantity,
                )
                .and_then(|_| self.begin_travel(request.trader, &request.destination))
            };
            match result {
                Ok(()) => {
                    handled.insert(request.trader_id);
                }
                Err(error) => {
                    // Buy-at-origin commitments are prepared without mutation,
                    // so they have nothing to roll back on failure. The laden
                    // reroute path still creates its reservation separately.
                    if !request.buy_at_origin
                        && let Some(id) = self
                            .world
                            .get::<Trader>(request.trader)
                            .unwrap()
                            .reservation
                    {
                        self.release_reservation(id, ReservationStatus::Cancelled)?;
                    }
                    if request.command_driven {
                        self.world
                            .resource_mut::<EventBuffer>()
                            .0
                            .push(GameEvent::Rejected(error.to_string()));
                    }
                }
            }
        }
        Ok(())
    }

    fn commit_and_depart(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
        good: &ContentId,
        requested: u32,
    ) -> Result<(), CoreError> {
        let prepared =
            self.prepare_trade_commitment(trader_entity, destination, good, requested)?;
        *self.world.resource_mut::<Reservations>() = prepared.reservations;
        *self
            .world
            .get_mut::<Market>(prepared.origin_entity)
            .unwrap() = prepared.origin_market;
        *self
            .world
            .get_mut::<Market>(prepared.destination_entity)
            .unwrap() = prepared.destination_market;
        *self
            .world
            .get_mut::<Trader>(prepared.trader_entity)
            .unwrap() = prepared.trader;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .extend(prepared.events);
        Ok(())
    }

    /// Calculates the complete reservation, origin purchase, route burn, ledger,
    /// and event result before any ECS component or resource is changed.
    fn prepare_trade_commitment(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
        good: &ContentId,
        requested: u32,
    ) -> Result<PreparedTradeCommitment, CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        if requested == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let original_trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
        self.ensure_carrier_contract_free(&trader_id, &original_trader)?;
        if original_trader.travel.is_some() {
            return Err(CoreError::InTransit);
        }
        let origin = original_trader.system.clone();
        if &origin == destination {
            return Err(CoreError::AlreadyThere);
        }
        let used = Self::cargo_used(&original_trader)?;
        let bay = u64::from(original_trader.cargo_capacity)
            .checked_sub(used)
            .ok_or(CoreError::InsufficientCapacity)?;
        let tank = original_trader.energy_tank;
        let tank_capacity = original_trader.energy_tank_capacity;
        let burn_per_distance = original_trader.travel_burn_per_distance;
        let speed = original_trader.speed;

        let origin_entity = self.market_entity(&origin)?;
        let destination_entity = self.market_entity(destination)?;
        let mut origin_market = self.world.get::<Market>(origin_entity).unwrap().clone();
        let mut destination_market = self
            .world
            .get::<Market>(destination_entity)
            .unwrap()
            .clone();
        let origin_policy = self.world.get::<MarketPolicy>(origin_entity).unwrap();
        let destination_policy = self.world.get::<MarketPolicy>(destination_entity).unwrap();
        let ask = self.ask_quote(&origin_market, origin_policy, good)?;
        let bid = self.bid_quote(&destination_market, destination_policy, good)?;
        if bid == Energy::ZERO {
            return Err(CoreError::Unfunded);
        }
        let available = origin_market.inventory.get(good).copied().unwrap_or(0);
        let (route, _) = self
            .graph()
            .shortest_path(&origin, destination)
            .ok_or(CoreError::NoRoute)?;
        let travel_burn = route_travel_energy(self.graph(), &route, burn_per_distance)?;
        if tank < travel_burn {
            return Err(CoreError::InsufficientEnergy);
        }
        let affordable =
            u32::try_from(tank.checked_sub(travel_burn)?.0 / ask.0).unwrap_or(u32::MAX);
        let settlement_headroom = tank_capacity.checked_sub(tank)?.checked_add(travel_burn)?;
        let profitable_headroom = if bid > ask {
            u32::try_from(settlement_headroom.0 / (bid.0 - ask.0)).unwrap_or(u32::MAX)
        } else {
            u32::MAX
        };
        let candidate_quantity = requested
            .min(u32::try_from(available.min(bay)).unwrap_or(u32::MAX))
            .min(affordable)
            .min(profitable_headroom);
        if candidate_quantity == 0 {
            return Err(CoreError::InsufficientTankCapacity);
        }

        let quantity = self.funded_quantity_with_preload_claims(
            destination,
            &destination_market,
            destination_policy,
            candidate_quantity,
            bid,
            FundingProtection {
                released_ordinary_claim: Energy::ZERO,
                protect_liquidation_budget: true,
            },
        )?;
        if quantity == 0 {
            return Err(CoreError::Unfunded);
        }
        let reserved_energy = bid.checked_mul(u64::from(quantity))?;
        let purchase_total = ask.checked_mul(u64::from(quantity))?;
        let required_tank = purchase_total.checked_add(travel_burn)?;
        if tank < required_tank {
            return Err(CoreError::InsufficientEnergy);
        }

        let mut reservations = self.world.resource::<Reservations>().clone();
        let reservation_id = reservations.next_id;
        reservations.next_id = reservation_id.checked_add(1).ok_or(CoreError::Overflow)?;
        let expires_at_tick = self
            .tick()
            .checked_add(u64::from(
                self.world.resource::<EconomyConfig>().reservation_ttl,
            ))
            .ok_or(CoreError::Overflow)?;
        reservations.entries.insert(
            reservation_id,
            TradeReservation {
                id: reservation_id,
                trader: trader_id.clone(),
                destination: destination.clone(),
                good: good.clone(),
                quantity,
                remaining_quantity: quantity,
                reserved_energy,
                floor_unit_price: bid,
                expires_at_tick,
                status: ReservationStatus::Active,
            },
        );
        destination_market.reserved_energy = destination_market
            .reserved_energy
            .checked_add(reserved_energy)?;

        let quantity_u64 = u64::from(quantity);
        if available < quantity_u64 {
            return Err(CoreError::InsufficientStock);
        }
        let cargo_cost = origin_market
            .cost_basis
            .get(good)
            .copied()
            .unwrap_or_default()
            .removal_cost(quantity_u64)?;
        let initial_market_energy = origin_market.energy_stock()?;
        let next_market_energy = initial_market_energy.checked_add(purchase_total)?;
        if next_market_energy > origin_market.energy_storage_cap {
            return Err(CoreError::InsufficientCapacity);
        }
        let mut trader = original_trader;
        let next_cargo = trader
            .cargo
            .get(good)
            .copied()
            .unwrap_or(0)
            .checked_add(quantity_u64)
            .ok_or(CoreError::Overflow)?;
        *origin_market
            .inventory
            .get_mut(good)
            .ok_or(CoreError::InsufficientStock)? -= quantity_u64;
        origin_market
            .cost_basis
            .get_mut(good)
            .ok_or(CoreError::InsufficientStock)?
            .remove(quantity_u64)?;
        origin_market.set_energy_stock(next_market_energy)?;
        origin_market.ledger.energy_received_from_traders = origin_market
            .ledger
            .energy_received_from_traders
            .checked_add(purchase_total)?;
        origin_market.ledger.units_sold_to_traders = origin_market
            .ledger
            .units_sold_to_traders
            .checked_add(quantity_u64)
            .ok_or(CoreError::Overflow)?;
        if origin_market.recipes.iter().any(|recipe_id| {
            self.world
                .resource::<Catalog>()
                .recipes
                .get(recipe_id)
                .is_some_and(|recipe| recipe.outputs.iter().any(|output| output.good == *good))
        }) {
            origin_market.ledger.processor_output_revenue = origin_market
                .ledger
                .processor_output_revenue
                .checked_add(purchase_total)?;
        }
        origin_market.energy_flow.tank_to_market = origin_market
            .energy_flow
            .tank_to_market
            .checked_add(purchase_total)?;
        trader.energy_tank = tank.checked_sub(required_tank)?;
        trader.ledger.travel_cost = trader.ledger.travel_cost.checked_add(travel_burn)?;
        trader.cargo.insert(good.clone(), next_cargo);
        trader
            .cargo_cost_basis
            .entry(good.clone())
            .or_default()
            .add(quantity_u64, cargo_cost)?;
        trader.ledger.purchase_cost = trader.ledger.purchase_cost.checked_add(purchase_total)?;
        trader.ledger.completed_transactions = trader
            .ledger
            .completed_transactions
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        trader.reservation = Some(reservation_id);
        trader.travel = Some(TravelPlan {
            destination: destination.clone(),
            route: route.clone(),
            next_leg: 1,
            remaining_ticks: ticks_for_distance(self.graph().route_distance(&route[..2]), speed),
        });
        origin_market.energy_flow.travel_burned = origin_market
            .energy_flow
            .travel_burned
            .checked_add(travel_burn)?;

        Ok(PreparedTradeCommitment {
            trader_entity,
            origin_entity,
            destination_entity,
            trader,
            origin_market,
            destination_market,
            reservations,
            events: vec![
                GameEvent::ReservationCreated {
                    reservation: reservation_id,
                    trader: trader_id.clone(),
                    destination: destination.clone(),
                    good: good.clone(),
                    quantity,
                    reserved_energy,
                },
                GameEvent::Bought {
                    trader: trader_id.clone(),
                    good: good.clone(),
                    quantity,
                    total: purchase_total,
                },
                GameEvent::Departed {
                    trader: trader_id,
                    destination: destination.clone(),
                    travel_burn,
                },
            ],
        })
    }
    fn expected_sale_insufficiency(error: &CoreError) -> bool {
        matches!(
            error,
            CoreError::Unfunded
                | CoreError::InsufficientEnergy
                | CoreError::InsufficientStock
                | CoreError::InsufficientTankCapacity
                | CoreError::InsufficientCapacity
        )
    }

    fn record_sale_deferred(&mut self, e: Entity, good: &ContentId, error: &CoreError) {
        let trader = self.world.get::<StableId>(e).unwrap().0.clone();
        let tick = self.tick();
        let dynamic_mode = matches!(
            &self.world.resource::<FleetDynamics>().mode,
            Some(FleetMode::Dynamic { .. })
        );
        if dynamic_mode
            && let Some(mut lifecycle) = self.world.get_mut::<TraderLifecycle>(e)
            && lifecycle.last_failed_tick != Some(tick)
        {
            lifecycle.failed_liquidation_ticks =
                lifecycle.failed_liquidation_ticks.saturating_add(1);
            lifecycle.last_failed_tick = Some(tick);
        }
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::SaleDeferred {
                trader,
                good: good.clone(),
                reason: error.to_string(),
            });
    }

    fn settle_idle_laden(&mut self) -> Result<(), CoreError> {
        let mut traders = self
            .world
            .query::<(&StableId, Entity, &Trader, Option<&PlayerControlled>)>()
            .iter(&self.world)
            .filter(|(id, _, trader, _)| {
                trader.travel.is_none()
                    && !trader.cargo.is_empty()
                    && trader.bulk_energy.locked.is_none()
                    && !self.carrier_has_active_energy_contract(&id.0)
            })
            .map(|(id, entity, trader, player)| {
                (id.0.clone(), entity, trader.reservation, player.is_some())
            })
            .collect::<Vec<_>>();
        traders.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, e, reservation, player) in traders {
            if self
                .world
                .get::<Trader>(e)
                .unwrap()
                .cargo
                .keys()
                .any(|good| good.as_str() == ENERGY_ID)
            {
                self.world
                    .resource_mut::<EventBuffer>()
                    .0
                    .push(GameEvent::Rejected(
                        CoreError::EnergyNotTradable.to_string(),
                    ));
                continue;
            }
            if let Some(id) = reservation {
                self.settle_reservation(e, id)?;
            }
            let cargo = self
                .world
                .get::<Trader>(e)
                .unwrap()
                .cargo
                .iter()
                .next()
                .map(|(g, q)| (g.clone(), *q));
            if let Some((good, q)) = cargo {
                if player {
                    // Player cargo remains command-controlled. The universal
                    // anti-strand guarantee still liquidates the minimum when
                    // the tank cannot fund the cheapest adjacent jump.
                    if let Err(error) = self.liquidate_for_jump(e, &good) {
                        if Self::expected_sale_insufficiency(&error) {
                            self.record_sale_deferred(e, &good, &error);
                        } else {
                            return Err(error);
                        }
                    }
                    continue;
                }
                if let Err(error) =
                    self.local_sell(e, &good, u32::try_from(q).unwrap_or(u32::MAX), false)
                {
                    if Self::expected_sale_insufficiency(&error) {
                        self.record_sale_deferred(e, &good, &error);
                    } else {
                        return Err(error);
                    }
                }
                if self
                    .world
                    .get::<Trader>(e)
                    .unwrap()
                    .cargo
                    .contains_key(&good)
                    && let Err(error) = self.liquidate_for_jump(e, &good)
                {
                    if Self::expected_sale_insufficiency(&error) {
                        self.record_sale_deferred(e, &good, &error);
                    } else {
                        return Err(error);
                    }
                }
                if self
                    .world
                    .get::<Trader>(e)
                    .unwrap()
                    .cargo
                    .contains_key(&good)
                    && let Err(error) = self.reroute_laden(e, &good)
                {
                    if Self::expected_sale_insufficiency(&error) {
                        self.record_sale_deferred(e, &good, &error);
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }
    fn liquidate_for_jump(&mut self, e: Entity, good: &ContentId) -> Result<(), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let (system, tank, burn) = {
            let t = self.world.get::<Trader>(e).unwrap();
            (t.system.clone(), t.energy_tank, t.travel_burn_per_distance)
        };
        let nearest = self
            .graph()
            .neighbors(&system)
            .iter()
            .map(|(_, d)| *d)
            .min_by(f64::total_cmp)
            .ok_or(CoreError::NoRoute)?;
        let market = self.market_entity(&system)?;
        let policy = self.world.get::<MarketPolicy>(market).unwrap();
        let target = liquidation_target_energy(
            travel_energy(nearest, burn)?,
            policy.liquidation_threshold_percent,
        )?;
        let needed = target.checked_sub(tank)?;
        if needed.0 <= 0 {
            return Ok(());
        }
        let reference = self
            .world
            .resource::<Catalog>()
            .goods
            .get(good)
            .ok_or_else(|| CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            })?
            .bootstrap_cost;
        let liquidation = liquidation_unit_price(reference, policy.liquidation_discount_percent)?;
        let quantity = u32::try_from((needed.0 + liquidation.0 - 1) / liquidation.0)
            .map_err(|_| CoreError::Overflow)?;
        self.local_sell(e, good, quantity, true).map(|_| ())
    }

    /// Deterministically sends remaining cargo to the best funded market. If no
    /// destination can currently reserve it, keep the trader moving over the
    /// cheapest adjacent jump so every market gets another liquidation chance.
    fn reroute_laden(&mut self, e: Entity, good: &ContentId) -> Result<(), CoreError> {
        if good.as_str() == ENERGY_ID {
            return Err(CoreError::EnergyNotTradable);
        }
        let (origin, tank, capacity, burn_per_distance, cargo) = {
            let trader = self.world.get::<Trader>(e).unwrap();
            let trader_id = self.world.get::<StableId>(e).unwrap();
            self.ensure_carrier_contract_free(&trader_id.0, trader)?;
            (
                trader.system.clone(),
                trader.energy_tank,
                trader.energy_tank_capacity,
                trader.travel_burn_per_distance,
                trader.cargo.get(good).copied().unwrap_or(0),
            )
        };
        if cargo == 0 {
            return Ok(());
        }
        let graph = self.graph().clone();
        let mut candidates = Vec::new();
        let markets = self
            .world
            .query_filtered::<(&StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, market, policy)| (id.0.clone(), market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        for (destination, market, policy) in markets {
            if destination == origin {
                continue;
            }
            let Some((route, _)) = graph.shortest_path(&origin, &destination) else {
                continue;
            };
            let burn = route_travel_energy(&graph, &route, burn_per_distance)?;
            if tank < burn {
                continue;
            }
            let bid = self.bid_quote(&market, &policy, good)?;
            if bid == Energy::ZERO {
                continue;
            }
            let funded = self.funded_quantity_with_preload_claims(
                &destination,
                &market,
                &policy,
                u32::try_from(cargo).unwrap_or(u32::MAX),
                bid,
                FundingProtection {
                    released_ordinary_claim: Energy::ZERO,
                    protect_liquidation_budget: true,
                },
            )?;
            let arrival_headroom = capacity.checked_sub(tank.checked_sub(burn)?)?;
            let quantity =
                funded.min(u32::try_from(arrival_headroom.0 / bid.0).unwrap_or(u32::MAX));
            if quantity == 0 {
                continue;
            }
            let payout = bid.checked_mul(u64::from(quantity))?;
            let score = i128::from(payout.0 - burn.0);
            candidates.push((score, destination, quantity));
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        if !candidates.is_empty() {
            let trader_id = self.world.get::<StableId>(e).unwrap().0.clone();
            let mut pending = self.world.resource_mut::<PendingTradeRequests>();
            for (score, destination, quantity) in candidates {
                pending.0.push(PendingTradeRequest {
                    score,
                    trader_id: trader_id.clone(),
                    trader: e,
                    destination,
                    good: good.clone(),
                    quantity,
                    buy_at_origin: false,
                    command_driven: false,
                });
            }
            return Ok(());
        }

        let destination = graph
            .neighbors(&origin)
            .iter()
            .filter_map(|(id, distance)| {
                let burn = travel_energy(*distance, burn_per_distance).ok()?;
                (tank >= burn).then_some((*distance, id.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, id)| id)
            .ok_or(CoreError::InsufficientEnergy)?;
        self.begin_travel(e, &destination)
    }

    fn ordinary_npc_opportunities(
        &self,
        carrier: &Trader,
        markets: &[(ContentId, Market, MarketPolicy)],
    ) -> Result<Vec<OrdinaryNpcOpportunity>, CoreError> {
        let origin = markets
            .iter()
            .find(|(system, _, _)| system == &carrier.system)
            .ok_or(CoreError::InvalidPhysicalDefinition)?;
        let mut opportunities = Vec::new();
        for (good, stock) in &origin.1.inventory {
            if good.as_str() == ENERGY_ID || *stock == 0 {
                continue;
            }
            let ask = self.ask_quote(&origin.1, &origin.2, good)?;
            for (destination, market, policy) in markets {
                if destination == &carrier.system {
                    continue;
                }
                let bid = self.bid_quote(market, policy, good)?;
                if bid <= ask {
                    continue;
                }
                let Some((route, distance)) =
                    self.graph().shortest_path(&carrier.system, destination)
                else {
                    continue;
                };
                let burn =
                    route_travel_energy(self.graph(), &route, carrier.travel_burn_per_distance)?;
                if carrier.energy_tank < burn {
                    continue;
                }
                let affordable = u32::try_from(carrier.energy_tank.checked_sub(burn)?.0 / ask.0)
                    .unwrap_or(u32::MAX);
                let requested = u32::try_from((*stock).min(u64::from(carrier.cargo_capacity)))
                    .unwrap_or(u32::MAX)
                    .min(affordable);
                let quantity = self.funded_quantity_with_preload_claims(
                    destination,
                    market,
                    policy,
                    requested,
                    bid,
                    FundingProtection {
                        released_ordinary_claim: Energy::ZERO,
                        protect_liquidation_budget: true,
                    },
                )?;
                let Some(score) = profitable_opportunity_score(
                    bid,
                    ask,
                    quantity,
                    burn,
                    distance,
                    carrier.speed,
                )?
                else {
                    continue;
                };
                if score <= 0 {
                    continue;
                }
                opportunities.push(OrdinaryNpcOpportunity {
                    score,
                    source: carrier.system.clone(),
                    destination: destination.clone(),
                    good: good.clone(),
                    quantity,
                });
            }
        }
        Ok(opportunities)
    }

    fn hypothetical_dynamic_trader(archetype: &FleetArchetype, source: &ContentId) -> Trader {
        Trader {
            system: source.clone(),
            archetype: Some(archetype.id.clone()),
            energy_tank: archetype.starting_tank,
            energy_tank_capacity: archetype.energy_tank_capacity,
            bulk_energy_capacity: archetype.bulk_energy_capacity,
            bulk_energy: BulkEnergyHold::default(),
            cargo: BTreeMap::new(),
            cargo_cost_basis: BTreeMap::new(),
            cargo_capacity: archetype.cargo_capacity,
            speed: archetype.speed,
            travel_burn_per_distance: archetype.travel_burn_per_distance,
            refuel_policy: archetype.refuel_policy,
            travel: None,
            reservation: None,
            ledger: TradeLedger::default(),
        }
    }

    fn dynamic_candidates_for_archetype_source(
        &mut self,
        archetype: &FleetArchetype,
        source: &ContentId,
        projected_source_market: &Market,
        ordinary_markets: &[(ContentId, Market, MarketPolicy)],
    ) -> Result<Vec<DynamicSpawnCandidate>, CoreError> {
        let hypothetical = Self::hypothetical_dynamic_trader(archetype, source);
        let mut candidates = self
            .ordinary_npc_opportunities(&hypothetical, ordinary_markets)?
            .into_iter()
            .map(|opportunity| DynamicSpawnCandidate {
                score: opportunity.score,
                opportunity: DynamicOpportunityKey {
                    kind: DynamicOpportunityKind::OrdinaryTrade,
                    source: opportunity.source,
                    destination: opportunity.destination,
                    good: Some(opportunity.good),
                },
                archetype: archetype.id.clone(),
            })
            .collect::<Vec<_>>();
        candidates.extend(
            self.hypothetical_energy_contract_opportunities(
                &archetype.id,
                &hypothetical,
                projected_source_market,
            )?
            .into_iter()
            .map(|opportunity| DynamicSpawnCandidate {
                score: opportunity.score,
                opportunity: DynamicOpportunityKey {
                    kind: DynamicOpportunityKind::EnergyContract,
                    source: opportunity.source,
                    destination: opportunity.destination,
                    good: None,
                },
                archetype: archetype.id.clone(),
            }),
        );
        Ok(candidates)
    }

    fn derive_dynamic_fleet_opportunity(
        &mut self,
        selected_opportunities: &BTreeSet<DynamicOpportunityKey>,
    ) -> Result<(u64, Option<DynamicSpawnCandidate>), CoreError> {
        let mut markets = self
            .world
            .query_filtered::<(Entity, &StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(entity, stable, market, policy)| {
                (entity, stable.0.clone(), market.clone(), policy.clone())
            })
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| left.1.cmp(&right.1));
        let ordinary_markets = markets
            .iter()
            .map(|(_, stable, market, policy)| (stable.clone(), market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        let counts = self
            .world
            .query_filtered::<&Trader, Without<PlayerControlled>>()
            .iter(&self.world)
            .filter_map(|trader| trader.archetype.clone())
            .fold(BTreeMap::<ContentId, usize>::new(), |mut counts, id| {
                *counts.entry(id).or_default() += 1;
                counts
            });
        let archetypes = self
            .world
            .resource::<FleetDynamics>()
            .archetypes
            .values()
            .filter(|archetype| {
                counts.get(&archetype.id).copied().unwrap_or(0) < archetype.maximum_count
            })
            .cloned()
            .collect::<Vec<_>>();

        let mut best_by_opportunity =
            BTreeMap::<DynamicOpportunityKey, DynamicSpawnCandidate>::new();
        for archetype in archetypes {
            for (source_entity, source, _, _) in &markets {
                let Some(projected_source_market) = self
                    .projected_market_after_exportable_withdrawal(
                        *source_entity,
                        archetype.starting_tank,
                    )?
                else {
                    continue;
                };
                for candidate in self.dynamic_candidates_for_archetype_source(
                    &archetype,
                    source,
                    &projected_source_market,
                    &ordinary_markets,
                )? {
                    retain_best_dynamic_candidate(&mut best_by_opportunity, candidate);
                }
            }
        }

        // One canonical row per underlying route prevents archetype variants
        // from inflating demand. Only routes actually selected by compatible
        // idle carriers consume their matching canonical backlog row.
        let mut unserved = best_by_opportunity
            .into_values()
            .filter(|candidate| !selected_opportunities.contains(&candidate.opportunity))
            .collect::<Vec<_>>();
        unserved.sort_by(DynamicSpawnCandidate::selection_order);
        let highest = unserved.first().cloned();
        let system_count = u64::try_from(markets.len().max(1)).map_err(|_| CoreError::Overflow)?;
        let normalized = unserved.into_iter().try_fold(0_u64, |sum, candidate| {
            let normalized_score = u64::try_from(candidate.score / 1_000_000)
                .map_err(|_| CoreError::Overflow)?
                .max(1);
            sum.checked_add(normalized_score).ok_or(CoreError::Overflow)
        })? / system_count;
        Ok((normalized, highest))
    }

    fn dynamic_spawn_candidate_is_current(
        &mut self,
        candidate: &DynamicSpawnCandidate,
        archetype: &FleetArchetype,
        projected_source_market: &Market,
    ) -> Result<bool, CoreError> {
        let mut markets = self
            .world
            .query_filtered::<(&StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(stable, market, policy)| (stable.0.clone(), market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(self
            .dynamic_candidates_for_archetype_source(
                archetype,
                &candidate.opportunity.source,
                projected_source_market,
                &markets,
            )?
            .into_iter()
            .any(|current| {
                current.archetype == candidate.archetype
                    && current.opportunity == candidate.opportunity
                    && current.score > 0
            }))
    }

    fn evaluate_dynamic_fleet(&mut self) -> Result<(), CoreError> {
        let mode = self
            .world
            .resource::<FleetDynamics>()
            .mode
            .clone()
            .ok_or(CoreError::InvalidWorldDynamics)?;
        let FleetMode::Dynamic {
            opportunity_threshold,
            opportunity_window,
            spawn_cooldown_ticks,
            retirement_window,
            retirement_threshold,
            maximum_count,
            ..
        } = mode
        else {
            // Fixed mode is a strict lifecycle bypass: no profitability,
            // persistence, cooldown, spawn, retirement, or lifecycle events.
            return Ok(());
        };

        let mut ordered = self
            .world
            .query_filtered::<
                (Entity, &StableId, &Trader, &TraderLifecycle),
                Without<PlayerControlled>,
            >()
            .iter(&self.world)
            .map(|(entity, id, trader, lifecycle)| {
                (entity, id.0.clone(), trader.clone(), lifecycle.clone())
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.1.cmp(&b.1));

        let mut prepared_lifecycles = Vec::with_capacity(ordered.len());
        for (entity, id, trader, mut lifecycle) in ordered {
            let checked_delta = |current: Energy, observed: Energy| {
                if current.0 < 0 || observed.0 < 0 || current < observed {
                    return Err(CoreError::InvalidPhysicalDefinition);
                }
                current.checked_sub(observed)
            };
            let checked_subset = |total: Energy, subset: Energy| {
                if total.0 < 0 || subset.0 < 0 || subset > total {
                    return Err(CoreError::InvalidPhysicalDefinition);
                }
                total.checked_sub(subset)
            };

            trader.ledger.validate_contract_subsets()?;
            checked_subset(
                lifecycle.observed_sales_revenue,
                lifecycle.observed_contract_reimbursement,
            )?;
            checked_subset(
                lifecycle.observed_travel_cost,
                lifecycle.observed_reimbursed_travel_cost,
            )?;

            let purchases = checked_delta(
                trader.ledger.purchase_cost,
                lifecycle.observed_purchase_cost,
            )?;
            let sales = checked_delta(
                trader.ledger.sales_revenue,
                lifecycle.observed_sales_revenue,
            )?;
            let reimbursement = checked_delta(
                trader.ledger.contract_reimbursement,
                lifecycle.observed_contract_reimbursement,
            )?;
            let travel = checked_delta(trader.ledger.travel_cost, lifecycle.observed_travel_cost)?;
            let reimbursed_travel = checked_delta(
                trader.ledger.reimbursed_travel_cost,
                lifecycle.observed_reimbursed_travel_cost,
            )?;
            let earned_sales = checked_subset(sales, reimbursement)?;
            let unreimbursed_travel = checked_subset(travel, reimbursed_travel)?;
            let profit = i128::from(earned_sales.0)
                .checked_sub(i128::from(purchases.0))
                .and_then(|value| value.checked_sub(i128::from(unreimbursed_travel.0)))
                .ok_or(CoreError::Overflow)?;
            lifecycle
                .profitability
                .push(i64::try_from(profit).map_err(|_| CoreError::Overflow)?);
            let window = usize::try_from(retirement_window).map_err(|_| CoreError::Overflow)?;
            if lifecycle.profitability.len() > window {
                lifecycle.profitability.remove(0);
            }
            lifecycle.observed_purchase_cost = trader.ledger.purchase_cost;
            lifecycle.observed_sales_revenue = trader.ledger.sales_revenue;
            lifecycle.observed_contract_reimbursement = trader.ledger.contract_reimbursement;
            lifecycle.observed_travel_cost = trader.ledger.travel_cost;
            lifecycle.observed_reimbursed_travel_cost = trader.ledger.reimbursed_travel_cost;
            if trader.cargo.is_empty() {
                lifecycle.failed_liquidation_ticks = 0;
                lifecycle.last_failed_tick = None;
            }
            let sustained_loss = lifecycle.profitability.len() == window
                && lifecycle
                    .profitability
                    .iter()
                    .try_fold(0_i64, |sum, value| {
                        sum.checked_add(*value).ok_or(CoreError::Overflow)
                    })?
                    <= retirement_threshold;
            let retirement_blocked =
                trader.bulk_energy.locked.is_some() || self.carrier_has_active_energy_contract(&id);
            if lifecycle.retirement.is_none()
                && !retirement_blocked
                && (sustained_loss || lifecycle.failed_liquidation_ticks >= retirement_window)
            {
                lifecycle.retirement = Some(TraderRetirementState::CleaningUp);
            }
            prepared_lifecycles.push((entity, lifecycle));
        }
        for (entity, lifecycle) in prepared_lifecycles {
            *self.world.get_mut::<TraderLifecycle>(entity).unwrap() = lifecycle;
        }

        self.finish_deferred_retirements()?;

        let opportunity = self
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity;
        let current_persistence = self
            .world
            .resource::<FleetDynamics>()
            .opportunity_persistence;
        let persistence = update_opportunity_persistence(
            current_persistence,
            opportunity,
            opportunity_threshold,
        )?
        .min(opportunity_window);
        self.world
            .resource_mut::<FleetDynamics>()
            .opportunity_persistence = persistence;

        let active_count = self
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&self.world)
            .count();
        let cooldown_until = self.world.resource::<FleetDynamics>().spawn_cooldown_until;
        if active_count < maximum_count
            && persistence >= opportunity_window
            && self.tick() >= cooldown_until
        {
            // Validate cooldown arithmetic before the spawn mutates stock or ECS.
            let next_cooldown = self
                .tick()
                .checked_add(u64::from(spawn_cooldown_ticks))
                .ok_or(CoreError::Overflow)?;
            if self.spawn_dynamic_trader()? {
                let mut fleet = self.world.resource_mut::<FleetDynamics>();
                fleet.opportunity_persistence = 0;
                fleet.spawn_cooldown_until = next_cooldown;
            }
        }
        Ok(())
    }

    fn finish_deferred_retirements(&mut self) -> Result<(), CoreError> {
        let mut candidates = self
            .world
            .query_filtered::<
                (Entity, &StableId, &Trader, &TraderLifecycle),
                Without<PlayerControlled>,
            >()
            .iter(&self.world)
            .filter(|(_, _, _, lifecycle)| lifecycle.retirement.is_some())
            .map(|(entity, id, trader, _)| (id.0.clone(), entity, trader.clone()))
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));

        // Reservation cancellation is one cleanup phase. A candidate with a
        // reservation is deliberately reconsidered for physical retirement on
        // the next tick, after that cancellation has reconciled.
        let cancellations = candidates
            .iter()
            .filter_map(|(_, entity, trader)| trader.reservation.map(|_| *entity))
            .collect::<Vec<_>>();
        let mut market_updates = BTreeMap::<ContentId, (Entity, Market)>::new();
        let mut retirements = Vec::new();
        for (id, entity, trader) in candidates {
            let active_contract = self
                .world
                .resource::<EnergyContracts>()
                .active
                .values()
                .any(|contract| contract.carrier == id);
            if trader.reservation.is_some()
                || trader.travel.is_some()
                || !trader.cargo.is_empty()
                || trader.bulk_energy.used()?.0 > 0
                || active_contract
            {
                continue;
            }
            let market_entity = self.market_entity(&trader.system)?;
            let next_market = if let Some((_, staged)) = market_updates.get(&trader.system) {
                staged.clone()
            } else {
                self.world.get::<Market>(market_entity).unwrap().clone()
            };
            let mut next_market = next_market;
            let next_stock = next_market
                .energy_stock()?
                .checked_add(trader.energy_tank)?;
            if next_stock > next_market.energy_storage_cap {
                // Tank energy is physical. A full market delays retirement.
                continue;
            }
            next_market.set_energy_stock(next_stock)?;
            next_market.energy_flow.tank_to_market = next_market
                .energy_flow
                .tank_to_market
                .checked_add(trader.energy_tank)?;
            market_updates.insert(trader.system.clone(), (market_entity, next_market));
            retirements.push((id, entity, trader.system));
        }
        let retirement_count = u64::try_from(retirements.len()).map_err(|_| CoreError::Overflow)?;
        let next_retirement_count = self
            .world
            .resource::<AggregateDynamicsHistory>()
            .fleet_retirements
            .checked_add(retirement_count)
            .ok_or(CoreError::Overflow)?;

        // Every checked counter and physical next state is validated above.
        // Apply cleanup and prepared retirement mutations only afterward.
        for entity in cancellations {
            self.cancel_trader_reservation(entity, ReservationStatus::Cancelled)?;
        }
        for (_, (entity, next)) in market_updates {
            let mut market = self.world.get_mut::<Market>(entity).unwrap();
            market.inventory = next.inventory;
            market.energy_flow = next.energy_flow;
        }
        for (_, entity, _) in &retirements {
            let _ = self.world.despawn(*entity);
        }
        self.world
            .resource_mut::<AggregateDynamicsHistory>()
            .fleet_retirements = next_retirement_count;
        self.world.resource_mut::<EventBuffer>().0.extend(
            retirements
                .into_iter()
                .map(|(trader, _, system)| GameEvent::TraderRetired { trader, system }),
        );
        Ok(())
    }

    fn spawn_dynamic_trader(&mut self) -> Result<bool, CoreError> {
        let tick = self.tick();
        let (captured_for_phase, phase_candidate) = {
            let state = self.world.resource::<DynamicFleetOpportunityState>();
            if state.captured_tick == Some(tick) {
                (true, state.candidate.clone())
            } else {
                (false, None)
            }
        };
        // Phase 13 must preserve and use its exact phase-10 choice, including
        // an explicitly captured no-choice. Lazy derivation is only for direct
        // internal callers that have no capture for the current tick.
        let candidate = if captured_for_phase {
            phase_candidate
        } else {
            self.derive_dynamic_fleet_opportunity(&BTreeSet::new())?.1
        };
        let Some(candidate) = candidate else {
            return Ok(false);
        };

        let maximum_count = match &self.world.resource::<FleetDynamics>().mode {
            Some(FleetMode::Dynamic { maximum_count, .. }) => *maximum_count,
            _ => return Ok(false),
        };
        let active_count = self
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&self.world)
            .count();
        if active_count >= maximum_count {
            return Ok(false);
        }
        let counts = self
            .world
            .query_filtered::<&Trader, Without<PlayerControlled>>()
            .iter(&self.world)
            .filter_map(|trader| trader.archetype.clone())
            .fold(BTreeMap::<ContentId, usize>::new(), |mut counts, id| {
                *counts.entry(id).or_default() += 1;
                counts
            });
        let Some(archetype) = self
            .world
            .resource::<FleetDynamics>()
            .archetypes
            .get(&candidate.archetype)
            .cloned()
        else {
            return Ok(false);
        };
        if counts.get(&archetype.id).copied().unwrap_or(0) >= archetype.maximum_count {
            return Ok(false);
        }

        let system = candidate.opportunity.source.clone();
        let market_entity = match self.market_entity(&system) {
            Ok(entity)
                if self
                    .world
                    .get::<StableId>(entity)
                    .is_some_and(|stable| stable.0 == system) =>
            {
                entity
            }
            Ok(_) | Err(_) => return Ok(false),
        };
        let Some(mut market) = self
            .projected_market_after_exportable_withdrawal(market_entity, archetype.starting_tank)?
        else {
            return Ok(false);
        };
        if !self.dynamic_spawn_candidate_is_current(&candidate, &archetype, &market)? {
            return Ok(false);
        }

        let current_sequence = self.world.resource::<FleetDynamics>().spawn_sequence;
        let next_sequence = current_sequence.checked_add(1).ok_or(CoreError::Overflow)?;
        let id = dynamic_trader_id(&archetype, next_sequence)?;
        if self
            .world
            .query::<&StableId>()
            .iter(&self.world)
            .any(|stable| stable.0 == id)
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        market.energy_flow.market_to_tank = market
            .energy_flow
            .market_to_tank
            .checked_add(archetype.starting_tank)?;
        let trader = Self::hypothetical_dynamic_trader(&archetype, &system);
        let history = self
            .world
            .resource::<AggregateDynamicsHistory>()
            .fleet_spawns
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        // Every checked counter, sequence, ID, and physical next state is
        // complete before market, entity, history, or event mutation.
        *self.world.get_mut::<Market>(market_entity).unwrap() = market;
        self.world.spawn((
            StableId(id.clone()),
            DisplayName(format!("{} {next_sequence:08}", archetype.name_prefix)),
            trader,
            TraderLifecycle::default(),
        ));
        self.world.resource_mut::<FleetDynamics>().spawn_sequence = next_sequence;
        self.world
            .resource_mut::<AggregateDynamicsHistory>()
            .fleet_spawns = history;
        if captured_for_phase {
            self.world
                .resource_mut::<DynamicFleetOpportunityState>()
                .candidate = None;
        }
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::TraderSpawned { trader: id, system });
        Ok(true)
    }

    fn collect_automated_trader_requests(&mut self) -> Result<(), CoreError> {
        #[derive(Clone)]
        struct Request {
            score: i128,
            trader_id: ContentId,
            e: Entity,
            source: ContentId,
            good: ContentId,
            destination: ContentId,
            quantity: u32,
        }

        enum Opportunity {
            Energy(energy_logistics::NpcEnergyContractOpportunity),
            Ordinary(Request),
        }

        impl Opportunity {
            fn score(&self) -> i128 {
                match self {
                    Self::Energy(opportunity) => opportunity.score,
                    Self::Ordinary(request) => request.score,
                }
            }

            fn kind_rank(&self) -> u8 {
                match self {
                    Self::Energy(_) => 0,
                    Self::Ordinary(_) => 1,
                }
            }

            fn source(&self) -> &ContentId {
                match self {
                    Self::Energy(opportunity) => &opportunity.source,
                    Self::Ordinary(request) => &request.source,
                }
            }

            fn destination(&self) -> &ContentId {
                match self {
                    Self::Energy(opportunity) => &opportunity.destination,
                    Self::Ordinary(request) => &request.destination,
                }
            }

            fn good(&self) -> Option<&ContentId> {
                match self {
                    Self::Energy(_) => None,
                    Self::Ordinary(request) => Some(&request.good),
                }
            }

            fn carrier(&self) -> &ContentId {
                match self {
                    Self::Energy(opportunity) => &opportunity.carrier,
                    Self::Ordinary(request) => &request.trader_id,
                }
            }

            fn key(&self) -> DynamicOpportunityKey {
                DynamicOpportunityKey {
                    kind: match self {
                        Self::Energy(_) => DynamicOpportunityKind::EnergyContract,
                        Self::Ordinary(_) => DynamicOpportunityKind::OrdinaryTrade,
                    },
                    source: self.source().clone(),
                    destination: self.destination().clone(),
                    good: self.good().cloned(),
                }
            }
        }
        let markets = self
            .world
            .query_filtered::<(&StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, market, policy)| (id.0.clone(), market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        let mut requests = Vec::new();
        let mut eligible_carriers = Vec::new();
        for (entity, id, trader, lifecycle) in self
            .world
            .query_filtered::<
                (Entity, &StableId, &Trader, &TraderLifecycle),
                Without<PlayerControlled>,
            >()
            .iter(&self.world)
        {
            if trader.travel.is_some()
                || !trader.cargo.is_empty()
                || trader.bulk_energy.locked.is_some()
                || self.carrier_has_active_energy_contract(&id.0)
                || lifecycle.retirement.is_some()
            {
                continue;
            }
            eligible_carriers.push(id.0.clone());
            requests.extend(
                self.ordinary_npc_opportunities(trader, &markets)?
                    .into_iter()
                    .map(|opportunity| Request {
                        score: opportunity.score,
                        trader_id: id.0.clone(),
                        e: entity,
                        source: opportunity.source,
                        good: opportunity.good,
                        destination: opportunity.destination,
                        quantity: opportunity.quantity,
                    }),
            );
        }
        requests.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.trader_id.cmp(&b.trader_id))
                .then_with(|| a.good.cmp(&b.good))
                .then_with(|| a.destination.cmp(&b.destination))
        });
        let mut opportunities = requests
            .into_iter()
            .map(Opportunity::Ordinary)
            .collect::<Vec<_>>();
        for carrier in eligible_carriers {
            opportunities.extend(
                self.npc_energy_contract_opportunities(&carrier)?
                    .into_iter()
                    .map(Opportunity::Energy),
            );
        }
        opportunities.sort_by(|left, right| {
            right
                .score()
                .cmp(&left.score())
                .then_with(|| left.kind_rank().cmp(&right.kind_rank()))
                .then_with(|| left.source().cmp(right.source()))
                .then_with(|| left.destination().cmp(right.destination()))
                .then_with(|| left.good().cmp(&right.good()))
                .then_with(|| left.carrier().cmp(right.carrier()))
        });

        let mut selected_carriers = BTreeSet::new();
        let mut selected_opportunities = BTreeSet::new();
        let mut energy_intents = Vec::new();
        let mut ordinary_requests = Vec::new();
        for opportunity in opportunities {
            if !selected_carriers.insert(opportunity.carrier().clone()) {
                continue;
            }
            selected_opportunities.insert(opportunity.key());
            match opportunity {
                Opportunity::Energy(opportunity) => {
                    energy_intents.push(EnergyContractIntent {
                        carrier: opportunity.carrier,
                        source: opportunity.source,
                        destination: opportunity.destination,
                        gross_payload: opportunity.gross_payload,
                        command_driven: false,
                    });
                }
                Opportunity::Ordinary(request) => {
                    ordinary_requests.push(PendingTradeRequest {
                        score: request.score,
                        trader_id: request.trader_id,
                        trader: request.e,
                        destination: request.destination,
                        good: request.good,
                        quantity: request.quantity,
                        buy_at_origin: true,
                        command_driven: false,
                    });
                }
            }
        }

        let dynamic_capture = if matches!(
            &self.world.resource::<FleetDynamics>().mode,
            Some(FleetMode::Dynamic { .. })
        ) {
            Some(self.derive_dynamic_fleet_opportunity(&selected_opportunities)?)
        } else {
            None
        };
        if let Some((unserved, candidate)) = dynamic_capture {
            let captured_tick = self.tick();
            self.world
                .resource_mut::<FleetDynamics>()
                .normalized_unserved_opportunity = unserved;
            *self.world.resource_mut::<DynamicFleetOpportunityState>() =
                DynamicFleetOpportunityState {
                    captured_tick: Some(captured_tick),
                    candidate,
                };
        }

        self.world
            .resource_mut::<PendingEnergyContractIntents>()
            .0
            .extend(energy_intents);
        self.world
            .resource_mut::<PendingTradeRequests>()
            .0
            .extend(ordinary_requests);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
