use game_core::{CarrierFeeSchedule, Energy, EnergyLogisticsPolicy};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct EnergyLogisticsSource {
    pub carrier_fee_bps: Vec<u32>,
    pub max_allocation_bps: u32,
    pub curtailment_projection_window: u32,
    pub export_reserve: i64,
    pub authored_export_base: i64,
    pub settlement_timeout_ticks: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct EnergyLogisticsOverrideSource {
    pub carrier_fee_bps: Option<Vec<u32>>,
    pub max_allocation_bps: Option<u32>,
    pub export_reserve: Option<i64>,
    pub authored_export_base: Option<i64>,
}

pub(super) fn compile_global(
    source: &EnergyLogisticsSource,
    errors: &mut Vec<String>,
) -> EnergyLogisticsPolicy {
    compile_effective(
        None,
        Some(&source.carrier_fee_bps),
        Some(source.max_allocation_bps),
        source.curtailment_projection_window,
        Some(source.export_reserve),
        Some(source.authored_export_base),
        source.settlement_timeout_ticks,
        "economy_config.ron:energy_logistics",
        errors,
    )
}

pub(super) fn merge_market(
    defaults: EnergyLogisticsPolicy,
    source: EnergyLogisticsOverrideSource,
    context: &str,
    errors: &mut Vec<String>,
) -> EnergyLogisticsPolicy {
    compile_effective(
        Some(defaults),
        source.carrier_fee_bps.as_ref(),
        source.max_allocation_bps,
        defaults.curtailment_projection_window,
        source.export_reserve,
        source.authored_export_base,
        defaults.settlement_timeout_ticks,
        &format!("{context}:energy_logistics"),
        errors,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_effective(
    defaults: Option<EnergyLogisticsPolicy>,
    fee_source: Option<&Vec<u32>>,
    max_allocation_bps: Option<u32>,
    curtailment_projection_window: u32,
    export_reserve: Option<i64>,
    authored_export_base: Option<i64>,
    settlement_timeout_ticks: u32,
    context: &str,
    errors: &mut Vec<String>,
) -> EnergyLogisticsPolicy {
    let fallback = defaults.unwrap_or_default();
    let fees = fee_source.map_or(fallback.carrier_fee_bps, |values| {
        if values.len() != 4 {
            errors.push(format!(
                "{context}:carrier_fee_bps: exactly four stage fees are required"
            ));
            return fallback.carrier_fee_bps;
        }
        CarrierFeeSchedule {
            normal: values[0],
            throttled: values[1],
            emergency: values[2],
            starvation: values[3],
        }
    });
    let max_allocation_bps = max_allocation_bps.unwrap_or(fallback.max_allocation_bps);
    let export_reserve = export_reserve.unwrap_or(fallback.export_reserve.0);
    let authored_export_base = authored_export_base.unwrap_or(fallback.authored_export_base.0);
    if export_reserve < 0 {
        errors.push(format!("{context}:export_reserve: cannot be negative"));
    }
    if authored_export_base < 0 {
        errors.push(format!(
            "{context}:authored_export_base: cannot be negative"
        ));
    }
    let policy = EnergyLogisticsPolicy {
        carrier_fee_bps: fees,
        max_allocation_bps,
        curtailment_projection_window,
        export_reserve: Energy(export_reserve),
        authored_export_base: Energy(authored_export_base),
        settlement_timeout_ticks,
    };
    if !(fees.normal < fees.throttled
        && fees.throttled < fees.emergency
        && fees.emergency < fees.starvation)
    {
        errors.push(format!(
            "{context}:carrier_fee_bps: fees must be strictly increasing"
        ));
    }
    if max_allocation_bps == 0 || max_allocation_bps > 10_000 {
        errors.push(format!(
            "{context}:max_allocation_bps: must be in 1..=10000"
        ));
    }
    if fees.starvation >= max_allocation_bps {
        errors.push(format!(
            "{context}:carrier_fee_bps: every fee must be below max_allocation_bps"
        ));
    }
    if curtailment_projection_window == 0 {
        errors.push(format!(
            "{context}:curtailment_projection_window: must be nonzero"
        ));
    }
    if settlement_timeout_ticks == 0 {
        errors.push(format!(
            "{context}:settlement_timeout_ticks: must be nonzero"
        ));
    }
    policy
}
