//! Physical Energy logistics domain.
//!
//! Root session orchestration remains authoritative for tick scheduling. Only
//! this module's contract executor may mutate Energy logistics state.

// These pure arithmetic seams are exercised before lifecycle orchestration is
// wired into the root session.
#![cfg_attr(not(test), allow(dead_code))]

use super::{
    BrownoutStage, ContentId, CoreError, ENERGY_ID, EconomyConfig, Energy, EventBuffer, GameEvent,
    GameSession, Market, MarketPolicy, StableId, SystemGraph, ThroughputScheduleKey, Trader,
    TravelPlan, composed_throughput, route_travel_energy, scaled_source_output, ticks_for_distance,
};
use bevy_ecs::prelude::{Entity, Resource};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FeeTerms {
    pub carrier_profit: Energy,
    pub carrier_allocation: Energy,
    pub net_delivery: Energy,
    pub effective_freight_bps: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectionTick {
    pub generated: Energy,
    pub life_support: Energy,
    pub operating_burn: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectionResult {
    pub final_stock: Energy,
    pub curtailed: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GrossSizingInput {
    pub offered_payload: Energy,
    pub bulk_headroom: Energy,
    pub candidate_net_cap: Energy,
    pub loaded_route_burn: Energy,
    pub recovery_burn: Energy,
    pub carrier_fee_bps: u32,
    pub max_allocation_bps: u32,
    pub deadhead_burn: Energy,
    pub tank_energy: Energy,
    pub tank_capacity: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementConstants {
    pub gross_payload: Energy,
    pub loaded_route_burn: Energy,
    pub carrier_profit: Energy,
    pub net_delivery: Energy,
    pub recovery_burn: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementState {
    pub cumulative_settled: Energy,
    pub locked_amount: Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementDelta {
    pub settled_now: Energy,
    pub reimbursement_conversion: Energy,
    pub fee_conversion: Energy,
    pub cumulative_after: Energy,
    pub locked_after: Energy,
    pub completed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TimeoutPlan {
    pub reimbursement_conversion: Energy,
    pub recovery_conversion: Energy,
    pub recovery_burn: Energy,
    pub locked_after_departure: Energy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContractId(u64);

impl ContractId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockedEnergyLot {
    pub contract_id: ContractId,
    pub amount: Energy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BulkEnergyHold {
    pub owned: Energy,
    pub locked: Option<LockedEnergyLot>,
}

impl BulkEnergyHold {
    pub fn used(self) -> Result<Energy, CoreError> {
        checked_sum_energy(&[
            self.owned,
            self.locked.map_or(Energy::ZERO, |lot| lot.amount),
        ])
    }

    pub fn headroom(self, capacity: Energy) -> Result<Energy, CoreError> {
        require_non_negative(&[capacity])?;
        let used = self.used()?;
        if used > capacity {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        capacity.checked_sub(used)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierFeeSchedule {
    pub normal: u32,
    pub throttled: u32,
    pub emergency: u32,
    pub starvation: u32,
}

impl Default for CarrierFeeSchedule {
    fn default() -> Self {
        Self {
            normal: 50,
            throttled: 100,
            emergency: 200,
            starvation: 300,
        }
    }
}

impl CarrierFeeSchedule {
    #[must_use]
    pub const fn for_stage(self, stage: BrownoutStage) -> u32 {
        match stage {
            BrownoutStage::Normal => self.normal,
            BrownoutStage::Throttled => self.throttled,
            BrownoutStage::Emergency => self.emergency,
            BrownoutStage::Starvation => self.starvation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnergyLogisticsPolicy {
    pub carrier_fee_bps: CarrierFeeSchedule,
    pub max_allocation_bps: u32,
    pub curtailment_projection_window: u32,
    pub export_reserve: Energy,
    pub authored_export_base: Energy,
    pub settlement_timeout_ticks: u32,
}

impl Default for EnergyLogisticsPolicy {
    fn default() -> Self {
        Self {
            carrier_fee_bps: CarrierFeeSchedule::default(),
            max_allocation_bps: 1_000,
            curtailment_projection_window: 20,
            export_reserve: Energy::ZERO,
            authored_export_base: Energy::ZERO,
            settlement_timeout_ticks: 20,
        }
    }
}

impl EnergyLogisticsPolicy {
    pub fn validate(self) -> Result<(), CoreError> {
        let fees = self.carrier_fee_bps;
        if !(fees.normal < fees.throttled
            && fees.throttled < fees.emergency
            && fees.emergency < fees.starvation)
            || self.max_allocation_bps == 0
            || self.max_allocation_bps > 10_000
            || fees.starvation >= self.max_allocation_bps
            || self.curtailment_projection_window == 0
            || self.settlement_timeout_ticks == 0
            || self.export_reserve.0 < 0
            || self.authored_export_base.0 < 0
        {
            return Err(CoreError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractRoute {
    pub systems: Vec<ContentId>,
    pub burn: Energy,
    pub ticks: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyContractState {
    DeadheadingToSource {
        source_claim: Energy,
        accepted_tick: u64,
    },
    InTransit {
        loaded_tick: u64,
    },
    Arrived {
        arrived_tick: u64,
        settlement_deadline: u64,
    },
    Recovering {
        recovery_departure_tick: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyContract {
    pub id: ContractId,
    pub carrier: ContentId,
    pub source: ContentId,
    pub destination: ContentId,
    pub deadhead_route: ContractRoute,
    pub loaded_route: ContractRoute,
    pub recovery_route: ContractRoute,
    pub gross_payload: Energy,
    pub carrier_fee_bps: u32,
    pub carrier_profit: Energy,
    pub net_delivery: Energy,
    pub cumulative_settled: Energy,
    pub state: EnergyContractState,
    pub latest_blocker: Option<EnergyContractBlocker>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyContractBlocker {
    NoReachableSurplus,
    NoViableCandidate,
    ViableButUnaccepted,
    ArrivedSettlementBlocked,
    AcceptedDeliveryPending,
    StaleMaximum,
    SourceClaimRevoked,
    StorageHeadroom,
    RecoveryReserve,
    Integrity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyContractTerminalOutcome {
    Completed,
    CancelledBeforeLoad,
    RevokedBeforeLoad,
    RejectedBeforeLoad,
    RecoveredAfterFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyContractIntent {
    pub carrier: ContentId,
    pub source: ContentId,
    pub destination: ContentId,
    pub gross_payload: Energy,
    pub command_driven: bool,
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct PendingEnergyContractIntents(pub Vec<EnergyContractIntent>);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnergyLogisticsDiagnostics {
    pub accepted: u64,
    pub completed: u64,
    pub cancelled_before_load: u64,
    pub revoked_before_load: u64,
    pub rejected_before_load: u64,
    pub recovered_after_failure: u64,
    pub recovery_curtailed: Energy,
    pub arrived_settlement_blocked: u64,
    pub accepted_delivery_pending: u64,
    pub no_reachable_surplus: u64,
    pub no_viable_candidate: u64,
    pub viable_but_unaccepted: u64,
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct EnergyContracts {
    next_id: u64,
    pub active: BTreeMap<ContractId, EnergyContract>,
    pub diagnostics: EnergyLogisticsDiagnostics,
}

impl EnergyContracts {
    pub fn allocate_id(&mut self) -> Result<ContractId, CoreError> {
        let next_id = self.next_id.checked_add(1).ok_or(CoreError::Overflow)?;
        self.next_id = next_id;
        Ok(ContractId(next_id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyContractSnapshot {
    pub contract: EnergyContract,
    pub locked_amount: Energy,
    pub converted_reimbursement: Energy,
    pub converted_fee: Energy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnergyContractEvent {
    Accepted {
        contract_id: ContractId,
    },
    Loaded {
        contract_id: ContractId,
    },
    Departed {
        contract_id: ContractId,
    },
    Settled {
        contract_id: ContractId,
        amount: Energy,
    },
    RecoveryCurtailed {
        contract_id: ContractId,
        source: ContentId,
        amount: Energy,
    },
    Terminal {
        contract_id: ContractId,
        outcome: EnergyContractTerminalOutcome,
    },
    Rejected {
        blocker: EnergyContractBlocker,
        current_maximum: Option<Energy>,
    },
    OwnedBulkTransferredToTank {
        trader: ContentId,
        amount: Energy,
    },
    OwnedBulkDepositedToMarket {
        trader: ContentId,
        system: ContentId,
        amount: Energy,
    },
}

fn require_non_negative(values: &[Energy]) -> Result<(), CoreError> {
    if values.iter().any(|value| value.0 < 0) {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    Ok(())
}

fn energy_from_i128(value: i128) -> Result<Energy, CoreError> {
    if value < 0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    Ok(Energy(
        i64::try_from(value).map_err(|_| CoreError::Overflow)?,
    ))
}

fn checked_sum_energy(values: &[Energy]) -> Result<Energy, CoreError> {
    require_non_negative(values)?;
    let sum = values.iter().try_fold(0_i128, |sum, value| {
        sum.checked_add(i128::from(value.0))
            .ok_or(CoreError::Overflow)
    })?;
    energy_from_i128(sum)
}

pub(crate) fn exportable_energy(
    stock: Energy,
    ordinary_payment_claims: Energy,
    preload_export_claims: Energy,
    operating_reserve: Energy,
    protected_liquidation_budget: Energy,
    export_reserve: Energy,
) -> Result<Energy, CoreError> {
    require_non_negative(&[
        stock,
        ordinary_payment_claims,
        preload_export_claims,
        operating_reserve,
        protected_liquidation_budget,
        export_reserve,
    ])?;
    let protected = checked_sum_energy(&[
        ordinary_payment_claims,
        preload_export_claims,
        operating_reserve,
        protected_liquidation_budget,
        export_reserve,
    ])?;
    let exportable = i128::from(stock.0)
        .checked_sub(i128::from(protected.0))
        .ok_or(CoreError::Overflow)?
        .max(0);
    energy_from_i128(exportable)
}

pub(crate) fn project_energy(
    start: Energy,
    storage_cap: Energy,
    ticks: &[ProjectionTick],
) -> Result<ProjectionResult, CoreError> {
    require_non_negative(&[start, storage_cap])?;
    let mut stock = start;
    let mut curtailed = Energy::ZERO;

    for tick in ticks {
        require_non_negative(&[tick.generated, tick.life_support, tick.operating_burn])?;
        let gross = checked_sum_energy(&[stock, tick.generated])?;
        let tick_curtailed = energy_from_i128(
            i128::from(gross.0)
                .checked_sub(i128::from(storage_cap.0))
                .ok_or(CoreError::Overflow)?
                .max(0),
        )?;
        curtailed = checked_sum_energy(&[curtailed, tick_curtailed])?;
        let stored = Energy(gross.0.min(storage_cap.0));
        let mandatory_burn = checked_sum_energy(&[tick.life_support, tick.operating_burn])?;
        stock = energy_from_i128(
            i128::from(stored.0)
                .checked_sub(i128::from(mandatory_burn.0))
                .ok_or(CoreError::Overflow)?
                .max(0),
        )?;
    }

    Ok(ProjectionResult {
        final_stock: stock,
        curtailed,
    })
}

pub(crate) fn fee_terms(
    gross_payload: Energy,
    loaded_route_burn: Energy,
    carrier_fee_bps: u32,
) -> Result<FeeTerms, CoreError> {
    require_non_negative(&[gross_payload, loaded_route_burn])?;
    if gross_payload.0 == 0 || carrier_fee_bps >= 10_000 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }

    let gross = i128::from(gross_payload.0);
    let profit = gross
        .checked_mul(i128::from(carrier_fee_bps))
        .ok_or(CoreError::Overflow)?
        / 10_000;
    let allocation = i128::from(loaded_route_burn.0)
        .checked_add(profit)
        .ok_or(CoreError::Overflow)?;
    let allocation_energy = energy_from_i128(allocation)?;
    if allocation > gross {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let net_delivery = energy_from_i128(gross.checked_sub(allocation).ok_or(CoreError::Overflow)?)?;
    let freight_numerator = allocation.checked_mul(10_000).ok_or(CoreError::Overflow)?;
    let effective_freight_bps = freight_numerator
        .checked_add(gross - 1)
        .ok_or(CoreError::Overflow)?
        / gross;

    Ok(FeeTerms {
        carrier_profit: energy_from_i128(profit)?,
        carrier_allocation: allocation_energy,
        net_delivery,
        effective_freight_bps: u32::try_from(effective_freight_bps)
            .map_err(|_| CoreError::Overflow)?,
    })
}

fn candidate_is_viable(input: GrossSizingInput, gross: Energy) -> Result<bool, CoreError> {
    let terms = match fee_terms(gross, input.loaded_route_burn, input.carrier_fee_bps) {
        Ok(terms) => terms,
        Err(CoreError::InvalidPhysicalDefinition) => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(terms.net_delivery.0 <= input.candidate_net_cap.0
        && terms.net_delivery.0 > input.recovery_burn.0
        && terms.effective_freight_bps <= input.max_allocation_bps
        && terms.carrier_profit.0 > input.deadhead_burn.0)
}

pub(crate) fn largest_viable_gross(input: GrossSizingInput) -> Result<Option<Energy>, CoreError> {
    require_non_negative(&[
        input.offered_payload,
        input.bulk_headroom,
        input.candidate_net_cap,
        input.loaded_route_burn,
        input.recovery_burn,
        input.deadhead_burn,
        input.tank_energy,
        input.tank_capacity,
    ])?;
    if input.carrier_fee_bps >= 10_000
        || input.max_allocation_bps == 0
        || input.max_allocation_bps >= 10_000
        || input.carrier_fee_bps >= input.max_allocation_bps
    {
        return Err(CoreError::InvalidPhysicalDefinition);
    }

    let required_tank = checked_sum_energy(&[input.deadhead_burn, input.loaded_route_burn])?;
    if input.tank_energy.0 < required_tank.0 || input.tank_capacity.0 < input.recovery_burn.0 {
        return Ok(None);
    }

    let upper = input.offered_payload.0.min(input.bulk_headroom.0);
    if upper == 0 || input.carrier_fee_bps == 0 {
        return Ok(None);
    }

    // Gross-to-net is monotonic despite fee-floor plateaus. First find the
    // greatest payload under the net cap in logarithmically bounded steps.
    let mut low = 1_i128;
    let mut high = i128::from(upper);
    let mut largest_under_net_cap = None;
    while low <= high {
        let middle = low + (high - low) / 2;
        let profit = middle
            .checked_mul(i128::from(input.carrier_fee_bps))
            .ok_or(CoreError::Overflow)?
            / 10_000;
        let net = middle
            .checked_sub(i128::from(input.loaded_route_burn.0))
            .and_then(|value| value.checked_sub(profit))
            .ok_or(CoreError::Overflow)?;
        if net <= i128::from(input.candidate_net_cap.0) {
            largest_under_net_cap = Some(middle);
            low = middle.checked_add(1).ok_or(CoreError::Overflow)?;
        } else {
            high = middle - 1;
        }
    }

    let Some(candidate) = largest_under_net_cap else {
        return Ok(None);
    };
    let candidate = energy_from_i128(candidate)?;
    if candidate_is_viable(input, candidate)? {
        return Ok(Some(candidate));
    }

    // Freight ceil can jump at a fee-floor boundary even though net delivery
    // remains monotonic. If the greatest payload is on such a jump, the only
    // possible lower maximum is the end of the preceding fee bucket.
    let candidate_profit = i128::from(candidate.0)
        .checked_mul(i128::from(input.carrier_fee_bps))
        .ok_or(CoreError::Overflow)?
        / 10_000;
    if candidate_profit == 0 {
        return Ok(None);
    }
    let previous_bucket_end = candidate_profit
        .checked_mul(10_000)
        .ok_or(CoreError::Overflow)?
        .checked_add(i128::from(input.carrier_fee_bps) - 1)
        .ok_or(CoreError::Overflow)?
        / i128::from(input.carrier_fee_bps)
        - 1;
    let fallback = previous_bucket_end.min(i128::from(candidate.0) - 1);
    if fallback <= 0 {
        return Ok(None);
    }
    let fallback = energy_from_i128(fallback)?;
    if candidate_is_viable(input, fallback)? {
        Ok(Some(fallback))
    } else {
        Ok(None)
    }
}

pub(crate) fn opportunity_score(
    carrier_profit: Energy,
    deadhead_burn: Energy,
    deadhead_ticks: u32,
    loaded_ticks: u32,
) -> Result<Option<i128>, CoreError> {
    require_non_negative(&[carrier_profit, deadhead_burn])?;
    let net_profit = i128::from(carrier_profit.0)
        .checked_sub(i128::from(deadhead_burn.0))
        .ok_or(CoreError::Overflow)?;
    if net_profit <= 0 {
        return Ok(None);
    }
    let opportunity_ticks = i128::from(deadhead_ticks)
        .checked_add(i128::from(loaded_ticks))
        .ok_or(CoreError::Overflow)?
        .max(1);
    let score = net_profit
        .checked_mul(1_000_000)
        .ok_or(CoreError::Overflow)?
        / opportunity_ticks;
    Ok((score > 0).then_some(score))
}

fn validate_settlement(
    constants: SettlementConstants,
    state: SettlementState,
) -> Result<Energy, CoreError> {
    require_non_negative(&[
        constants.gross_payload,
        constants.loaded_route_burn,
        constants.carrier_profit,
        constants.net_delivery,
        constants.recovery_burn,
        state.cumulative_settled,
        state.locked_amount,
    ])?;
    if constants.gross_payload.0 == 0
        || constants.net_delivery.0 == 0
        || constants.net_delivery.0 <= constants.recovery_burn.0
        || state.cumulative_settled.0 > constants.net_delivery.0
    {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let gross_sum = checked_sum_energy(&[
        constants.loaded_route_burn,
        constants.carrier_profit,
        constants.net_delivery,
    ])?;
    if gross_sum != constants.gross_payload {
        return Err(CoreError::InvalidPhysicalDefinition);
    }

    let fee = earned_fee(constants, state.cumulative_settled)?;
    let reimbursement = if state.cumulative_settled.0 > 0 {
        constants.loaded_route_burn
    } else {
        Energy::ZERO
    };
    let expected_locked = i128::from(constants.gross_payload.0)
        .checked_sub(i128::from(state.cumulative_settled.0))
        .and_then(|value| value.checked_sub(i128::from(reimbursement.0)))
        .and_then(|value| value.checked_sub(i128::from(fee.0)))
        .ok_or(CoreError::Overflow)?;
    if expected_locked < 0 || energy_from_i128(expected_locked)? != state.locked_amount {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    Ok(fee)
}

fn earned_fee(
    constants: SettlementConstants,
    cumulative_settled: Energy,
) -> Result<Energy, CoreError> {
    let fee = i128::from(constants.carrier_profit.0)
        .checked_mul(i128::from(cumulative_settled.0))
        .ok_or(CoreError::Overflow)?
        / i128::from(constants.net_delivery.0);
    energy_from_i128(fee)
}

fn prepared_settlement(
    constants: SettlementConstants,
    state: SettlementState,
    settled_now: Energy,
    fee_before: Energy,
) -> Result<SettlementDelta, CoreError> {
    let cumulative_after = energy_from_i128(
        i128::from(state.cumulative_settled.0)
            .checked_add(i128::from(settled_now.0))
            .ok_or(CoreError::Overflow)?,
    )?;
    let reimbursement_conversion = if state.cumulative_settled.0 == 0 && settled_now.0 > 0 {
        constants.loaded_route_burn
    } else {
        Energy::ZERO
    };
    let fee_after = earned_fee(constants, cumulative_after)?;
    let fee_conversion = energy_from_i128(
        i128::from(fee_after.0)
            .checked_sub(i128::from(fee_before.0))
            .ok_or(CoreError::Overflow)?,
    )?;
    let locked_after = energy_from_i128(
        i128::from(state.locked_amount.0)
            .checked_sub(i128::from(settled_now.0))
            .and_then(|value| value.checked_sub(i128::from(reimbursement_conversion.0)))
            .and_then(|value| value.checked_sub(i128::from(fee_conversion.0)))
            .ok_or(CoreError::Overflow)?,
    )?;

    Ok(SettlementDelta {
        settled_now,
        reimbursement_conversion,
        fee_conversion,
        cumulative_after,
        locked_after,
        completed: cumulative_after == constants.net_delivery,
    })
}

pub(crate) fn settlement_delta(
    constants: SettlementConstants,
    state: SettlementState,
    destination_headroom: Energy,
) -> Result<SettlementDelta, CoreError> {
    require_non_negative(&[destination_headroom])?;
    let fee_before = validate_settlement(constants, state)?;
    let remaining_net = energy_from_i128(
        i128::from(constants.net_delivery.0)
            .checked_sub(i128::from(state.cumulative_settled.0))
            .ok_or(CoreError::Overflow)?,
    )?;

    if remaining_net.0 == 0 || destination_headroom.0 == 0 {
        return Ok(SettlementDelta {
            settled_now: Energy::ZERO,
            reimbursement_conversion: Energy::ZERO,
            fee_conversion: Energy::ZERO,
            cumulative_after: state.cumulative_settled,
            locked_after: state.locked_amount,
            completed: remaining_net.0 == 0,
        });
    }

    // A complete delivery has priority over the contingent recovery reserve.
    if destination_headroom.0 >= remaining_net.0 {
        return prepared_settlement(constants, state, remaining_net, fee_before);
    }

    // For an incomplete delivery, locked_after is monotonic non-increasing as
    // settlement grows. Binary search the maximal reserve-preserving amount.
    let mut low = 1_i128;
    let mut high = i128::from(destination_headroom.0);
    let mut best = None;
    while low <= high {
        let middle = low + (high - low) / 2;
        let delta = prepared_settlement(constants, state, energy_from_i128(middle)?, fee_before);
        match delta {
            Ok(delta) if delta.locked_after.0 >= constants.recovery_burn.0 => {
                best = Some(delta);
                low = middle.checked_add(1).ok_or(CoreError::Overflow)?;
            }
            Ok(_) | Err(CoreError::InvalidPhysicalDefinition) => high = middle - 1,
            Err(error) => return Err(error),
        }
    }

    Ok(best.unwrap_or(SettlementDelta {
        settled_now: Energy::ZERO,
        reimbursement_conversion: Energy::ZERO,
        fee_conversion: Energy::ZERO,
        cumulative_after: state.cumulative_settled,
        locked_after: state.locked_amount,
        completed: false,
    }))
}

pub(crate) fn timeout_plan(
    constants: SettlementConstants,
    state: SettlementState,
) -> Result<TimeoutPlan, CoreError> {
    validate_settlement(constants, state)?;
    if state.cumulative_settled.0 >= constants.net_delivery.0 {
        return Err(CoreError::InvalidPhysicalDefinition);
    }
    let reimbursement_conversion = if state.cumulative_settled.0 == 0 {
        constants.loaded_route_burn
    } else {
        Energy::ZERO
    };
    let locked_after_departure = energy_from_i128(
        i128::from(state.locked_amount.0)
            .checked_sub(i128::from(reimbursement_conversion.0))
            .and_then(|value| value.checked_sub(i128::from(constants.recovery_burn.0)))
            .ok_or(CoreError::Overflow)?,
    )?;

    Ok(TimeoutPlan {
        reimbursement_conversion,
        recovery_conversion: constants.recovery_burn,
        recovery_burn: constants.recovery_burn,
        locked_after_departure,
    })
}

#[derive(Clone, Debug)]
struct PreparedEnergyCandidate {
    carrier_entity: Entity,
    source_entity: Entity,
    deadhead_route: ContractRoute,
    loaded_route: ContractRoute,
    recovery_route: ContractRoute,
    gross_payload: Energy,
    carrier_fee_bps: u32,
    carrier_profit: Energy,
    net_delivery: Energy,
}

#[derive(Clone, Copy, Debug)]
struct EnergyIntentRejection {
    blocker: EnergyContractBlocker,
    current_maximum: Option<Energy>,
}

#[derive(Clone, Copy, Debug)]
struct EnergyIntentOrderKey {
    destination_stage: BrownoutStage,
    destination_runway: u64,
}

impl GameSession {
    pub(super) fn enqueue_player_energy_contract(
        &mut self,
        source: ContentId,
        destination: ContentId,
        gross_payload: Energy,
    ) -> Result<(), CoreError> {
        if gross_payload.0 <= 0 {
            return Err(CoreError::ZeroQuantity);
        }
        if source == destination {
            return Err(CoreError::AlreadyThere);
        }
        self.market_entity(&source)?;
        self.market_entity(&destination)?;
        let carrier_entity = self.player_entity()?;
        let carrier = self
            .world
            .get::<StableId>(carrier_entity)
            .unwrap()
            .0
            .clone();
        self.world
            .resource_mut::<PendingEnergyContractIntents>()
            .0
            .push(EnergyContractIntent {
                carrier,
                source,
                destination,
                gross_payload,
                command_driven: true,
            });
        Ok(())
    }

    pub(super) fn cancel_player_energy_contract(
        &mut self,
        contract_id: ContractId,
    ) -> Result<(), CoreError> {
        let player_entity = self.player_entity()?;
        let player = self.world.get::<StableId>(player_entity).unwrap().0.clone();
        let contract = self
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .cloned()
            .ok_or_else(|| CoreError::Unknown {
                kind: "energy contract",
                id: contract_id.get().to_string(),
            })?;
        if contract.carrier != player
            || !matches!(
                contract.state,
                EnergyContractState::DeadheadingToSource { .. }
            )
        {
            return Err(CoreError::ActiveEnergyContract);
        }
        if self
            .world
            .get::<Trader>(player_entity)
            .unwrap()
            .bulk_energy
            .locked
            .is_some()
        {
            return Err(CoreError::LockedEnergy);
        }

        let mut contracts = self.world.resource::<EnergyContracts>().clone();
        contracts.diagnostics.cancelled_before_load = contracts
            .diagnostics
            .cancelled_before_load
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        contracts.active.remove(&contract_id);
        *self.world.resource_mut::<EnergyContracts>() = contracts;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
                contract_id,
                outcome: EnergyContractTerminalOutcome::CancelledBeforeLoad,
            }));
        Ok(())
    }

    fn trader_entity_by_id(&mut self, id: &ContentId) -> Result<Entity, CoreError> {
        self.world
            .query::<(Entity, &StableId, &Trader)>()
            .iter(&self.world)
            .find(|(_, stable, _)| &stable.0 == id)
            .map(|(entity, _, _)| entity)
            .ok_or_else(|| CoreError::Unknown {
                kind: "trader",
                id: id.to_string(),
            })
    }

    pub(super) fn carrier_has_active_energy_contract(&self, carrier: &ContentId) -> bool {
        self.world
            .resource::<EnergyContracts>()
            .active
            .values()
            .any(|contract| &contract.carrier == carrier)
    }

    pub(super) fn ensure_carrier_contract_free(
        &self,
        carrier: &ContentId,
        trader: &Trader,
    ) -> Result<(), CoreError> {
        if self.carrier_has_active_energy_contract(carrier) {
            return Err(CoreError::ActiveEnergyContract);
        }
        if trader.bulk_energy.locked.is_some() {
            return Err(CoreError::LockedEnergy);
        }
        Ok(())
    }

    fn checked_energy_from_i128(value: i128) -> Result<Energy, CoreError> {
        if value < 0 {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        Ok(Energy(
            i64::try_from(value).map_err(|_| CoreError::Overflow)?,
        ))
    }

    fn route_ticks(graph: &SystemGraph, route: &[ContentId], speed: f64) -> Result<u32, CoreError> {
        if !speed.is_finite() || speed <= 0.0 || route.is_empty() {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        route.windows(2).try_fold(0_u32, |total, leg| {
            let distance = graph
                .neighbors(&leg[0])
                .iter()
                .find(|(id, _)| id == &leg[1])
                .map(|(_, distance)| *distance)
                .ok_or(CoreError::NoRoute)?;
            total
                .checked_add(ticks_for_distance(distance, speed))
                .ok_or(CoreError::Overflow)
        })
    }

    fn snapshot_contract_route(
        &self,
        start: &ContentId,
        destination: &ContentId,
        burn_per_distance: Energy,
        speed: f64,
    ) -> Result<ContractRoute, CoreError> {
        let (systems, _) = self
            .graph()
            .shortest_path(start, destination)
            .ok_or(CoreError::NoRoute)?;
        Ok(ContractRoute {
            burn: route_travel_energy(self.graph(), &systems, burn_per_distance)?,
            ticks: Self::route_ticks(self.graph(), &systems, speed)?,
            systems,
        })
    }

    fn projection_ticks_for_market(
        &self,
        market: &Market,
        count: u32,
    ) -> Result<Vec<ProjectionTick>, CoreError> {
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita
            .checked_mul(market.population)?;
        let mut carry = market.throughput_carry.clone();
        let mut ticks =
            Vec::with_capacity(usize::try_from(count).map_err(|_| CoreError::Overflow)?);
        for offset in 1..=count {
            let projected_tick = self
                .tick()
                .checked_add(u64::from(offset))
                .ok_or(CoreError::Overflow)?;
            let generated = market
                .seasonal_generation
                .effective_output_at(projected_tick)?;
            let mut operating_burn = Energy::ZERO;
            for source in &market.sources {
                let base_output =
                    scaled_source_output(source.quantity_per_tick, market.source_output_percent)?;
                let output = composed_throughput(
                    u64::from(base_output),
                    market.operating_profile.throughput_percent,
                    market.operating_profile.labor_percent,
                    carry
                        .entry(ThroughputScheduleKey::Source(source.good.clone()))
                        .or_insert(0),
                )?;
                operating_burn =
                    operating_burn.checked_add(source.extraction_energy.checked_mul(output)?)?;
            }
            for (recipe, operating_energy) in &market.recipe_operating_energy {
                let executions = composed_throughput(
                    1,
                    market.operating_profile.throughput_percent,
                    market.operating_profile.labor_percent,
                    carry
                        .entry(ThroughputScheduleKey::Recipe(recipe.clone()))
                        .or_insert(0),
                )?;
                operating_burn =
                    operating_burn.checked_add(operating_energy.checked_mul(executions)?)?;
            }
            ticks.push(ProjectionTick {
                generated,
                life_support: life,
                operating_burn,
            });
        }
        Ok(ticks)
    }

    fn offered_energy_payload(&self, source_entity: Entity) -> Result<Energy, CoreError> {
        let market = self.world.get::<Market>(source_entity).unwrap();
        let exportable = self.market_exportable_energy(source_entity)?;
        let ticks = self.projection_ticks_for_market(
            market,
            market.energy_logistics.curtailment_projection_window,
        )?;
        let projection = project_energy(market.energy_stock()?, market.energy_storage_cap, &ticks)?;
        let pressure = projection
            .curtailed
            .checked_add(market.energy_logistics.authored_export_base)?;
        Ok(Energy(exportable.0.min(pressure.0)))
    }

    fn remaining_travel_ticks(&self, trader: &Trader) -> Result<u32, CoreError> {
        let Some(travel) = &trader.travel else {
            return Ok(0);
        };
        if travel.next_leg == 0
            || travel.next_leg >= travel.route.len()
            || travel.route.last() != Some(&travel.destination)
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let mut remaining = travel.remaining_ticks;
        for leg_index in travel.next_leg..travel.route.len() - 1 {
            let leg = &travel.route[leg_index..=leg_index + 1];
            remaining = remaining
                .checked_add(Self::route_ticks(self.graph(), leg, trader.speed)?)
                .ok_or(CoreError::Overflow)?;
        }
        Ok(remaining)
    }

    fn inbound_energy_at_horizon(
        &mut self,
        destination: &ContentId,
        horizon: u32,
    ) -> Result<(Energy, Energy), CoreError> {
        let active = self
            .world
            .resource::<EnergyContracts>()
            .active
            .values()
            .filter(|contract| &contract.destination == destination)
            .cloned()
            .collect::<Vec<_>>();
        let mut committed = Energy::ZERO;
        let mut arriving_by_horizon = Energy::ZERO;
        for contract in active {
            if matches!(contract.state, EnergyContractState::Recovering { .. }) {
                continue;
            }
            let remaining = contract
                .net_delivery
                .checked_sub(contract.cumulative_settled)?;
            committed = committed.checked_add(remaining)?;
            let arrival_ticks = match contract.state {
                EnergyContractState::DeadheadingToSource { .. } => {
                    let carrier = self.trader_entity_by_id(&contract.carrier)?;
                    self.remaining_travel_ticks(self.world.get::<Trader>(carrier).unwrap())?
                        .checked_add(contract.loaded_route.ticks)
                        .ok_or(CoreError::Overflow)?
                }
                EnergyContractState::InTransit { .. } => {
                    let carrier = self.trader_entity_by_id(&contract.carrier)?;
                    self.remaining_travel_ticks(self.world.get::<Trader>(carrier).unwrap())?
                }
                EnergyContractState::Arrived { .. } => 0,
                EnergyContractState::Recovering { .. } => unreachable!(),
            };
            if arrival_ticks <= horizon {
                arriving_by_horizon = arriving_by_horizon.checked_add(remaining)?;
            }
        }
        Ok((committed, arriving_by_horizon))
    }

    fn projected_destination_context(
        &mut self,
        destination_entity: Entity,
        horizon: u32,
    ) -> Result<(Energy, Energy, Energy), CoreError> {
        let destination_id = self
            .world
            .get::<StableId>(destination_entity)
            .unwrap()
            .0
            .clone();
        let market = self
            .world
            .get::<Market>(destination_entity)
            .unwrap()
            .clone();
        let ticks = self.projection_ticks_for_market(&market, horizon)?;
        let projected = project_energy(market.energy_stock()?, market.energy_storage_cap, &ticks)?;
        let (committed, prior_at_arrival) =
            self.inbound_energy_at_horizon(&destination_id, horizon)?;
        let occupied_at_arrival = projected.final_stock.checked_add(prior_at_arrival)?;
        let arrival_headroom = Self::checked_energy_from_i128(
            i128::from(market.energy_storage_cap.0)
                .checked_sub(i128::from(occupied_at_arrival.0))
                .ok_or(CoreError::Overflow)?
                .max(0),
        )?;
        Ok((committed, prior_at_arrival, arrival_headroom))
    }

    fn energy_intent_order_key(
        &mut self,
        intent: &EnergyContractIntent,
    ) -> Result<EnergyIntentOrderKey, CoreError> {
        let destination_entity = self.market_entity(&intent.destination)?;
        let destination = self
            .world
            .get::<Market>(destination_entity)
            .unwrap()
            .clone();
        let carrier_entity = self.trader_entity_by_id(&intent.carrier)?;
        let carrier = self.world.get::<Trader>(carrier_entity).unwrap().clone();
        let deadhead = self.snapshot_contract_route(
            &carrier.system,
            &intent.source,
            carrier.travel_burn_per_distance,
            carrier.speed,
        )?;
        let loaded = self.snapshot_contract_route(
            &intent.source,
            &intent.destination,
            carrier.travel_burn_per_distance,
            carrier.speed,
        )?;
        let horizon = deadhead
            .ticks
            .checked_add(loaded.ticks)
            .ok_or(CoreError::Overflow)?;
        let (_, prior, _) = self.projected_destination_context(destination_entity, horizon)?;
        let ticks = self.projection_ticks_for_market(&destination, horizon)?;
        let physical = project_energy(
            destination.energy_stock()?,
            destination.energy_storage_cap,
            &ticks,
        )?
        .final_stock;
        let occupancy = physical.checked_add(prior)?;
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita
            .checked_mul(destination.population)?;
        let destination_runway = if life == Energy::ZERO {
            u64::MAX
        } else {
            u64::try_from(occupancy.0 / life.0).map_err(|_| CoreError::Overflow)?
        };
        Ok(EnergyIntentOrderKey {
            destination_stage: destination.brownout.stage,
            destination_runway,
        })
    }

    fn evaluate_energy_intent(
        &mut self,
        intent: &EnergyContractIntent,
    ) -> Result<Result<PreparedEnergyCandidate, EnergyIntentRejection>, CoreError> {
        let carrier_entity = self.trader_entity_by_id(&intent.carrier)?;
        let carrier = self.world.get::<Trader>(carrier_entity).unwrap().clone();
        if carrier.travel.is_some()
            || carrier.reservation.is_some()
            || self
                .ensure_carrier_contract_free(&intent.carrier, &carrier)
                .is_err()
        {
            return Ok(Err(EnergyIntentRejection {
                blocker: EnergyContractBlocker::NoViableCandidate,
                current_maximum: None,
            }));
        }
        if intent.source == intent.destination {
            return Ok(Err(EnergyIntentRejection {
                blocker: EnergyContractBlocker::NoViableCandidate,
                current_maximum: None,
            }));
        }

        let source_entity = self.market_entity(&intent.source)?;
        let destination_entity = self.market_entity(&intent.destination)?;
        let source_offer = self.offered_energy_payload(source_entity)?;
        let deadhead_route = self.snapshot_contract_route(
            &carrier.system,
            &intent.source,
            carrier.travel_burn_per_distance,
            carrier.speed,
        )?;
        let loaded_route = self.snapshot_contract_route(
            &intent.source,
            &intent.destination,
            carrier.travel_burn_per_distance,
            carrier.speed,
        )?;
        let recovery_route = self.snapshot_contract_route(
            &intent.destination,
            &intent.source,
            carrier.travel_burn_per_distance,
            carrier.speed,
        )?;
        let horizon = deadhead_route
            .ticks
            .checked_add(loaded_route.ticks)
            .ok_or(CoreError::Overflow)?;
        let (committed_inbound, _, arrival_headroom) =
            self.projected_destination_context(destination_entity, horizon)?;
        let destination = self.world.get::<Market>(destination_entity).unwrap();
        let energy_id = ContentId::new(ENERGY_ID).expect("constant energy id");
        let Some(target) = destination.targets.get(&energy_id).copied() else {
            return Ok(Err(EnergyIntentRejection {
                blocker: EnergyContractBlocker::NoViableCandidate,
                current_maximum: None,
            }));
        };
        let remaining_requested = Self::checked_energy_from_i128(
            i128::from(target)
                .checked_sub(i128::from(destination.energy_stock()?.0))
                .and_then(|value| value.checked_sub(i128::from(committed_inbound.0)))
                .ok_or(CoreError::Overflow)?
                .max(0),
        )?;
        let candidate_net_cap = Energy(remaining_requested.0.min(arrival_headroom.0));
        let bulk_headroom = carrier.bulk_energy.headroom(carrier.bulk_energy_capacity)?;
        let carrier_fee_bps = destination
            .energy_logistics
            .carrier_fee_bps
            .for_stage(destination.brownout.stage);
        let sizing = GrossSizingInput {
            offered_payload: source_offer,
            bulk_headroom,
            candidate_net_cap,
            loaded_route_burn: loaded_route.burn,
            recovery_burn: recovery_route.burn,
            carrier_fee_bps,
            max_allocation_bps: destination.energy_logistics.max_allocation_bps,
            deadhead_burn: deadhead_route.burn,
            tank_energy: carrier.energy_tank,
            tank_capacity: carrier.energy_tank_capacity,
        };
        let Some(current_maximum) = largest_viable_gross(sizing)? else {
            return Ok(Err(EnergyIntentRejection {
                blocker: if source_offer == Energy::ZERO {
                    EnergyContractBlocker::NoReachableSurplus
                } else {
                    EnergyContractBlocker::NoViableCandidate
                },
                current_maximum: None,
            }));
        };
        if intent.gross_payload > current_maximum {
            return Ok(Err(EnergyIntentRejection {
                blocker: EnergyContractBlocker::StaleMaximum,
                current_maximum: Some(current_maximum),
            }));
        }
        let exact_sizing = GrossSizingInput {
            offered_payload: intent.gross_payload,
            ..sizing
        };
        if largest_viable_gross(exact_sizing)? != Some(intent.gross_payload) {
            return Ok(Err(EnergyIntentRejection {
                blocker: EnergyContractBlocker::NoViableCandidate,
                current_maximum: Some(current_maximum),
            }));
        }
        let terms = fee_terms(intent.gross_payload, loaded_route.burn, carrier_fee_bps)?;
        Ok(Ok(PreparedEnergyCandidate {
            carrier_entity,
            source_entity,
            deadhead_route,
            loaded_route,
            recovery_route,
            gross_payload: intent.gross_payload,
            carrier_fee_bps,
            carrier_profit: terms.carrier_profit,
            net_delivery: terms.net_delivery,
        }))
    }

    fn travel_plan_for_route(
        &self,
        route: &ContractRoute,
        destination: &ContentId,
        speed: f64,
    ) -> Result<TravelPlan, CoreError> {
        if route.systems.len() < 2
            || route.systems.last() != Some(destination)
            || Self::route_ticks(self.graph(), &route.systems, speed)? != route.ticks
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let first_leg_ticks = Self::route_ticks(self.graph(), &route.systems[..2], speed)?;
        Ok(TravelPlan {
            destination: destination.clone(),
            route: route.systems.clone(),
            next_leg: 1,
            remaining_ticks: first_leg_ticks,
        })
    }

    fn accept_energy_candidate(
        &mut self,
        intent: &EnergyContractIntent,
        candidate: PreparedEnergyCandidate,
    ) -> Result<(), CoreError> {
        let tick = self.tick();
        let mut trader = self
            .world
            .get::<Trader>(candidate.carrier_entity)
            .unwrap()
            .clone();
        self.ensure_carrier_contract_free(&intent.carrier, &trader)?;
        if trader.travel.is_some() || trader.reservation.is_some() {
            return Err(CoreError::ActiveEnergyContract);
        }
        let local_pickup = trader.system == intent.source;
        let (departure_route, departure_destination) = if local_pickup {
            (&candidate.loaded_route, &intent.destination)
        } else {
            (&candidate.deadhead_route, &intent.source)
        };
        let departure_burn = departure_route.burn;
        let required_tank = if local_pickup {
            candidate.loaded_route.burn
        } else {
            candidate
                .deadhead_route
                .burn
                .checked_add(candidate.loaded_route.burn)?
        };
        if trader.energy_tank < required_tank {
            return Err(CoreError::InsufficientEnergy);
        }
        if trader.bulk_energy.headroom(trader.bulk_energy_capacity)? < candidate.gross_payload {
            return Err(CoreError::InsufficientCapacity);
        }
        let travel =
            self.travel_plan_for_route(departure_route, departure_destination, trader.speed)?;
        let departure_market_entity = self.market_entity(&trader.system)?;
        let mut departure_market = self
            .world
            .get::<Market>(departure_market_entity)
            .unwrap()
            .clone();
        if local_pickup {
            if departure_market.energy_stock()? < candidate.gross_payload {
                return Err(CoreError::InsufficientEnergy);
            }
            departure_market.set_energy_stock(
                departure_market
                    .energy_stock()?
                    .checked_sub(candidate.gross_payload)?,
            )?;
            departure_market.energy_flow.market_to_energy_cargo = departure_market
                .energy_flow
                .market_to_energy_cargo
                .checked_add(candidate.gross_payload)?;
        } else if candidate.source_entity == departure_market_entity {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        departure_market.energy_flow.travel_burned = departure_market
            .energy_flow
            .travel_burned
            .checked_add(departure_burn)?;
        trader.energy_tank = trader.energy_tank.checked_sub(departure_burn)?;
        trader.ledger.travel_cost = trader.ledger.travel_cost.checked_add(departure_burn)?;
        trader.travel = Some(travel);

        let mut contracts = self.world.resource::<EnergyContracts>().clone();
        contracts.diagnostics.accepted = contracts
            .diagnostics
            .accepted
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        let contract_id = contracts.allocate_id()?;
        if local_pickup {
            trader.bulk_energy.locked = Some(LockedEnergyLot {
                contract_id,
                amount: candidate.gross_payload,
            });
        }
        let state = if local_pickup {
            EnergyContractState::InTransit { loaded_tick: tick }
        } else {
            EnergyContractState::DeadheadingToSource {
                source_claim: candidate.gross_payload,
                accepted_tick: tick,
            }
        };
        contracts.active.insert(
            contract_id,
            EnergyContract {
                id: contract_id,
                carrier: intent.carrier.clone(),
                source: intent.source.clone(),
                destination: intent.destination.clone(),
                deadhead_route: candidate.deadhead_route,
                loaded_route: candidate.loaded_route,
                recovery_route: candidate.recovery_route,
                gross_payload: candidate.gross_payload,
                carrier_fee_bps: candidate.carrier_fee_bps,
                carrier_profit: candidate.carrier_profit,
                net_delivery: candidate.net_delivery,
                cumulative_settled: Energy::ZERO,
                state,
                latest_blocker: None,
            },
        );

        let mut events = vec![GameEvent::EnergyLogistics(EnergyContractEvent::Accepted {
            contract_id,
        })];
        if local_pickup {
            events.push(GameEvent::EnergyLogistics(EnergyContractEvent::Loaded {
                contract_id,
            }));
        }
        events.push(GameEvent::EnergyLogistics(EnergyContractEvent::Departed {
            contract_id,
        }));
        events.push(GameEvent::Departed {
            trader: intent.carrier.clone(),
            destination: departure_destination.clone(),
            travel_burn: departure_burn,
        });

        *self
            .world
            .get_mut::<Market>(departure_market_entity)
            .unwrap() = departure_market;
        *self
            .world
            .get_mut::<Trader>(candidate.carrier_entity)
            .unwrap() = trader;
        *self.world.resource_mut::<EnergyContracts>() = contracts;
        self.world.resource_mut::<EventBuffer>().0.extend(events);
        Ok(())
    }

    fn record_energy_intent_rejection(
        &mut self,
        rejection: EnergyIntentRejection,
    ) -> Result<(), CoreError> {
        let mut contracts = self.world.resource::<EnergyContracts>().clone();
        match rejection.blocker {
            EnergyContractBlocker::NoReachableSurplus => {
                contracts.diagnostics.no_reachable_surplus = contracts
                    .diagnostics
                    .no_reachable_surplus
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?;
            }
            EnergyContractBlocker::NoViableCandidate => {
                contracts.diagnostics.no_viable_candidate = contracts
                    .diagnostics
                    .no_viable_candidate
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?;
            }
            EnergyContractBlocker::ViableButUnaccepted => {
                contracts.diagnostics.viable_but_unaccepted = contracts
                    .diagnostics
                    .viable_but_unaccepted
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?;
            }
            _ => {}
        }
        *self.world.resource_mut::<EnergyContracts>() = contracts;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::EnergyLogistics(EnergyContractEvent::Rejected {
                blocker: rejection.blocker,
                current_maximum: rejection.current_maximum,
            }));
        Ok(())
    }

    pub(super) fn resolve_pending_energy_contract_intents(&mut self) -> Result<(), CoreError> {
        let intents =
            std::mem::take(&mut self.world.resource_mut::<PendingEnergyContractIntents>().0);
        let mut ordered = Vec::with_capacity(intents.len());
        for intent in intents {
            match self.energy_intent_order_key(&intent) {
                Ok(key) => ordered.push((key, intent)),
                Err(error) => {
                    self.world
                        .resource_mut::<PendingEnergyContractIntents>()
                        .0
                        .push(intent);
                    return Err(error);
                }
            }
        }
        ordered.sort_by(|(left_key, left), (right_key, right)| {
            right_key
                .destination_stage
                .cmp(&left_key.destination_stage)
                .then_with(|| {
                    left_key
                        .destination_runway
                        .cmp(&right_key.destination_runway)
                })
                .then_with(|| right.gross_payload.cmp(&left.gross_payload))
                .then_with(|| left.destination.cmp(&right.destination))
                .then_with(|| left.source.cmp(&right.source))
                .then_with(|| left.carrier.cmp(&right.carrier))
        });

        while !ordered.is_empty() {
            let (_, intent) = ordered.remove(0);
            match self.evaluate_energy_intent(&intent) {
                Ok(Ok(candidate)) => {
                    if let Err(error) = self.accept_energy_candidate(&intent, candidate) {
                        self.world
                            .resource_mut::<PendingEnergyContractIntents>()
                            .0
                            .push(intent);
                        return Err(error);
                    }
                }
                Ok(Err(rejection)) => self.record_energy_intent_rejection(rejection)?,
                Err(error) => {
                    self.world
                        .resource_mut::<PendingEnergyContractIntents>()
                        .0
                        .push(intent);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn claim_capacity_for_source(&mut self, source: &ContentId) -> Result<Energy, CoreError> {
        let entity = self.market_entity(source)?;
        let market = self.world.get::<Market>(entity).unwrap();
        let policy = self.world.get::<MarketPolicy>(entity).unwrap();
        let life = self
            .world
            .resource::<EconomyConfig>()
            .life_support_burn_per_capita;
        exportable_energy(
            market.energy_stock()?,
            market.reserved_energy,
            Energy::ZERO,
            market.operating_reserve(policy, life)?,
            market.protected_liquidation_budget,
            market.energy_logistics.export_reserve,
        )
    }

    fn terminalize_preload_contract(
        &mut self,
        contract_id: ContractId,
        outcome: EnergyContractTerminalOutcome,
    ) -> Result<(), CoreError> {
        let contract = self
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .cloned()
            .ok_or(CoreError::InvalidPhysicalDefinition)?;
        if !matches!(
            contract.state,
            EnergyContractState::DeadheadingToSource { .. }
        ) {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let carrier = self.trader_entity_by_id(&contract.carrier)?;
        if self
            .world
            .get::<Trader>(carrier)
            .unwrap()
            .bulk_energy
            .locked
            .is_some()
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let mut contracts = self.world.resource::<EnergyContracts>().clone();
        match outcome {
            EnergyContractTerminalOutcome::RevokedBeforeLoad => {
                contracts.diagnostics.revoked_before_load = contracts
                    .diagnostics
                    .revoked_before_load
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?;
            }
            EnergyContractTerminalOutcome::RejectedBeforeLoad => {
                contracts.diagnostics.rejected_before_load = contracts
                    .diagnostics
                    .rejected_before_load
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?;
            }
            _ => return Err(CoreError::InvalidPhysicalDefinition),
        }
        contracts.active.remove(&contract_id);
        *self.world.resource_mut::<EnergyContracts>() = contracts;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
                contract_id,
                outcome,
            }));
        Ok(())
    }

    fn load_preload_contract(&mut self, contract_id: ContractId) -> Result<(), CoreError> {
        let tick = self.tick();
        let contract = self
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .cloned()
            .ok_or(CoreError::InvalidPhysicalDefinition)?;
        let source_claim = match contract.state {
            EnergyContractState::DeadheadingToSource { source_claim, .. } => source_claim,
            _ => return Err(CoreError::InvalidPhysicalDefinition),
        };
        if source_claim != contract.gross_payload
            || contract.loaded_route.systems.first() != Some(&contract.source)
            || contract.loaded_route.systems.last() != Some(&contract.destination)
            || contract.recovery_route.systems.first() != Some(&contract.destination)
            || contract.recovery_route.systems.last() != Some(&contract.source)
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let carrier_entity = self.trader_entity_by_id(&contract.carrier)?;
        let mut trader = self.world.get::<Trader>(carrier_entity).unwrap().clone();
        if trader.travel.is_some()
            || trader.system != contract.source
            || trader.bulk_energy.locked.is_some()
            || trader.bulk_energy.headroom(trader.bulk_energy_capacity)? < contract.gross_payload
            || trader.energy_tank < contract.loaded_route.burn
            || route_travel_energy(
                self.graph(),
                &contract.loaded_route.systems,
                trader.travel_burn_per_distance,
            )? != contract.loaded_route.burn
            || route_travel_energy(
                self.graph(),
                &contract.recovery_route.systems,
                trader.travel_burn_per_distance,
            )? != contract.recovery_route.burn
            || Self::route_ticks(self.graph(), &contract.loaded_route.systems, trader.speed)?
                != contract.loaded_route.ticks
            || Self::route_ticks(self.graph(), &contract.recovery_route.systems, trader.speed)?
                != contract.recovery_route.ticks
        {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let active_for_carrier = self
            .world
            .resource::<EnergyContracts>()
            .active
            .values()
            .filter(|active| active.carrier == contract.carrier)
            .count();
        if active_for_carrier != 1 {
            return Err(CoreError::InvalidPhysicalDefinition);
        }
        let travel = self.travel_plan_for_route(
            &contract.loaded_route,
            &contract.destination,
            trader.speed,
        )?;
        let source_entity = self.market_entity(&contract.source)?;
        let mut source = self.world.get::<Market>(source_entity).unwrap().clone();
        if source.energy_stock()? < contract.gross_payload {
            return Err(CoreError::InsufficientEnergy);
        }
        source.set_energy_stock(source.energy_stock()?.checked_sub(contract.gross_payload)?)?;
        source.energy_flow.market_to_energy_cargo = source
            .energy_flow
            .market_to_energy_cargo
            .checked_add(contract.gross_payload)?;
        source.energy_flow.travel_burned = source
            .energy_flow
            .travel_burned
            .checked_add(contract.loaded_route.burn)?;
        trader.energy_tank = trader.energy_tank.checked_sub(contract.loaded_route.burn)?;
        trader.ledger.travel_cost = trader
            .ledger
            .travel_cost
            .checked_add(contract.loaded_route.burn)?;
        trader.bulk_energy.locked = Some(LockedEnergyLot {
            contract_id,
            amount: contract.gross_payload,
        });
        trader.travel = Some(travel);
        let mut contracts = self.world.resource::<EnergyContracts>().clone();
        contracts
            .active
            .get_mut(&contract_id)
            .ok_or(CoreError::InvalidPhysicalDefinition)?
            .state = EnergyContractState::InTransit { loaded_tick: tick };
        let events = vec![
            GameEvent::EnergyLogistics(EnergyContractEvent::Loaded { contract_id }),
            GameEvent::EnergyLogistics(EnergyContractEvent::Departed { contract_id }),
            GameEvent::Departed {
                trader: contract.carrier,
                destination: contract.destination,
                travel_burn: contract.loaded_route.burn,
            },
        ];

        *self.world.get_mut::<Market>(source_entity).unwrap() = source;
        *self.world.get_mut::<Trader>(carrier_entity).unwrap() = trader;
        *self.world.resource_mut::<EnergyContracts>() = contracts;
        self.world.resource_mut::<EventBuffer>().0.extend(events);
        Ok(())
    }

    pub(super) fn maintain_preload_energy_contracts(&mut self) -> Result<(), CoreError> {
        let ordered = self
            .world
            .resource::<EnergyContracts>()
            .active
            .iter()
            .filter_map(|(id, contract)| {
                matches!(
                    contract.state,
                    EnergyContractState::DeadheadingToSource { .. }
                )
                .then_some((*id, contract.source.clone()))
            })
            .collect::<Vec<_>>();
        let mut capacities = BTreeMap::<ContentId, Energy>::new();
        for (_, source) in &ordered {
            if !capacities.contains_key(source) {
                capacities.insert(source.clone(), self.claim_capacity_for_source(source)?);
            }
        }

        for (contract_id, source) in ordered {
            let contract = self
                .world
                .resource::<EnergyContracts>()
                .active
                .get(&contract_id)
                .cloned();
            let Some(contract) = contract else {
                continue;
            };
            let source_claim = match contract.state {
                EnergyContractState::DeadheadingToSource { source_claim, .. } => source_claim,
                _ => continue,
            };
            let remaining = capacities
                .get_mut(&source)
                .ok_or(CoreError::InvalidPhysicalDefinition)?;
            if source_claim > *remaining {
                self.terminalize_preload_contract(
                    contract_id,
                    EnergyContractTerminalOutcome::RevokedBeforeLoad,
                )?;
                continue;
            }
            *remaining = remaining.checked_sub(source_claim)?;

            let reached_source = match self.trader_entity_by_id(&contract.carrier) {
                Ok(carrier) => {
                    let trader = self.world.get::<Trader>(carrier).unwrap();
                    trader.travel.is_none() && trader.system == contract.source
                }
                Err(_) => false,
            };
            if reached_source && self.load_preload_contract(contract_id).is_err() {
                self.terminalize_preload_contract(
                    contract_id,
                    EnergyContractTerminalOutcome::RejectedBeforeLoad,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
