//! Physical Energy logistics domain.
//!
//! Root session orchestration remains authoritative for tick scheduling. Only
//! this module's contract executor may mutate Energy logistics state.

// These pure arithmetic seams are exercised before lifecycle orchestration is
// wired into the root session.
#![cfg_attr(not(test), allow(dead_code))]

use super::{BrownoutStage, ContentId, CoreError, Energy};
use bevy_ecs::prelude::Resource;
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

#[cfg(test)]
mod tests;
