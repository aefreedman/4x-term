//! Headless, deterministic simulation core for the physical energy economy.

use bevy_ecs::prelude::*;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt::{Display, Formatter};
use thiserror::Error;

pub const ENERGY_ID: &str = "core:energy";

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

#[derive(Clone, Debug)]
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
    pub emergency_energy_bid_ceiling: Energy,
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
            emergency_energy_bid_ceiling: Energy(10),
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
            || self.emergency_energy_bid_ceiling.0 <= 0
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PopulationState {
    pub current: u64,
    pub reference: u64,
    pub carrying_capacity: u64,
    pub sufficiency_samples: Vec<u32>,
    pub sufficiency_sum: u64,
    pub change_remainder: u64,
    pub growth_ticks: u64,
    pub decline_ticks: u64,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InvestmentState {
    pub levels: BTreeMap<InvestmentKind, u32>,
    pub cooldown_until: BTreeMap<InvestmentKind, u64>,
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

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct FleetDynamics {
    pub mode: Option<FleetMode>,
    /// Validated common NPC capability used by anti-strand protection even
    /// when dynamic mode currently has no active NPC trader.
    pub archetype_capability: Option<LiquidationTraderCapability>,
    pub opportunity_persistence: u32,
    pub spawn_sequence: u64,
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

impl MarketPolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.producer_margin_percent > 10_000
            || self.liquidation_threshold_percent < 100
            || self.liquidation_discount_percent > 100
            || self.default_target == 0
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
    /// Graph/content-compiled anti-strand reserve; never derived from policy knobs.
    pub protected_liquidation_budget: Energy,
    pub bootstrap_risk_acknowledged: bool,
}

#[derive(Clone, Debug)]
pub struct TraderDefinition {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
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

pub fn validate_population_config(config: &PopulationConfig) -> Result<(), CoreError> {
    let decline = u64::from(config.decline_per_thousand);
    let growth = u64::from(config.growth_per_thousand);
    if config.sufficiency_window == 0
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
            || (shape.enabled && (shape.maximum_level == 0 || shape.effect_per_level == 0))
        {
            return Err(CoreError::InvalidWorldDynamics);
        }
    }
    Ok(())
}

pub fn logistic_population_delta(
    population: u64,
    carrying_capacity: u64,
    rate_per_thousand: u32,
    scale: u32,
    remainder: &mut u64,
) -> Result<u64, CoreError> {
    if scale == 0 || carrying_capacity == 0 {
        return Err(CoreError::InvalidWorldDynamics);
    }
    let denominator = u128::from(carrying_capacity)
        .checked_mul(1_000)
        .and_then(|value| value.checked_mul(u128::from(scale)))
        .ok_or(CoreError::Overflow)?;
    if u128::from(*remainder) >= denominator {
        return Err(CoreError::InvalidWorldDynamics);
    }
    if population >= carrying_capacity {
        return Ok(0);
    }
    let numerator = u128::from(population)
        .checked_mul(u128::from(carrying_capacity - population))
        .and_then(|value| value.checked_mul(u128::from(rate_per_thousand)))
        .and_then(|value| value.checked_add(u128::from(*remainder)))
        .ok_or(CoreError::Overflow)?;
    let next_remainder = u64::try_from(numerator % denominator).map_err(|_| CoreError::Overflow)?;
    let delta = u64::try_from(numerator / denominator)
        .map_err(|_| CoreError::Overflow)?
        .min(carrying_capacity - population);
    *remainder = next_remainder;
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
    pub travel_burned: Energy,
    pub curtailed: Energy,
    pub market_to_tank: Energy,
    pub tank_to_market: Energy,
    pub market_to_energy_cargo: Energy,
    pub energy_cargo_to_market: Energy,
}

impl EnergyFlowLedger {
    pub fn net_external_delta(self) -> Result<Energy, CoreError> {
        self.external_inflow
            .checked_add(self.generated)?
            .checked_sub(self.life_support_burned)?
            .checked_sub(self.source_burned)?
            .checked_sub(self.production_burned)?
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
    pub travel_burned: WideEnergy,
    pub curtailed: WideEnergy,
    pub market_to_tank: WideEnergy,
    pub tank_to_market: WideEnergy,
    pub market_to_energy_cargo: WideEnergy,
    pub energy_cargo_to_market: WideEnergy,
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
        add!(travel_burned);
        add!(curtailed);
        add!(market_to_tank);
        add!(tank_to_market);
        add!(market_to_energy_cargo);
        add!(energy_cargo_to_market);
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
    pub targets: BTreeMap<ContentId, u32>,
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
    pub cargo_units_moved: u64,
    pub completed_transactions: u64,
}

#[derive(Component, Clone, Debug)]
pub struct Trader {
    pub system: ContentId,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    TickAdvanced(u64),
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
        reason: String,
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
    SetMarketPolicy {
        system: ContentId,
        policy: MarketPolicy,
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
    #[error("invalid market policy")]
    InvalidPolicy,
    #[error("player is not authorized to govern this market")]
    UnauthorizedMarketPolicy,
    #[error("invalid physical definition")]
    InvalidPhysicalDefinition,
    #[error("refuel policy forbids this transfer")]
    RefuelForbidden,
    #[error("no funded quantity available")]
    Unfunded,
    #[error("reservation not found")]
    ReservationNotFound,
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
    pub cost_basis: BTreeMap<ContentId, CostBasis>,
    pub ledger: MarketLedger,
    pub energy_flow: EnergyFlowLedger,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TraderSnapshot {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub energy_tank: Energy,
    pub energy_tank_capacity: Energy,
    pub cargo: BTreeMap<ContentId, u64>,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel_burn_per_distance: Energy,
    pub refuel_policy: RefuelPolicy,
    pub travel: Option<TravelPlan>,
    pub reservation: Option<u64>,
    pub ledger: TradeLedger,
    pub player: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CoreSnapshot {
    pub tick: u64,
    pub markets: Vec<MarketSnapshot>,
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
        validate_population_config(&definition.economy.population)?;
        validate_investment_shapes(&definition.economy.investments)?;
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
        for mut system in definition.systems {
            system.policy.validate()?;
            // Additive Slice-2 defaults keep older fixed-output/static-population
            // fixtures source-compatible when they adjust the legacy fields.
            if system.seasonal_generation.amplitude_percent == 0 {
                system.seasonal_generation.base_output = system.energy_output_per_tick;
                system.seasonal_generation.current_effective_output = system.energy_output_per_tick;
            }
            if world
                .resource::<EconomyConfig>()
                .population
                .static_population
            {
                system.population_state.current = system.population;
                system.population_state.reference = system.population.max(1);
                system.population_state.carrying_capacity = system
                    .population_state
                    .carrying_capacity
                    .max(system.population);
            }
            system.seasonal_generation.validate()?;
            let initial_effective_output = system.seasonal_generation.effective_output_at(0)?;
            system.seasonal_generation.current_effective_output = initial_effective_output;
            if system.energy_output_per_tick.0 < 0
                || system.energy_storage_cap.0 <= 0
                || system.protected_liquidation_budget.0 < 0
                || system.seasonal_generation.base_output != system.energy_output_per_tick
                || system.population_state.current != system.population
                || system.population_state.reference == 0
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
            world.spawn((
                StableId(system.id),
                DisplayName(system.name),
                SystemMarker,
                SpatialPosition(system.position),
                system.policy,
                Market {
                    inventory: system.inventory,
                    targets: system.targets,
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
                || trader.cargo_capacity == 0
                || !trader.speed.is_finite()
                || trader.speed <= 0.0
                || trader.travel_burn_per_distance.0 < 0
            {
                return Err(CoreError::InvalidPhysicalDefinition);
            }
            let mut e = world.spawn((
                StableId(trader.id),
                DisplayName(trader.name),
                Trader {
                    system: trader.system,
                    energy_tank: trader.energy_tank,
                    energy_tank_capacity: trader.energy_tank_capacity,
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
            if trader.player {
                e.insert(PlayerControlled);
            }
        }
        let mut session = Self { world };
        session.validate_emergency_energy_bid_ceiling()?;
        Ok(session)
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
        let result = match command {
            GameCommand::Buy { good, quantity } => {
                let e = self.player_entity()?;
                self.local_buy(e, &good, quantity)
            }
            GameCommand::Sell { good, quantity } => {
                let e = self.player_entity()?;
                self.local_sell(e, &good, quantity, false).map(|_| ())
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
                let e = self.player_entity()?;
                let system = self.world.get::<Trader>(e).unwrap().system.clone();
                if system != origin {
                    return Err(CoreError::WrongLocation);
                }
                self.enqueue_commit_request(e, &destination, &good, quantity, true, true)
            }
            GameCommand::DepositTank { amount } => {
                let e = self.player_entity()?;
                self.transfer_tank(e, amount, true)
            }
            GameCommand::WithdrawTank { amount } => {
                let e = self.player_entity()?;
                self.transfer_tank(e, amount, false)
            }
            GameCommand::SetMarketPolicy { system, policy } => {
                self.set_player_policy(&system, policy)
            }
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
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::Rejected(error.to_string()));
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
        self.refresh_enroute_reservations()?;
        self.expire_reservations()?;
        self.generate_and_life_support()?;
        self.classify_brownouts()?;
        self.execute_sources_and_recipes()?;
        self.settle_idle_laden()?;
        self.rebalance_idle_npc_tanks()?;
        self.collect_automated_trader_requests()?;
        self.resolve_pending_trade_requests()?;
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
        let e = self.market_entity(system)?;
        let market = self.world.get::<Market>(e).unwrap();
        let policy = self.world.get::<MarketPolicy>(e).unwrap();
        Ok((
            self.bid_quote(market, policy, good)?,
            self.ask_quote(market, policy, good)?,
        ))
    }

    pub fn market_demand(
        &mut self,
        system: &ContentId,
        good: &ContentId,
    ) -> Result<MarketDemandSnapshot, CoreError> {
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
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let funded =
            market.funded_quantity_for_purchases(policy, life, advertised, bid, Energy::ZERO)?;
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

    pub fn snapshot(&mut self) -> CoreSnapshot {
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
            .query::<(&StableId, &DisplayName, &Trader, Option<&PlayerControlled>)>()
            .iter(&self.world)
            .map(|(id, n, t, p)| TraderSnapshot {
                id: id.0.clone(),
                name: n.0.clone(),
                system: t.system.clone(),
                energy_tank: t.energy_tank,
                energy_tank_capacity: t.energy_tank_capacity,
                cargo: t.cargo.clone(),
                cargo_capacity: t.cargo_capacity,
                speed: t.speed,
                travel_burn_per_distance: t.travel_burn_per_distance,
                refuel_policy: t.refuel_policy,
                travel: t.travel.clone(),
                reservation: t.reservation,
                ledger: t.ledger.clone(),
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
        CoreSnapshot {
            tick: self.tick(),
            markets,
            traders,
            reservations,
            energy_flow,
            dynamics_history: self.world.resource::<AggregateDynamicsHistory>().clone(),
            fleet: self.world.resource::<FleetDynamics>().clone(),
        }
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
            return Err(CoreError::InvalidPolicy);
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

    fn maximum_normal_energy_bid(
        &self,
        market: &Market,
        policy: &MarketPolicy,
    ) -> Result<Energy, CoreError> {
        let energy = ContentId::new(ENERGY_ID).expect("constant id");
        let mut maximum_scarcity = market.clone();
        maximum_scarcity.set_energy_stock(Energy::ZERO)?;
        let ask = self.ask_quote(&maximum_scarcity, policy, &energy)?;
        let priority = u64::from(
            policy
                .import_priorities
                .get(&energy)
                .copied()
                .unwrap_or(100),
        );
        checked_mul_ratio_ceil(ask, priority, 100)
    }

    fn validate_emergency_energy_bid_ceiling(&mut self) -> Result<(), CoreError> {
        let ceiling = self
            .world
            .resource::<EconomyConfig>()
            .brownouts
            .emergency_energy_bid_ceiling;
        let markets = self
            .world
            .query::<(&Market, &MarketPolicy)>()
            .iter(&self.world)
            .map(|(market, policy)| (market.clone(), policy.clone()))
            .collect::<Vec<_>>();
        for (market, policy) in markets {
            if self.maximum_normal_energy_bid(&market, &policy)? > ceiling {
                return Err(CoreError::InvalidWorldDynamics);
            }
        }
        Ok(())
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
        if !self.demand_allowed(market, good) {
            return Ok(Energy::ZERO);
        }
        let mut bid = self.normal_bid_quote(market, policy, good)?;
        if good.as_str() == ENERGY_ID && market.operating_profile.stage >= BrownoutStage::Emergency
        {
            let brownouts = &self.world.resource::<EconomyConfig>().brownouts;
            let ceiling = brownouts.emergency_energy_bid_ceiling;
            debug_assert!(
                bid <= ceiling,
                "validated emergency ceiling must not lower bids"
            );
            if market.operating_profile.stage == BrownoutStage::Starvation {
                bid = ceiling;
            } else if bid < ceiling {
                let range = u64::from(brownouts.emergency_recovery_ticks);
                let pressure = range.saturating_sub(u64::from(market.brownout.ticks_of_burn));
                let increase = Energy(ceiling.0 - bid.0);
                bid = bid
                    .checked_add(checked_mul_ratio_ceil(increase, pressure, range)?)?
                    .min(ceiling);
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
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let (system, tank, travel, used, cap) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
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
        let market_energy = if good.as_str() == ENERGY_ID {
            initial_market_energy
                .checked_sub(Energy(i64::from(quantity)))?
                .checked_add(total)?
        } else {
            initial_market_energy.checked_add(total)?
        };
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
        if good.as_str() == ENERGY_ID {
            market.energy_flow.market_to_energy_cargo = market
                .energy_flow
                .market_to_energy_cargo
                .checked_add(Energy(i64::from(quantity)))?;
        }
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
        if requested == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let (system, cargo, tank, cap, travel) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
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
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
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
            let quantity = if liquidation {
                funded_quantity(
                    requested,
                    m.energy_stock()?,
                    m.reserved_energy,
                    m.operating_reserve(p, life)?,
                    Energy::ZERO,
                    bid,
                )?
            } else {
                m.funded_quantity_for_purchases(p, life, requested, bid, Energy::ZERO)?
            };
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

    /// Shared validate-before-mutate settlement used by reservations, immediate
    /// funded sales, energy cargo, and liquidation.
    fn execute_funded_sale(
        &mut self,
        trader_entity: Entity,
        market_entity: Entity,
        good: &ContentId,
        quantity: u32,
        terms: SaleTerms,
    ) -> Result<(), CoreError> {
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
        if good.as_str() == ENERGY_ID {
            market.set_energy_stock(after_payment.checked_add(Energy(i64::from(quantity)))?)?;
            market.energy_flow.energy_cargo_to_market = market
                .energy_flow
                .energy_cargo_to_market
                .checked_add(Energy(i64::from(quantity)))?;
        } else {
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
        }
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
            let life = self
                .world
                .resource::<EconomyConfig>()
                .life_support_burn_per_capita;
            let market = self.world.get::<Market>(market_entity).unwrap();
            let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
            if market.unreserved_energy_for_purchases(policy, life)? < amount {
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
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let mut traders = self
            .world
            .query_filtered::<(Entity, &StableId, &Trader), Without<PlayerControlled>>()
            .iter(&self.world)
            .filter(|(_, _, trader)| trader.travel.is_none() && trader.cargo.is_empty())
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
                let market = self.world.get::<Market>(market_entity).unwrap();
                let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
                let available = market.unreserved_energy_for_purchases(policy, life)?;
                let amount = Energy((target.0 - tank.0).min(available.0));
                if amount.0 > 0 {
                    self.transfer_tank(entity, amount, false)?;
                }
            }
        }
        Ok(())
    }

    fn set_player_policy(
        &mut self,
        system: &ContentId,
        policy: MarketPolicy,
    ) -> Result<(), CoreError> {
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
        self.apply_market_policy(system, market_entity, policy)
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
        let market = self.world.get::<Market>(market_entity).unwrap();
        let emergency_ceiling = self
            .world
            .resource::<EconomyConfig>()
            .brownouts
            .emergency_energy_bid_ceiling;
        if self.maximum_normal_energy_bid(market, &policy)? > emergency_ceiling {
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
            .query::<&Trader>()
            .iter(&self.world)
            .map(|trader| LiquidationTraderCapability {
                cargo_capacity: trader.cargo_capacity,
                energy_tank_capacity: trader.energy_tank_capacity,
                travel_burn_per_distance: trader.travel_burn_per_distance,
            })
            .collect::<Vec<_>>();
        if let Some(capability) = self.world.resource::<FleetDynamics>().archetype_capability {
            capabilities.push(capability);
        }
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

    fn begin_travel(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
    ) -> Result<(), CoreError> {
        let (start, speed, burn, tank) = {
            let t = self.world.get::<Trader>(trader_entity).unwrap();
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
                    labor_percent: 100,
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
        let market_entity = self.market_entity(destination)?;
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let (price, quantity, total) = {
            let m = self.world.get::<Market>(market_entity).unwrap();
            let p = self.world.get::<MarketPolicy>(market_entity).unwrap();
            let price = self.bid_quote(m, p, good)?;
            if price == Energy::ZERO {
                return Err(CoreError::Unfunded);
            }
            let q = m.funded_quantity_for_purchases(p, life, requested, price, Energy::ZERO)?;
            if q == 0 {
                return Err(CoreError::Unfunded);
            }
            let total = price.checked_mul(u64::from(q))?;
            (price, q, total)
        };
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();
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
        let system = self.world.get::<Trader>(e).unwrap().system.clone();
        if system != r.destination {
            return Ok(());
        }
        if r.floor_unit_price.0 <= 0 {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let trader = self.world.get::<Trader>(e).unwrap();
        let cargo = trader.cargo.get(&r.good).copied().unwrap_or(0);
        let headroom = trader
            .energy_tank_capacity
            .checked_sub(trader.energy_tank)?;
        let market_entity = self.market_entity(&system)?;
        let market = self.world.get::<Market>(market_entity).unwrap();
        let policy = self.world.get::<MarketPolicy>(market_entity).unwrap();
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let quantity = market
            .funded_quantity_for_purchases(
                policy,
                life,
                r.remaining_quantity,
                r.floor_unit_price,
                r.reserved_energy,
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
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let state = self.world.get::<Trader>(trader).unwrap();
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
        let trader_id = self.world.get::<StableId>(trader).unwrap().0.clone();
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
        if requested == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let original_trader = self.world.get::<Trader>(trader_entity).unwrap().clone();
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
        let trader_id = self.world.get::<StableId>(trader_entity).unwrap().0.clone();

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

        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let quantity = destination_market.funded_quantity_for_purchases(
            destination_policy,
            life,
            candidate_quantity,
            bid,
            Energy::ZERO,
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
        let next_market_energy = if good.as_str() == ENERGY_ID {
            initial_market_energy
                .checked_sub(Energy(i64::from(quantity)))?
                .checked_add(purchase_total)?
        } else {
            initial_market_energy.checked_add(purchase_total)?
        };
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
        if good.as_str() == ENERGY_ID {
            origin_market.energy_flow.market_to_energy_cargo = origin_market
                .energy_flow
                .market_to_energy_cargo
                .checked_add(Energy(i64::from(quantity)))?;
        }
        trader.energy_tank = tank.checked_sub(required_tank)?;
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
            .filter(|(_, _, trader, _)| trader.travel.is_none() && !trader.cargo.is_empty())
            .map(|(id, entity, trader, player)| {
                (id.0.clone(), entity, trader.reservation, player.is_some())
            })
            .collect::<Vec<_>>();
        traders.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, e, reservation, player) in traders {
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
        let (origin, tank, capacity, burn_per_distance, cargo) = {
            let trader = self.world.get::<Trader>(e).unwrap();
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
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
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
            let funded = market.funded_quantity_for_purchases(
                &policy,
                life,
                u32::try_from(cargo).unwrap_or(u32::MAX),
                bid,
                Energy::ZERO,
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

    fn collect_automated_trader_requests(&mut self) -> Result<(), CoreError> {
        #[derive(Clone)]
        struct Request {
            score: i128,
            trader_id: ContentId,
            e: Entity,
            good: ContentId,
            destination: ContentId,
            quantity: u32,
        }
        let graph = self.graph().clone();
        let markets = self
            .world
            .query_filtered::<(&StableId, &Market, &MarketPolicy), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, m, p)| (id.0.clone(), m.clone(), p.clone()))
            .collect::<Vec<_>>();
        let mut requests = Vec::new();
        for (e, id, t) in self
            .world
            .query_filtered::<(Entity, &StableId, &Trader), (Without<PlayerControlled>,)>()
            .iter(&self.world)
        {
            if t.travel.is_some() || !t.cargo.is_empty() {
                continue;
            }
            for (good, stock) in markets
                .iter()
                .find(|(sid, _, _)| sid == &t.system)
                .unwrap()
                .1
                .inventory
                .iter()
            {
                if *stock == 0 {
                    continue;
                }
                let origin = markets.iter().find(|(sid, _, _)| sid == &t.system).unwrap();
                let ask = self.ask_quote(&origin.1, &origin.2, good)?;
                for (destination, m, p) in &markets {
                    if destination == &t.system {
                        continue;
                    }
                    let bid = self.bid_quote(m, p, good)?;
                    if bid <= ask {
                        continue;
                    }
                    let Some((route, distance)) = graph.shortest_path(&t.system, destination)
                    else {
                        continue;
                    };
                    let burn = route_travel_energy(&graph, &route, t.travel_burn_per_distance)?;
                    if t.energy_tank < burn {
                        continue;
                    }
                    let affordable = u32::try_from(t.energy_tank.checked_sub(burn)?.0 / ask.0)
                        .unwrap_or(u32::MAX);
                    let quantity = u32::try_from((*stock).min(u64::from(t.cargo_capacity)))
                        .unwrap_or(u32::MAX)
                        .min(affordable);
                    if quantity > 0 {
                        let gross_margin =
                            Energy(bid.0 - ask.0).checked_mul(u64::from(quantity))?;
                        if gross_margin <= burn {
                            continue;
                        }
                        let net_profit = gross_margin.checked_sub(burn)?;
                        let score = i128::from(net_profit.0) * 1_000_000
                            / i128::from(ticks_for_distance(distance, t.speed));
                        requests.push(Request {
                            score,
                            trader_id: id.0.clone(),
                            e,
                            good: good.clone(),
                            destination: destination.clone(),
                            quantity,
                        });
                    }
                }
            }
        }
        requests.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.trader_id.cmp(&b.trader_id))
                .then_with(|| a.good.cmp(&b.good))
                .then_with(|| a.destination.cmp(&b.destination))
        });
        self.world
            .resource_mut::<PendingTradeRequests>()
            .0
            .extend(requests.into_iter().map(|request| PendingTradeRequest {
                score: request.score,
                trader_id: request.trader_id,
                trader: request.e,
                destination: request.destination,
                good: request.good,
                quantity: request.quantity,
                buy_at_origin: true,
                command_driven: false,
            }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(s: &str) -> ContentId {
        ContentId::new(s).unwrap()
    }
    fn physical_energy(snapshot: &CoreSnapshot) -> i128 {
        let markets = snapshot
            .markets
            .iter()
            .map(|market| i128::from(market.energy_stock.0))
            .sum::<i128>();
        let tanks = snapshot
            .traders
            .iter()
            .map(|trader| i128::from(trader.energy_tank.0))
            .sum::<i128>();
        let cargo = snapshot
            .traders
            .iter()
            .map(|trader| {
                trader
                    .cargo
                    .get(&id(ENERGY_ID))
                    .copied()
                    .map_or(0, i128::from)
            })
            .sum::<i128>();
        markets + tanks + cargo
    }
    fn definition() -> GameDefinition {
        let energy = id(ENERGY_ID);
        let ore = id("core:ore");
        GameDefinition {
            goods: vec![
                GoodDefinition {
                    id: energy.clone(),
                    name: "Energy".into(),
                    category: GoodCategory::Energy,
                    bootstrap_cost: Energy(1),
                },
                GoodDefinition {
                    id: ore.clone(),
                    name: "Ore".into(),
                    category: GoodCategory::Raw,
                    bootstrap_cost: Energy(3),
                },
            ],
            recipes: vec![],
            systems: (0..2)
                .map(|i| SystemDefinition {
                    id: id(&format!("core:s{i}")),
                    name: format!("S{i}"),
                    position: Position3 {
                        x: f64::from(i) * 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    inventory: BTreeMap::from([
                        (energy.clone(), 1000),
                        (ore.clone(), if i == 0 { 100 } else { 0 }),
                    ]),
                    targets: BTreeMap::from([(ore.clone(), 10), (energy.clone(), 100)]),
                    recipes: vec![],
                    sources: vec![],
                    energy_output_per_tick: Energy(10),
                    seasonal_generation: SeasonalGenerationState {
                        base_output: Energy(10),
                        amplitude_percent: 0,
                        period_ticks: 100,
                        phase_ticks: 0,
                        current_effective_output: Energy(10),
                    },
                    energy_storage_cap: Energy(2000),
                    population: 1,
                    population_state: PopulationState {
                        current: 1,
                        reference: 1,
                        carrying_capacity: 1,
                        ..PopulationState::default()
                    },
                    investment_policy: InvestmentPolicy::default(),
                    governance: Governance {
                        authority: MarketAuthority::Player(id("core:player")),
                    },
                    policy: MarketPolicy::default(),
                    protected_liquidation_budget: Energy(20),
                    bootstrap_risk_acknowledged: false,
                })
                .collect(),
            traders: vec![TraderDefinition {
                id: id("core:player"),
                name: "Player".into(),
                system: id("core:s0"),
                energy_tank: Energy(100),
                energy_tank_capacity: Energy(1000),
                cargo_capacity: 20,
                speed: 10.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: true,
            }],
            fleet: FleetDynamics {
                mode: Some(FleetMode::Fixed { count: 0 }),
                ..FleetDynamics::default()
            },
            economy: EconomyConfig::default(),
        }
    }
    #[test]
    fn physical_tick_generates_caps_burns_and_reports_deficit() {
        let mut d = definition();
        d.systems[0].inventory.insert(id(ENERGY_ID), 1999);
        d.systems[0].population = 3;
        let mut s = GameSession::new(d).unwrap();
        s.step().unwrap();
        let m = &s.snapshot().markets[0];
        assert_eq!(m.energy_stock, Energy(1997));
        assert_eq!(m.energy_flow.generated, Energy(10));
        assert_eq!(m.energy_flow.curtailed, Energy(9));
        assert_eq!(m.energy_flow.life_support_burned, Energy(3));
    }
    #[test]
    fn funded_quantity_keeps_reserves_independent() {
        assert_eq!(
            funded_quantity(
                30,
                Energy(400),
                Energy(87),
                Energy(50),
                Energy(20),
                Energy(13)
            )
            .unwrap(),
            18
        );
        assert_eq!(
            funded_quantity(
                30,
                Energy(400),
                Energy(87),
                Energy(100),
                Energy(20),
                Energy(13)
            )
            .unwrap(),
            14
        );
    }
    #[test]
    fn cost_basis_and_weighted_allocation_preserve_exact_energy() {
        let mut b = CostBasis {
            stock_quantity: 3,
            total_embodied_energy: Energy(10),
        };
        assert_eq!(b.remove(2).unwrap(), Energy(6));
        assert_eq!(
            b,
            CostBasis {
                stock_quantity: 1,
                total_embodied_energy: Energy(4)
            }
        );
        let a = allocate_embodied_energy(Energy(11), &[(id("core:a"), 1, 1), (id("core:b"), 1, 2)])
            .unwrap();
        assert_eq!(a[0].1, Energy(4));
        assert_eq!(a[1].1, Energy(7));
        let permuted =
            allocate_embodied_energy(Energy(11), &[(id("core:b"), 1, 2), (id("core:a"), 1, 1)])
                .unwrap();
        assert_eq!(a, permuted);
    }
    #[test]
    fn bay_energy_is_not_tank_energy_and_travel_only_burns_tank() {
        let mut s = GameSession::new(definition()).unwrap();
        s.submit(GameCommand::Buy {
            good: id(ENERGY_ID),
            quantity: 5,
        })
        .unwrap();
        let before = s.snapshot().traders[0].clone();
        assert_eq!(before.cargo.get(&id(ENERGY_ID)), Some(&5));
        s.submit(GameCommand::BeginTravel {
            destination: id("core:s1"),
        })
        .unwrap();
        let after = s.snapshot().traders[0].clone();
        assert_eq!(after.cargo.get(&id(ENERGY_ID)), Some(&5));
        assert_eq!(before.energy_tank.0 - after.energy_tank.0, 10);
    }
    #[test]
    fn energy_cargo_uses_funded_reservation_and_tank_headroom() {
        let mut d = definition();
        let energy = id(ENERGY_ID);
        d.systems[0].targets.insert(energy.clone(), 100);
        d.systems[1].inventory.insert(energy.clone(), 100);
        d.systems[1].targets.insert(energy.clone(), 1_000);
        d.systems[1]
            .policy
            .import_priorities
            .insert(energy.clone(), 200);
        d.traders[0].energy_tank = Energy(990);
        d.traders[0].energy_tank_capacity = Energy(1_000);
        let mut s = GameSession::new(d).unwrap();
        assert_eq!(s.quotes(&id("core:s0"), &energy).unwrap().1, Energy(2));
        assert_eq!(s.quotes(&id("core:s1"), &energy).unwrap().0, Energy(6));

        s.submit(GameCommand::CommitTrade {
            origin: id("core:s0"),
            destination: id("core:s1"),
            good: energy.clone(),
            quantity: 20,
        })
        .unwrap();
        assert!(
            s.snapshot().reservations.is_empty(),
            "requests resolve only on tick"
        );
        s.step().unwrap();
        let departure = s.snapshot();
        let reservation = departure
            .reservations
            .iter()
            .find(|reservation| reservation.status == ReservationStatus::Active)
            .unwrap();
        assert_eq!(reservation.quantity, 5, "profit must fit arrival headroom");
        assert_eq!(departure.traders[0].cargo.get(&energy), Some(&5));

        s.step().unwrap();
        let arrival = s.snapshot();
        assert_eq!(arrival.traders[0].cargo.get(&energy), None);
        assert_eq!(arrival.traders[0].energy_tank, Energy(1_000));
        assert_eq!(
            arrival.energy_flow.market_to_energy_cargo,
            WideEnergy(WideAmount(5))
        );
        assert_eq!(
            arrival.energy_flow.energy_cargo_to_market,
            WideEnergy(WideAmount(5))
        );
        assert_eq!(arrival.markets[1].reserved_energy, Energy::ZERO);
    }

    #[test]
    fn invalid_policy_and_failed_purchase_are_atomic() {
        let mut s = GameSession::new(definition()).unwrap();
        let before = format!("{:?}", s.snapshot());
        let p = MarketPolicy {
            default_target: 0,
            ..MarketPolicy::default()
        };
        assert_eq!(
            s.submit(GameCommand::SetMarketPolicy {
                system: id("core:s0"),
                policy: p
            }),
            Err(CoreError::InvalidPolicy)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);
        let before = format!("{:?}", s.snapshot());
        assert!(
            s.submit(GameCommand::Buy {
                good: id("core:ore"),
                quantity: u32::MAX
            })
            .is_err()
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);
    }
    #[test]
    fn policy_replacement_recomputes_protection_and_rejects_infeasible_changes_atomically() {
        let mut s = GameSession::new(definition()).unwrap();
        let system = id("core:s0");
        let mut policy = MarketPolicy {
            liquidation_discount_percent: 100,
            operating_reserve_ticks: 99,
            ..MarketPolicy::default()
        };
        s.submit(GameCommand::SetMarketPolicy {
            system: system.clone(),
            policy: policy.clone(),
        })
        .unwrap();
        let changed = s.snapshot();
        assert_eq!(changed.markets[0].policy, policy);
        assert_eq!(changed.markets[0].protected_liquidation_budget, Energy(21));

        policy.operating_reserve_ticks = 0;
        s.submit(GameCommand::SetMarketPolicy {
            system: system.clone(),
            policy: policy.clone(),
        })
        .unwrap();
        assert_eq!(
            s.snapshot().markets[0].protected_liquidation_budget,
            Energy(21),
            "operating reserve must not weaken or inflate anti-strand protection"
        );

        s.drain_events();
        let before = format!("{:?}", s.snapshot());
        policy.liquidation_threshold_percent = u32::MAX;
        assert_eq!(
            s.submit(GameCommand::SetMarketPolicy { system, policy }),
            Err(CoreError::InvalidPhysicalDefinition)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);
        assert!(matches!(
            s.drain_events().as_slice(),
            [GameEvent::Rejected(_)]
        ));

        let trader = s.player_entity().unwrap();
        s.world
            .get_mut::<Trader>(trader)
            .unwrap()
            .travel_burn_per_distance = Energy(i64::MAX);
        let before = format!("{:?}", s.snapshot());
        let feasible_policy = MarketPolicy {
            liquidation_discount_percent: 100,
            operating_reserve_ticks: 0,
            ..MarketPolicy::default()
        };
        assert_eq!(
            s.submit(GameCommand::SetMarketPolicy {
                system: id("core:s0"),
                policy: feasible_policy,
            }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);
        assert!(matches!(
            s.drain_events().as_slice(),
            [GameEvent::Rejected(_)]
        ));
    }

    #[test]
    fn failed_departure_after_staged_purchase_leaves_commitment_snapshot_and_events_unchanged() {
        let mut s = GameSession::new(definition()).unwrap();
        let trader = s.player_entity().unwrap();
        let origin = s.market_entity(&id("core:s0")).unwrap();
        s.world
            .get_mut::<Market>(origin)
            .unwrap()
            .energy_flow
            .travel_burned = Energy(i64::MAX);
        s.drain_events();
        let before_snapshot = format!("{:?}", s.snapshot());
        let before_events = s.drain_events();

        assert_eq!(
            s.commit_and_depart(trader, &id("core:s1"), &id("core:ore"), 1),
            Err(CoreError::Overflow)
        );

        assert_eq!(format!("{:?}", s.snapshot()), before_snapshot);
        assert_eq!(s.drain_events(), before_events);
    }

    #[test]
    fn same_tick_contention_winner_is_invariant_to_trader_insertion_order() {
        fn run(reverse: bool) -> (ContentId, Energy) {
            let mut d = definition();
            d.systems[1].inventory.insert(id(ENERGY_ID), 50);
            d.systems[1].energy_output_per_tick = Energy::ZERO;
            d.systems[1].population = 0;
            let mut npcs = vec![
                TraderDefinition {
                    id: id("core:ai_a"),
                    name: "A".into(),
                    system: id("core:s0"),
                    energy_tank: Energy(500),
                    energy_tank_capacity: Energy(1_000),
                    cargo_capacity: 20,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                    player: false,
                },
                TraderDefinition {
                    id: id("core:ai_b"),
                    name: "B".into(),
                    system: id("core:s0"),
                    energy_tank: Energy(500),
                    energy_tank_capacity: Energy(1_000),
                    cargo_capacity: 20,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                    player: false,
                },
            ];
            if reverse {
                npcs.reverse();
            }
            d.traders.extend(npcs);
            let mut s = GameSession::new(d).unwrap();
            s.step().unwrap();
            let snapshot = s.snapshot();
            let reservation = snapshot
                .reservations
                .iter()
                .filter(|reservation| reservation.status == ReservationStatus::Active)
                .min_by_key(|reservation| reservation.trader.clone())
                .unwrap();
            let market = snapshot
                .markets
                .iter()
                .find(|market| market.system_id == id("core:s1"))
                .unwrap();
            assert!(market.reserved_energy <= Energy(30));
            (reservation.trader.clone(), market.reserved_energy)
        }

        let forward = run(false);
        let reverse = run(true);
        assert_eq!(forward, reverse);
        assert_eq!(forward.0, id("core:ai_a"));
    }

    #[test]
    fn low_liquidity_arrival_partially_settles_releases_claim_and_reroutes() {
        let mut d = definition();
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[1].energy_output_per_tick = Energy::ZERO;
        d.systems[0].population = 0;
        d.systems[1].population = 0;
        d.traders.push(TraderDefinition {
            id: id("core:ai"),
            name: "AI".into(),
            system: id("core:s0"),
            energy_tank: Energy(100),
            energy_tank_capacity: Energy(1_000),
            cargo_capacity: 20,
            speed: 10.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: false,
        });
        let mut s = GameSession::new(d).unwrap();
        let ai = s
            .world
            .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
            .iter(&s.world)
            .find(|(_, stable)| stable.0 == id("core:ai"))
            .unwrap()
            .0;
        s.commit_and_depart(ai, &id("core:s1"), &id("core:ore"), 10)
            .unwrap();
        let reservation_id = s.world.get::<Trader>(ai).unwrap().reservation.unwrap();
        let reservation = s
            .world
            .resource::<Reservations>()
            .entries
            .get(&reservation_id)
            .unwrap()
            .clone();
        let destination = s.market_entity(&id("core:s1")).unwrap();
        let protected = s
            .world
            .get::<Market>(destination)
            .unwrap()
            .protected_liquidation_budget;
        s.world
            .get_mut::<Market>(destination)
            .unwrap()
            .set_energy_stock(
                protected
                    .checked_add(reservation.floor_unit_price.checked_mul(2).unwrap())
                    .unwrap(),
            )
            .unwrap();
        s.drain_events();
        s.step().unwrap();
        let snapshot = s.snapshot();
        let trader = snapshot
            .traders
            .iter()
            .find(|trader| trader.id == id("core:ai"))
            .unwrap();
        assert!(trader.cargo.get(&id("core:ore")).copied().unwrap_or(0) > 0);
        assert!(trader.travel.is_some() || trader.energy_tank > Energy::ZERO);
        assert_eq!(snapshot.markets[1].reserved_energy, Energy::ZERO);
        let released = snapshot
            .reservations
            .iter()
            .find(|entry| entry.id == reservation_id)
            .unwrap();
        assert_eq!(released.status, ReservationStatus::Fulfilled);
        assert_eq!(released.reserved_energy, Energy::ZERO);
        let events = s.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::Sold {
                partial: true,
                quantity: 2,
                ..
            }
        )));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GameEvent::ReservationReleased { reservation, .. } if *reservation == reservation_id))
                .count(),
            1
        );
    }

    #[test]
    fn mandatory_life_support_may_exhaust_claimed_stock_without_failing_arrival_tick() {
        let mut d = definition();
        d.systems[0].population = 0;
        d.systems[1].population = 1_000;
        d.systems[1].policy.operating_reserve_ticks = 0;
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[1].energy_output_per_tick = Energy::ZERO;
        d.traders.push(TraderDefinition {
            id: id("core:ai"),
            name: "AI".into(),
            system: id("core:s0"),
            energy_tank: Energy(100),
            energy_tank_capacity: Energy(1_000),
            cargo_capacity: 20,
            speed: 10.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: false,
        });
        let mut s = GameSession::new(d).unwrap();
        let ai = s
            .world
            .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
            .iter(&s.world)
            .find(|(_, stable)| stable.0 == id("core:ai"))
            .unwrap()
            .0;
        s.commit_and_depart(ai, &id("core:s1"), &id("core:ore"), 2)
            .unwrap();
        s.step().unwrap();
        let snapshot = s.snapshot();
        assert_eq!(snapshot.markets[1].energy_stock, Energy::ZERO);
        assert_eq!(snapshot.markets[1].reserved_energy, Energy::ZERO);
        assert_eq!(
            snapshot.markets[1].energy_flow.life_support_burned,
            Energy(1_000)
        );
    }

    #[test]
    fn reservation_contention_is_stable_and_partial_settlement_releases_claim() {
        let mut d = definition();
        d.traders.push(TraderDefinition {
            id: id("core:ai"),
            name: "AI".into(),
            system: id("core:s0"),
            energy_tank: Energy(500),
            energy_tank_capacity: Energy(1000),
            cargo_capacity: 20,
            speed: 10.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: false,
        });
        let mut s = GameSession::new(d).unwrap();
        let ai = s
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&s.world)
            .next()
            .unwrap();
        let q = s
            .create_reservation(ai, &id("core:s1"), &id("core:ore"), 20)
            .unwrap();
        assert!(q > 0);
        let reserved = s.snapshot().markets[1].reserved_energy;
        assert!(reserved.0 > 0);
        s.release_reservation(
            s.world.get::<Trader>(ai).unwrap().reservation.unwrap(),
            ReservationStatus::Cancelled,
        )
        .unwrap();
        assert_eq!(s.snapshot().markets[1].reserved_energy, Energy(0));
    }
    #[test]
    fn energy_flow_reconciles_external_delta() {
        let mut s = GameSession::new(definition()).unwrap();
        let before = s.snapshot();
        let total_before: i64 = before.markets.iter().map(|m| m.energy_stock.0).sum::<i64>()
            + before
                .traders
                .iter()
                .map(|t| {
                    t.energy_tank.0
                        + i64::try_from(t.cargo.get(&id(ENERGY_ID)).copied().unwrap_or(0)).unwrap()
                })
                .sum::<i64>();
        s.step().unwrap();
        let after = s.snapshot();
        let total_after: i64 = after.markets.iter().map(|m| m.energy_stock.0).sum::<i64>()
            + after
                .traders
                .iter()
                .map(|t| {
                    t.energy_tank.0
                        + i64::try_from(t.cargo.get(&id(ENERGY_ID)).copied().unwrap_or(0)).unwrap()
                })
                .sum::<i64>();
        assert_eq!(
            total_after - total_before,
            i64::try_from(i128::from(after.energy_flow.net_external_delta().0)).unwrap()
        );
    }

    #[test]
    fn active_claims_block_discretionary_burn_independently_of_operating_reserve() {
        let mut d = definition();
        d.economy.source_output_percent = 50;
        d.systems[0].policy.operating_reserve_ticks = 0;
        d.systems[0].sources = vec![SourceDefinition {
            good: id("core:ore"),
            quantity_per_tick: 2,
            extraction_energy: Energy(2),
        }];
        d.goods.push(GoodDefinition {
            id: id("core:alloy"),
            name: "Alloy".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(5),
        });
        d.recipes.push(RecipeDefinition {
            id: id("core:smelt"),
            name: "Smelt".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 1,
            }],
            outputs: vec![RecipeOutput {
                good: id("core:alloy"),
                quantity: 1,
                cost_weight: 1,
            }],
            operating_energy: Energy(2),
            margin_percent: None,
        });
        d.systems[0].recipes.push(id("core:smelt"));
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[0].population = 0;
        let mut s = GameSession::new(d).unwrap();
        let market = s.market_entity(&id("core:s0")).unwrap();
        let stock = s
            .world
            .get::<Market>(market)
            .unwrap()
            .energy_stock()
            .unwrap();
        s.world.get_mut::<Market>(market).unwrap().reserved_energy = Energy(stock.0 - 1);
        s.step().unwrap();
        let snapshot = s.snapshot();
        let market = snapshot
            .markets
            .iter()
            .find(|market| market.system_id == id("core:s0"))
            .unwrap();
        assert_eq!(market.energy_flow.source_burned, Energy::ZERO);
        assert_eq!(market.energy_flow.production_burned, Energy::ZERO);
        assert_eq!(market.inventory[&id("core:ore")], 100);
        assert_eq!(market.reserved_energy, Energy(stock.0 - 1));

        let mut d = definition();
        d.economy.source_output_percent = 50;
        d.economy.life_support_burn_per_capita = Energy::ZERO;
        d.systems[0].policy.operating_reserve_ticks = 0;
        d.systems[0].sources = vec![SourceDefinition {
            good: id("core:ore"),
            quantity_per_tick: 2,
            extraction_energy: Energy(2),
        }];
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        let mut s = GameSession::new(d).unwrap();
        let entity = s.market_entity(&id("core:s0")).unwrap();
        let protected = s
            .world
            .get::<Market>(entity)
            .unwrap()
            .protected_liquidation_budget;
        s.world
            .get_mut::<Market>(entity)
            .unwrap()
            .set_energy_stock(protected.checked_add(Energy(1)).unwrap())
            .unwrap();
        s.step().unwrap();
        let snapshot = s.snapshot();
        assert_eq!(snapshot.markets[0].energy_flow.source_burned, Energy::ZERO);
        assert_eq!(
            snapshot.markets[0].energy_stock,
            protected.checked_add(Energy(1)).unwrap()
        );
    }

    #[test]
    fn authored_refuel_policy_and_all_protected_claims_bound_tank_withdrawal() {
        let mut d = definition();
        d.traders[0].refuel_policy = RefuelPolicy::DepositOnly;
        let mut s = GameSession::new(d).unwrap();
        assert_eq!(
            s.submit(GameCommand::WithdrawTank { amount: Energy(1) }),
            Err(CoreError::RefuelForbidden)
        );

        let trader = s.player_entity().unwrap();
        s.world.get_mut::<Trader>(trader).unwrap().refuel_policy = RefuelPolicy::DepositAndWithdraw;
        let market = s.market_entity(&id("core:s0")).unwrap();
        let life = s
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        let policy = s.world.get::<MarketPolicy>(market).unwrap().clone();
        s.world.get_mut::<Market>(market).unwrap().reserved_energy = Energy(100);
        let available = s
            .world
            .get::<Market>(market)
            .unwrap()
            .unreserved_energy_for_purchases(&policy, life)
            .unwrap();
        assert_eq!(
            s.submit(GameCommand::WithdrawTank {
                amount: available.checked_add(Energy(1)).unwrap(),
            }),
            Err(CoreError::InsufficientEnergy)
        );
        s.submit(GameCommand::WithdrawTank { amount: available })
            .unwrap();
        let market = s.world.get::<Market>(market).unwrap();
        assert_eq!(
            market.energy_stock().unwrap(),
            market
                .reserved_energy
                .checked_add(market.operating_reserve(&policy, life).unwrap())
                .unwrap()
                .checked_add(market.protected_liquidation_budget)
                .unwrap()
        );
    }

    #[test]
    fn buy_tank_transfer_and_travel_are_atomic_on_ledger_overflow() {
        let mut s = GameSession::new(definition()).unwrap();
        let market = s.market_entity(&id("core:s0")).unwrap();
        s.world
            .get_mut::<Market>(market)
            .unwrap()
            .ledger
            .energy_received_from_traders = Energy(i64::MAX);
        let before = format!("{:?}", s.snapshot());
        assert_eq!(
            s.submit(GameCommand::Buy {
                good: id("core:ore"),
                quantity: 1,
            }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);

        s.world
            .get_mut::<Market>(market)
            .unwrap()
            .energy_flow
            .tank_to_market = Energy(i64::MAX);
        let before = format!("{:?}", s.snapshot());
        assert_eq!(
            s.submit(GameCommand::DepositTank { amount: Energy(1) }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);

        s.world
            .get_mut::<Market>(market)
            .unwrap()
            .energy_flow
            .travel_burned = Energy(i64::MAX);
        let before = format!("{:?}", s.snapshot());
        assert_eq!(
            s.submit(GameCommand::BeginTravel {
                destination: id("core:s1"),
            }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", s.snapshot()), before);
    }

    #[test]
    fn cost_aware_ask_compounds_margin_and_bounded_scarcity_with_checked_rounding() {
        let mut d = definition();
        d.systems[0].inventory.insert(id("core:ore"), 0);
        d.systems[0].targets.insert(id("core:ore"), 10);
        d.systems[0].policy.producer_margin_percent = 20;
        let mut s = GameSession::new(d).unwrap();
        // ceil(3 * 1.20) = 4, then ceil(4 * 1.50) = 6.
        assert_eq!(
            s.quotes(&id("core:s0"), &id("core:ore")).unwrap().1,
            Energy(6)
        );
        assert_eq!(
            checked_mul_ratio_ceil(Energy(i64::MAX), 2, 1),
            Err(CoreError::Overflow)
        );
    }

    #[test]
    fn processor_input_bids_are_non_recursive_and_structurally_solvent() {
        let mut d = definition();
        d.goods.extend([
            GoodDefinition {
                id: id("core:catalyst"),
                name: "Catalyst".into(),
                category: GoodCategory::Raw,
                bootstrap_cost: Energy(2),
            },
            GoodDefinition {
                id: id("core:alloy"),
                name: "Alloy".into(),
                category: GoodCategory::Primary,
                bootstrap_cost: Energy(12),
            },
        ]);
        d.recipes.push(RecipeDefinition {
            id: id("core:smelt"),
            name: "Smelt".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![
                GoodAmount {
                    good: id("core:ore"),
                    quantity: 2,
                },
                GoodAmount {
                    good: id("core:catalyst"),
                    quantity: 1,
                },
            ],
            outputs: vec![RecipeOutput {
                good: id("core:alloy"),
                quantity: 1,
                cost_weight: 1,
            }],
            operating_energy: Energy(2),
            margin_percent: Some(20),
        });
        d.systems[0].recipes.push(id("core:smelt"));
        d.systems[0].inventory.insert(id("core:catalyst"), 10);
        d.systems[0].inventory.insert(id("core:alloy"), 10);
        d.systems[0].targets.insert(id("core:catalyst"), 10);
        d.systems[0].targets.insert(id("core:alloy"), 10);
        let mut s = GameSession::new(d).unwrap();
        let rows = s.processor_solvency().unwrap();
        let row = rows
            .iter()
            .find(|row| row.recipe == id("core:smelt"))
            .unwrap();
        assert!(row.solvent, "{row:?}");
        assert!(row.expected_input_bids.0 > 0);
    }

    #[test]
    fn runtime_cost_propagates_through_single_multi_output_and_consuming_recipes() {
        let mut d = definition();
        d.economy.life_support_burn_per_capita = Energy::ZERO;
        d.goods.extend([
            GoodDefinition {
                id: id("core:alloy"),
                name: "Alloy".into(),
                category: GoodCategory::Primary,
                bootstrap_cost: Energy(5),
            },
            GoodDefinition {
                id: id("core:slag"),
                name: "Slag".into(),
                category: GoodCategory::Primary,
                bootstrap_cost: Energy(1),
            },
            GoodDefinition {
                id: id("core:machine"),
                name: "Machine".into(),
                category: GoodCategory::Secondary,
                bootstrap_cost: Energy(9),
            },
        ]);
        d.recipes.extend([
            RecipeDefinition {
                id: id("core:split"),
                name: "Split".into(),
                layer: RecipeLayer::Primary,
                inputs: vec![GoodAmount {
                    good: id("core:ore"),
                    quantity: 1,
                }],
                outputs: vec![
                    RecipeOutput {
                        good: id("core:alloy"),
                        quantity: 1,
                        cost_weight: 1,
                    },
                    RecipeOutput {
                        good: id("core:slag"),
                        quantity: 1,
                        cost_weight: 2,
                    },
                ],
                operating_energy: Energy(2),
                margin_percent: None,
            },
            RecipeDefinition {
                id: id("core:forge"),
                name: "Forge".into(),
                layer: RecipeLayer::Secondary,
                inputs: vec![GoodAmount {
                    good: id("core:alloy"),
                    quantity: 1,
                }],
                outputs: vec![RecipeOutput {
                    good: id("core:machine"),
                    quantity: 1,
                    cost_weight: 1,
                }],
                operating_energy: Energy(3),
                margin_percent: None,
            },
            RecipeDefinition {
                id: id("core:consume"),
                name: "Consume".into(),
                layer: RecipeLayer::Tertiary,
                inputs: vec![GoodAmount {
                    good: id("core:machine"),
                    quantity: 1,
                }],
                outputs: vec![],
                operating_energy: Energy(1),
                margin_percent: None,
            },
        ]);
        d.systems[0].recipes = vec![id("core:split"), id("core:forge"), id("core:consume")];
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        let mut s = GameSession::new(d).unwrap();
        s.step().unwrap();
        let snapshot = s.snapshot();
        let market = &snapshot.markets[0];
        assert_eq!(
            market.cost_basis[&id("core:slag")].total_embodied_energy,
            Energy(3)
        );
        assert_eq!(market.cost_basis[&id("core:alloy")].stock_quantity, 0);
        assert_eq!(market.cost_basis[&id("core:machine")].stock_quantity, 0);
        assert_eq!(market.energy_flow.production_burned, Energy(6));
        assert_eq!(market.ledger.processor_input_cost, Energy(5));
        assert_eq!(market.ledger.processor_operating_energy, Energy(5));
    }

    #[test]
    fn recipe_margin_override_is_applied_to_runtime_quote() {
        let mut d = definition();
        d.goods.push(GoodDefinition {
            id: id("core:alloy"),
            name: "Alloy".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(5),
        });
        d.recipes.push(RecipeDefinition {
            id: id("core:smelt"),
            name: "Smelt".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 1,
            }],
            outputs: vec![RecipeOutput {
                good: id("core:alloy"),
                quantity: 1,
                cost_weight: 1,
            }],
            operating_energy: Energy(1),
            margin_percent: Some(50),
        });
        d.systems[0].recipes.push(id("core:smelt"));
        d.systems[0].inventory.insert(id("core:alloy"), 10);
        d.systems[0].targets.insert(id("core:alloy"), 10);
        d.systems[0].policy.producer_margin_percent = 0;
        let mut s = GameSession::new(d).unwrap();
        assert_eq!(
            s.quotes(&id("core:s0"), &id("core:alloy")).unwrap().1,
            Energy(8)
        );
    }

    #[test]
    fn source_scaling_controls_runtime_output_burn_and_operating_reserve() {
        let mut d = definition();
        d.economy.source_output_percent = 50;
        d.economy.life_support_burn_per_capita = Energy::ZERO;
        d.systems[0].sources.push(SourceDefinition {
            good: id("core:ore"),
            quantity_per_tick: 3,
            extraction_energy: Energy(1),
        });
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[0].policy.operating_reserve_ticks = 1;
        let mut s = GameSession::new(d).unwrap();
        assert_eq!(s.snapshot().markets[0].operating_reserve, Energy(1));
        s.step().unwrap();
        let market = &s.snapshot().markets[0];
        assert_eq!(market.inventory[&id("core:ore")], 101);
        assert_eq!(market.energy_flow.source_burned, Energy(1));
    }

    #[test]
    fn route_burn_sums_each_leg_ceiling_and_global_flow_never_clamps() {
        let a = id("core:a");
        let b = id("core:b");
        let c = id("core:c");
        let graph = SystemGraph {
            positions: BTreeMap::new(),
            edges: BTreeMap::from([
                (a.clone(), vec![(b.clone(), 0.4)]),
                (b.clone(), vec![(a.clone(), 0.4), (c.clone(), 0.4)]),
                (c.clone(), vec![(b.clone(), 0.4)]),
            ]),
        };
        assert_eq!(
            route_travel_energy(&graph, &[a, b, c], Energy(1)).unwrap(),
            Energy(2)
        );
        assert_eq!(travel_energy(0.8, Energy(1)).unwrap(), Energy(1));

        let mut aggregate = GlobalEnergyFlowLedger::default();
        let flow = EnergyFlowLedger {
            generated: Energy(i64::MAX),
            ..EnergyFlowLedger::default()
        };
        aggregate.add_market(flow);
        aggregate.add_market(flow);
        assert_eq!(
            aggregate.generated,
            WideEnergy(WideAmount(i128::from(i64::MAX) * 2))
        );
    }

    #[test]
    fn liquidation_contract_and_threshold_are_deterministic() {
        let reference = Energy(7);
        assert_eq!(liquidation_unit_price(reference, 50).unwrap(), Energy(3));
        assert_eq!(
            liquidation_target_energy(Energy(11), 150).unwrap(),
            Energy(17)
        );
        let dynamic_adversarial_bid = Energy(i64::MAX / 100);
        assert_ne!(
            liquidation_unit_price(reference, 50).unwrap(),
            liquidation_unit_price(dynamic_adversarial_bid, 50).unwrap()
        );
    }

    #[test]
    fn brownout_boundaries_shocks_and_recovery_are_deterministic() {
        let config = BrownoutConfig::default();
        let normal = BrownoutState::default();
        for (runway, expected) in [
            (u32::MAX, BrownoutStage::Normal),
            (13, BrownoutStage::Normal),
            (12, BrownoutStage::Throttled),
            (7, BrownoutStage::Throttled),
            (6, BrownoutStage::Emergency),
            (2, BrownoutStage::Emergency),
            (1, BrownoutStage::Starvation),
            (0, BrownoutStage::Starvation),
        ] {
            assert_eq!(
                classify_brownout(&normal, &config, runway, Energy::ZERO, 10).unwrap(),
                expected,
                "runway {runway}"
            );
        }
        assert_eq!(
            classify_brownout(&normal, &config, 100, Energy(1), 10).unwrap(),
            BrownoutStage::Starvation,
            "unsupplied life support directly crosses all bands"
        );

        let mut state = BrownoutState {
            stage: BrownoutStage::Starvation,
            entered_at_tick: 5,
            ..BrownoutState::default()
        };
        assert_eq!(
            classify_brownout(&state, &config, 100, Energy::ZERO, 5).unwrap(),
            BrownoutStage::Starvation,
            "minimum occupancy blocks same-tick recovery"
        );
        assert_eq!(
            classify_brownout(&state, &config, 3, Energy::ZERO, 6).unwrap(),
            BrownoutStage::Emergency
        );
        state.stage = BrownoutStage::Emergency;
        state.entered_at_tick = 6;
        assert_eq!(
            classify_brownout(&state, &config, 8, Energy::ZERO, 7).unwrap(),
            BrownoutStage::Throttled
        );
        state.stage = BrownoutStage::Throttled;
        state.entered_at_tick = 7;
        assert_eq!(
            classify_brownout(&state, &config, 16, Energy::ZERO, 8).unwrap(),
            BrownoutStage::Normal
        );
    }

    #[test]
    fn triangle_throughput_population_fleet_and_investment_helpers_cover_boundaries() {
        assert_eq!(
            (0..4)
                .map(|tick| triangle_wave_output(Energy(100), 20, 4, 0, tick).unwrap())
                .collect::<Vec<_>>(),
            vec![Energy(80), Energy(100), Energy(120), Energy(100)]
        );
        assert_eq!(
            triangle_wave_output(Energy(i64::MAX), 0, 2, 0, u64::MAX).unwrap(),
            Energy(i64::MAX),
            "zero amplitude is exactly fixed output without tick overflow"
        );
        assert!(triangle_wave_output(Energy(1), 101, 2, 0, 0).is_err());
        assert!(
            triangle_wave_output(Energy(100), 20, 3, 0, 0).is_err(),
            "nonzero seasonal amplitude requires an even period"
        );
        assert_eq!(
            triangle_wave_output(Energy(100), 0, 3, 0, 1).unwrap(),
            Energy(100),
            "odd periods remain harmless for fixed-output seasons"
        );
        let odd_state = SeasonalGenerationState {
            base_output: Energy(100),
            amplitude_percent: 20,
            period_ticks: 3,
            phase_ticks: 0,
            current_effective_output: Energy(100),
        };
        assert_eq!(odd_state.validate(), Err(CoreError::InvalidWorldDynamics));
        assert_eq!(
            triangle_wave_output(Energy(100), 100, 4, 0, 0).unwrap(),
            Energy::ZERO,
            "the maximum permitted amplitude cannot produce negative generation"
        );
        assert_eq!(
            (0..4)
                .map(|tick| triangle_wave_output(Energy(100), 20, 4, 1, tick).unwrap())
                .collect::<Vec<_>>(),
            vec![Energy(100), Energy(120), Energy(100), Energy(80)],
            "an even period reaches exact extrema at the phase-shifted turning points"
        );
        assert_eq!(
            triangle_wave_output(Energy(100), 20, 4, 1, 3).unwrap(),
            triangle_wave_output(Energy(100), 20, 4, 1, 7).unwrap(),
            "phase-shifted output repeats exactly after one period"
        );
        assert_eq!(
            triangle_wave_output(Energy(100), 20, 4, 0, u64::MAX).unwrap(),
            triangle_wave_output(Energy(100), 20, 4, 0, u64::MAX % 4).unwrap(),
            "large ticks wrap before phase addition"
        );
        assert!(triangle_wave_output(Energy(i64::MAX), 100, 2, 0, 1).is_err());
        let phase = seasonal_phase(4, 0, 0).unwrap();
        assert_eq!(phase.trend, SeasonalTrend::Rising);
        assert_eq!(phase.next_turning_point_tick, Some(2));
        assert_eq!(
            seasonal_phase(4, 0, 2).unwrap().trend,
            SeasonalTrend::Falling
        );
        assert_eq!(
            seasonal_phase(4, 0, u64::MAX)
                .unwrap()
                .next_turning_point_tick,
            None,
            "a turning point beyond the clock range is explicit"
        );

        for (stage, labor, expected) in [(0, 100, 0), (1, 100, 1), (100, 100, 100)] {
            let mut production_carry = 0;
            let mut reserve_carry = 0;
            let mut diagnostic_carry = 0;
            assert_eq!(
                composed_throughput(100, stage, labor, &mut production_carry).unwrap(),
                expected
            );
            assert_eq!(
                composed_throughput(100, stage, labor, &mut reserve_carry).unwrap(),
                expected
            );
            assert_eq!(
                composed_throughput(100, stage, labor, &mut diagnostic_carry).unwrap(),
                expected
            );
        }
        let mut carry = 0;
        assert_eq!(
            (0..4)
                .map(|_| composed_throughput(1, 50, 50, &mut carry).unwrap())
                .collect::<Vec<_>>(),
            vec![0, 0, 0, 1],
            "stage and labor are multiplied before one final carry"
        );
        assert_eq!(carry, 0);

        let mut population_remainder = 0;
        assert_eq!(
            logistic_population_delta(90, 100, 1_000, 1, &mut population_remainder).unwrap(),
            9
        );
        assert_eq!(
            logistic_population_delta(100, 100, 1_000, 1, &mut population_remainder).unwrap(),
            0
        );
        assert_eq!(update_opportunity_persistence(4, 10, 10).unwrap(), 5);
        assert_eq!(update_opportunity_persistence(4, 9, 10).unwrap(), 0);
        assert!(update_opportunity_persistence(0, 1, 0).is_err());

        let shape = InvestmentShape {
            enabled: true,
            base_cost: Energy(100),
            cost_growth_percent: 150,
            maximum_level: 3,
            cooldown_ticks: 1,
            effect_per_level: 1,
        };
        assert_eq!(investment_cost(&shape, 0).unwrap(), Energy(100));
        assert_eq!(investment_cost(&shape, 1).unwrap(), Energy(150));
        assert_eq!(investment_cost(&shape, 2).unwrap(), Energy(225));
        assert!(investment_cost(&shape, 3).is_err());
    }

    #[test]
    fn seasonal_generation_runs_before_life_support_and_is_projected() {
        let mut d = definition();
        d.systems[0].energy_output_per_tick = Energy(100);
        d.systems[0].seasonal_generation = SeasonalGenerationState {
            base_output: Energy(100),
            amplitude_percent: 20,
            period_ticks: 4,
            phase_ticks: 0,
            current_effective_output: Energy(100),
        };
        d.systems[0].energy_storage_cap = Energy(10_000);
        d.systems[0].inventory.insert(id(ENERGY_ID), 1_000);
        d.systems[0].population = 1;
        let mut session = GameSession::new(d).unwrap();
        session.step().unwrap();
        let events = session.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::EnergyGenerated { system, amount: Energy(80), .. }
                if system == &id("core:s0")
        )));
        let market = session
            .snapshot()
            .markets
            .into_iter()
            .find(|market| market.system_id == id("core:s0"))
            .unwrap();
        assert_eq!(market.energy_stock, Energy(1_079));
        assert_eq!(market.seasonal_generation.base_output, Energy(100));
        assert_eq!(
            market.seasonal_generation.current_effective_output,
            Energy(80)
        );
        assert_eq!(market.seasonal_phase.position_ticks, 0);
        assert_eq!(market.seasonal_phase.next_turning_point_tick, Some(2));
    }

    #[test]
    fn recorded_external_delivery_is_atomic_and_reconciles_a_stage_intervention() {
        let mut d = definition();
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[0].seasonal_generation.base_output = Energy::ZERO;
        d.systems[0].seasonal_generation.current_effective_output = Energy::ZERO;
        d.systems[0].inventory.insert(id(ENERGY_ID), 7);
        d.systems[0].population = 1;
        let mut baseline = GameSession::new(d.clone()).unwrap();
        let mut intervention = GameSession::new(d).unwrap();
        let initial_physical = physical_energy(&intervention.snapshot());
        intervention
            .submit(GameCommand::RecordExternalDelivery {
                system: id("core:s0"),
                good: id(ENERGY_ID),
                quantity: 10,
            })
            .unwrap();
        baseline.step().unwrap();
        intervention.step().unwrap();
        let baseline_market = baseline.snapshot().markets.remove(0);
        let intervention_snapshot = intervention.snapshot();
        let intervention_market = intervention_snapshot.markets[0].clone();
        assert_eq!(baseline_market.brownout.stage, BrownoutStage::Emergency);
        assert_eq!(intervention_market.brownout.stage, BrownoutStage::Normal);
        assert_eq!(
            i128::from(intervention_snapshot.energy_flow.external_inflow.0),
            10_i128
        );
        assert_eq!(
            i128::from(intervention_snapshot.energy_flow.net_external_delta().0),
            physical_energy(&intervention_snapshot) - initial_physical
        );
        assert_eq!(
            intervention
                .drain_events()
                .iter()
                .filter(|event| matches!(event, GameEvent::ExternalDeliveryRecorded { .. }))
                .count(),
            1
        );

        let before = intervention.snapshot().markets[0].energy_stock;
        assert_eq!(
            intervention.submit(GameCommand::RecordExternalDelivery {
                system: id("core:s0"),
                good: id(ENERGY_ID),
                quantity: 20_000,
            }),
            Err(CoreError::InsufficientCapacity)
        );
        assert_eq!(intervention.snapshot().markets[0].energy_stock, before);
        assert!(
            !intervention
                .drain_events()
                .iter()
                .any(|event| matches!(event, GameEvent::ExternalDeliveryRecorded { .. }))
        );
    }

    #[test]
    fn brownout_runtime_suppresses_demand_caps_price_and_preserves_reservations() {
        let mut d = definition();
        d.economy.brownouts.emergency_energy_bid_ceiling = Energy(10);
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[0].inventory.insert(id(ENERGY_ID), 7);
        d.systems[0].population = 1;
        let mut session = GameSession::new(d).unwrap();
        let energy = id(ENERGY_ID);
        let ore = id("core:ore");
        let normal_energy_bid = session.quotes(&id("core:s0"), &energy).unwrap().0;
        let player = session.player_entity().unwrap();
        let reserved_quantity = session
            .create_reservation(player, &id("core:s1"), &ore, 1)
            .unwrap();
        assert_eq!(reserved_quantity, 1);
        let reservation_id = session
            .world
            .get::<Trader>(player)
            .unwrap()
            .reservation
            .unwrap();
        let reserved_before = session.snapshot().markets[1].reserved_energy;

        session.step().unwrap();
        let snapshot = session.snapshot();
        let distressed = snapshot
            .markets
            .iter()
            .find(|market| market.system_id == id("core:s0"))
            .unwrap();
        assert_eq!(distressed.brownout.stage, BrownoutStage::Emergency);
        assert_eq!(distressed.operating_profile.throughput_percent, 0);
        assert_eq!(
            session.quotes(&id("core:s0"), &ore).unwrap().0,
            Energy::ZERO
        );
        let emergency_bid = session.quotes(&id("core:s0"), &energy).unwrap().0;
        assert!(emergency_bid >= normal_energy_bid);
        assert!(emergency_bid <= Energy(10));
        assert_eq!(distressed.unreserved_energy_for_purchases, Energy::ZERO);
        assert_eq!(distressed.protected_liquidation_budget, Energy(20));
        assert_eq!(snapshot.markets[1].reserved_energy, reserved_before);
        assert_eq!(
            snapshot
                .reservations
                .iter()
                .find(|reservation| reservation.id == reservation_id)
                .unwrap()
                .status,
            ReservationStatus::Active
        );
        let events = session.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::BrownoutTransition {
                from: BrownoutStage::Normal,
                to: BrownoutStage::Emergency,
                ..
            }
        )));
        assert!(!events.iter().any(|event| matches!(
            event,
            GameEvent::TraderSpawned { .. } | GameEvent::TraderRetired { .. }
        )));

        session.step().unwrap();
        let steady_events = session.drain_events();
        assert!(
            !steady_events
                .iter()
                .any(|event| matches!(event, GameEvent::BrownoutTransition { .. }))
        );
        let steady = session.snapshot();
        let distressed = steady
            .markets
            .iter()
            .find(|market| market.system_id == id("core:s0"))
            .unwrap();
        assert_eq!(
            distressed.brownout.occupancy_ticks[BrownoutStage::Emergency.index()],
            2
        );
        assert_eq!(distressed.brownout.transition_count, 1);
        assert_eq!(
            steady
                .dynamics_history
                .stage_occupancy_ticks
                .iter()
                .sum::<u64>(),
            4
        );
    }

    #[test]
    fn throttled_recipe_uses_one_deterministic_final_carry() {
        let mut d = definition();
        d.goods.push(GoodDefinition {
            id: id("core:alloy"),
            name: "Alloy".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(5),
        });
        d.recipes.push(RecipeDefinition {
            id: id("core:smelt"),
            name: "Smelt".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 1,
            }],
            outputs: vec![RecipeOutput {
                good: id("core:alloy"),
                quantity: 1,
                cost_weight: 1,
            }],
            operating_energy: Energy(1),
            margin_percent: None,
        });
        d.systems[0].recipes.push(id("core:smelt"));
        d.systems[0].inventory.insert(id("core:alloy"), 0);
        d.systems[0].energy_output_per_tick = Energy::ZERO;
        d.systems[0].inventory.insert(id(ENERGY_ID), 130);
        d.systems[0].population = 10;
        let mut session = GameSession::new(d).unwrap();

        session.step().unwrap();
        let first = session.snapshot();
        assert_eq!(first.markets[0].brownout.stage, BrownoutStage::Throttled);
        assert_eq!(first.markets[0].inventory[&id("core:alloy")], 0);
        session.step().unwrap();
        let second = session.snapshot();
        assert_eq!(second.markets[0].brownout.stage, BrownoutStage::Throttled);
        assert_eq!(second.markets[0].inventory[&id("core:alloy")], 1);
        assert_eq!(second.markets[0].energy_flow.production_burned, Energy(1));
    }

    #[test]
    fn player_policy_changes_require_matching_governance_and_are_atomic() {
        let mut definition = definition();
        definition.systems[1].governance = Governance::default();
        let mut session = GameSession::new(definition).unwrap();

        let before = format!("{:?}", session.snapshot());
        let unauthorized = MarketPolicy {
            producer_margin_percent: 44,
            ..MarketPolicy::default()
        };
        assert_eq!(
            session.submit(GameCommand::SetMarketPolicy {
                system: id("core:s1"),
                policy: unauthorized,
            }),
            Err(CoreError::UnauthorizedMarketPolicy)
        );
        assert_eq!(format!("{:?}", session.snapshot()), before);

        let authorized = MarketPolicy {
            producer_margin_percent: 33,
            ..MarketPolicy::default()
        };
        session
            .submit(GameCommand::SetMarketPolicy {
                system: id("core:s0"),
                policy: authorized.clone(),
            })
            .unwrap();
        assert_eq!(session.snapshot().markets[0].policy, authorized);
    }

    #[test]
    fn canonical_market_demand_covers_normal_emergency_and_reserved_funding() {
        let mut session = GameSession::new(definition()).unwrap();
        let system = id("core:s1");
        let ore = id("core:ore");
        let energy = id(ENERGY_ID);

        let normal = session.market_demand(&system, &ore).unwrap();
        assert_eq!(normal.advertised, 10);
        assert_eq!(session.snapshot().markets[1].demand[&ore], normal);

        let entity = session.market_entity(&system).unwrap();
        session
            .world
            .get_mut::<MarketPolicy>(entity)
            .unwrap()
            .operating_reserve_ticks = 0;
        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.set_energy_stock(Energy(40)).unwrap();
            market.reserved_energy = Energy(9);
        }
        let constrained = session.market_demand(&system, &ore).unwrap();
        assert!(constrained.funded < constrained.advertised);
        assert_eq!(session.snapshot().markets[1].demand[&ore], constrained);

        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.operating_profile.stage = BrownoutStage::Emergency;
            market.targets.insert(energy.clone(), 100);
        }
        assert_eq!(
            session.market_demand(&system, &ore).unwrap(),
            MarketDemandSnapshot::default()
        );
        assert!(session.market_demand(&system, &energy).unwrap().advertised > 0);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.markets[1].demand[&ore].advertised, 0);
        assert_eq!(
            snapshot.markets[1].demand[&energy],
            session.market_demand(&system, &energy).unwrap()
        );
    }

    #[test]
    fn operating_reserve_follows_distinct_source_and_recipe_carry_schedules() {
        let mut definition = definition();
        definition.economy.life_support_burn_per_capita = Energy::ZERO;
        definition.systems[0].sources.push(SourceDefinition {
            good: id("core:ore"),
            quantity_per_tick: 1,
            extraction_energy: Energy(5),
        });
        for (recipe, cost) in [("core:r1", 3), ("core:r2", 7)] {
            definition.recipes.push(RecipeDefinition {
                id: id(recipe),
                name: recipe.into(),
                layer: RecipeLayer::Tertiary,
                inputs: vec![GoodAmount {
                    good: id("core:ore"),
                    quantity: 1,
                }],
                outputs: vec![],
                operating_energy: Energy(cost),
                margin_percent: None,
            });
            definition.systems[0].recipes.push(id(recipe));
        }
        let mut session = GameSession::new(definition).unwrap();
        let entity = session.market_entity(&id("core:s0")).unwrap();
        let mut policy = session.world.get::<MarketPolicy>(entity).unwrap().clone();
        policy.operating_reserve_ticks = 4;

        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.operating_profile.throughput_percent = 0;
        }
        assert_eq!(
            session
                .world
                .get::<Market>(entity)
                .unwrap()
                .operating_reserve(&policy, Energy::ZERO)
                .unwrap(),
            Energy::ZERO
        );
        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.operating_profile.throughput_percent = 50;
        }
        assert_eq!(
            session
                .world
                .get::<Market>(entity)
                .unwrap()
                .operating_reserve(&policy, Energy::ZERO)
                .unwrap(),
            Energy(30)
        );
        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.operating_profile.throughput_percent = 100;
        }
        assert_eq!(
            session
                .world
                .get::<Market>(entity)
                .unwrap()
                .operating_reserve(&policy, Energy::ZERO)
                .unwrap(),
            Energy(60)
        );

        policy.operating_reserve_ticks = 1;
        {
            let mut market = session.world.get_mut::<Market>(entity).unwrap();
            market.operating_profile.throughput_percent = 50;
            market
                .throughput_carry
                .insert(ThroughputScheduleKey::Source(id("core:ore")), 5_000);
            for recipe in ["core:r1", "core:r2"] {
                market
                    .throughput_carry
                    .insert(ThroughputScheduleKey::Recipe(id(recipe)), 5_000);
            }
        }
        assert_eq!(
            session
                .world
                .get::<Market>(entity)
                .unwrap()
                .operating_reserve(&policy, Energy::ZERO)
                .unwrap(),
            Energy(15),
            "reserve must begin from each persistent carry without mutating it"
        );
        assert!(
            session
                .world
                .get::<Market>(entity)
                .unwrap()
                .throughput_carry
                .values()
                .all(|carry| *carry == 5_000)
        );
    }

    #[test]
    fn duplicate_market_schedules_are_rejected_by_core() {
        let mut duplicate_source = definition();
        let source = SourceDefinition {
            good: id("core:ore"),
            quantity_per_tick: 1,
            extraction_energy: Energy(1),
        };
        duplicate_source.systems[0].sources = vec![source.clone(), source];
        assert!(matches!(
            GameSession::new(duplicate_source),
            Err(CoreError::InvalidPhysicalDefinition)
        ));

        let mut duplicate_recipe = definition();
        duplicate_recipe.systems[0].recipes = vec![id("core:r"), id("core:r")];
        assert!(matches!(
            GameSession::new(duplicate_recipe),
            Err(CoreError::InvalidPhysicalDefinition)
        ));
    }

    #[test]
    fn emergency_ceiling_and_recovery_ladder_validation_are_ordered() {
        let mut invalid_ceiling = definition();
        invalid_ceiling.systems[0]
            .policy
            .import_priorities
            .insert(id(ENERGY_ID), 2_000);
        assert!(matches!(
            GameSession::new(invalid_ceiling),
            Err(CoreError::InvalidWorldDynamics)
        ));

        let mut session = GameSession::new(definition()).unwrap();
        let mut invalid_policy = MarketPolicy::default();
        invalid_policy
            .import_priorities
            .insert(id(ENERGY_ID), 2_000);
        let before = format!("{:?}", session.snapshot());
        assert_eq!(
            session.submit(GameCommand::SetMarketPolicy {
                system: id("core:s0"),
                policy: invalid_policy,
            }),
            Err(CoreError::InvalidPolicy)
        );
        assert_eq!(format!("{:?}", session.snapshot()), before);

        let mut invalid_recovery = BrownoutConfig::default();
        invalid_recovery.starvation_recovery_ticks = invalid_recovery.emergency_recovery_ticks;
        assert_eq!(
            invalid_recovery.validate(),
            Err(CoreError::InvalidWorldDynamics)
        );
        invalid_recovery = BrownoutConfig::default();
        invalid_recovery.emergency_recovery_ticks = invalid_recovery.throttled_recovery_ticks;
        assert_eq!(
            invalid_recovery.validate(),
            Err(CoreError::InvalidWorldDynamics)
        );
    }

    #[test]
    fn logistic_population_delta_rejects_invalid_inputs_without_mutating_remainder() {
        let mut remainder = 17;
        assert_eq!(
            logistic_population_delta(10, 100, 1, 0, &mut remainder),
            Err(CoreError::InvalidWorldDynamics)
        );
        assert_eq!(remainder, 17);

        let mut invalid_remainder = 100_000;
        assert_eq!(
            logistic_population_delta(10, 100, 1, 1, &mut invalid_remainder),
            Err(CoreError::InvalidWorldDynamics)
        );
        assert_eq!(invalid_remainder, 100_000);

        let mut overflow_remainder = 23;
        assert_eq!(
            logistic_population_delta(u64::MAX / 2, u64::MAX, u32::MAX, 1, &mut overflow_remainder),
            Err(CoreError::Overflow)
        );
        assert_eq!(overflow_remainder, 23);
    }

    #[test]
    fn brownout_history_overflow_is_atomic() {
        let mut session = GameSession::new(definition()).unwrap();
        session
            .world
            .resource_mut::<AggregateDynamicsHistory>()
            .stage_occupancy_ticks[BrownoutStage::Normal.index()] = u64::MAX;
        let before = format!("{:?}", session.snapshot());
        assert_eq!(session.classify_brownouts(), Err(CoreError::Overflow));
        assert_eq!(format!("{:?}", session.snapshot()), before);
        assert!(session.drain_events().is_empty());
    }
}
