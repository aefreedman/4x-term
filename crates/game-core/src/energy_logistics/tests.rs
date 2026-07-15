use super::*;

fn e(value: i64) -> Energy {
    Energy(value)
}

fn worked_constants() -> SettlementConstants {
    SettlementConstants {
        gross_payload: e(4_000),
        loaded_route_burn: e(20),
        carrier_profit: e(40),
        net_delivery: e(3_940),
        recovery_burn: e(20),
    }
}

fn authored_sizing(deadhead_burn: i64, net_cap: i64) -> GrossSizingInput {
    GrossSizingInput {
        offered_payload: e(3_440),
        bulk_headroom: e(4_000),
        candidate_net_cap: e(net_cap),
        loaded_route_burn: e(14),
        recovery_burn: e(14),
        carrier_fee_bps: 50,
        max_allocation_bps: 1_000,
        deadhead_burn: e(deadhead_burn),
        tank_energy: e(1_000),
        tank_capacity: e(1_500),
    }
}

#[test]
fn el_inv_lot_bulk_usage_and_headroom_are_checked() {
    let contract_id = ContractId(7);
    let hold = BulkEnergyHold {
        owned: e(40),
        locked: Some(LockedEnergyLot {
            contract_id,
            amount: e(60),
        }),
    };
    assert_eq!(hold.used(), Ok(e(100)));
    assert_eq!(hold.headroom(e(125)), Ok(e(25)));
    assert_eq!(
        hold.headroom(e(99)),
        Err(CoreError::InvalidPhysicalDefinition)
    );
    assert_eq!(
        BulkEnergyHold {
            owned: e(-1),
            locked: None,
        }
        .used(),
        Err(CoreError::InvalidPhysicalDefinition)
    );
}

#[test]
fn el_d15_policy_validation_and_stage_lookup_are_exact() {
    let valid = EnergyLogisticsPolicy {
        carrier_fee_bps: CarrierFeeSchedule {
            normal: 50,
            throttled: 100,
            emergency: 200,
            starvation: 300,
        },
        max_allocation_bps: 1_000,
        curtailment_projection_window: 20,
        export_reserve: e(0),
        authored_export_base: e(3_200),
        settlement_timeout_ticks: 20,
    };
    assert_eq!(valid.validate(), Ok(()));
    assert_eq!(valid.carrier_fee_bps.for_stage(BrownoutStage::Normal), 50);
    assert_eq!(
        valid.carrier_fee_bps.for_stage(BrownoutStage::Starvation),
        300
    );

    let mut invalid = valid;
    invalid.carrier_fee_bps.emergency = 100;
    assert_eq!(invalid.validate(), Err(CoreError::InvalidPolicy));
    let mut invalid = valid;
    invalid.max_allocation_bps = 300;
    assert_eq!(invalid.validate(), Err(CoreError::InvalidPolicy));
    let mut invalid = valid;
    invalid.curtailment_projection_window = 0;
    assert_eq!(invalid.validate(), Err(CoreError::InvalidPolicy));
    let mut invalid = valid;
    invalid.settlement_timeout_ticks = 0;
    assert_eq!(invalid.validate(), Err(CoreError::InvalidPolicy));
    let mut invalid = valid;
    invalid.export_reserve = e(-1);
    assert_eq!(invalid.validate(), Err(CoreError::InvalidPolicy));
}

#[test]
fn el_inv_claim_contract_ids_are_monotonic_and_atomic_on_overflow() {
    let mut contracts = EnergyContracts::default();
    assert_eq!(contracts.allocate_id().unwrap().get(), 1);
    assert_eq!(contracts.allocate_id().unwrap().get(), 2);
    contracts.next_id = u64::MAX;
    assert_eq!(contracts.allocate_id(), Err(CoreError::Overflow));
    assert_eq!(contracts.next_id, u64::MAX);
}

#[test]
fn el_d1_projection_and_protection_vectors_are_exact() {
    assert_eq!(
        exportable_energy(e(5_000), e(0), e(0), e(54), e(55), e(0)),
        Ok(e(4_891))
    );
    assert_eq!(
        exportable_energy(e(100), e(80), e(50), e(30), e(20), e(10)),
        Ok(Energy::ZERO)
    );

    let ticks = vec![
        ProjectionTick {
            generated: e(30),
            life_support: e(3),
            operating_burn: e(15),
        };
        20
    ];
    assert_eq!(
        project_energy(e(4_982), e(5_000), &ticks),
        Ok(ProjectionResult {
            final_stock: e(4_982),
            curtailed: e(240),
        })
    );
}

#[test]
fn el_d2_fee_floor_and_freight_ceil_vectors_are_exact() {
    assert_eq!(
        fee_terms(e(4_000), e(20), 100),
        Ok(FeeTerms {
            carrier_profit: e(40),
            carrier_allocation: e(60),
            net_delivery: e(3_940),
            effective_freight_bps: 150,
        })
    );
    assert_eq!(fee_terms(e(199), e(0), 50).unwrap().carrier_profit, e(0));
    assert_eq!(fee_terms(e(200), e(0), 50).unwrap().carrier_profit, e(1));
}

#[test]
fn el_d3_largest_gross_obeys_net_and_profit_boundaries() {
    assert_eq!(
        largest_viable_gross(authored_sizing(0, 3_000)),
        Ok(Some(e(3_029)))
    );
    assert_eq!(
        largest_viable_gross(authored_sizing(0, 2_999)),
        Ok(Some(e(3_028)))
    );
    assert_eq!(largest_viable_gross(authored_sizing(23, 3_000)), Ok(None));

    let mut recovery_blocked = authored_sizing(0, 14);
    recovery_blocked.offered_payload = e(100);
    assert_eq!(largest_viable_gross(recovery_blocked), Ok(None));

    let mut recovery_capacity_blocked = authored_sizing(0, 3_000);
    recovery_capacity_blocked.tank_capacity = e(13);
    assert_eq!(largest_viable_gross(recovery_capacity_blocked), Ok(None));
}

#[test]
fn el_d4_utility_is_positive_only_after_deadhead_cost() {
    assert_eq!(opportunity_score(e(15), e(0), 0, 2), Ok(Some(7_500_000)));
    assert_eq!(opportunity_score(e(15), e(23), 3, 2), Ok(None));
    assert_eq!(opportunity_score(e(1), e(0), 0, 0), Ok(Some(1_000_000)));
}

#[test]
fn el_d7_multi_retry_derives_allocation_once() {
    let constants = worked_constants();
    let first = settlement_delta(
        constants,
        SettlementState {
            cumulative_settled: e(0),
            locked_amount: e(4_000),
        },
        e(2_000),
    )
    .unwrap();
    assert_eq!(
        first,
        SettlementDelta {
            settled_now: e(2_000),
            reimbursement_conversion: e(20),
            fee_conversion: e(20),
            cumulative_after: e(2_000),
            locked_after: e(1_960),
            completed: false,
        }
    );

    let second = settlement_delta(
        constants,
        SettlementState {
            cumulative_settled: first.cumulative_after,
            locked_amount: first.locked_after,
        },
        e(1_000),
    )
    .unwrap();
    assert_eq!(second.settled_now, e(1_000));
    assert_eq!(second.reimbursement_conversion, e(0));
    assert_eq!(second.fee_conversion, e(10));
    assert_eq!(second.cumulative_after, e(3_000));
    assert_eq!(second.locked_after, e(950));

    let third = settlement_delta(
        constants,
        SettlementState {
            cumulative_settled: second.cumulative_after,
            locked_amount: second.locked_after,
        },
        e(940),
    )
    .unwrap();
    assert_eq!(third.settled_now, e(940));
    assert_eq!(third.reimbursement_conversion, e(0));
    assert_eq!(third.fee_conversion, e(10));
    assert_eq!(third.locked_after, e(0));
    assert!(third.completed);
}

#[test]
fn el_d7_partial_settlement_preserves_exact_recovery_reserve() {
    let constants = worked_constants();
    let delta = settlement_delta(
        constants,
        SettlementState {
            cumulative_settled: e(0),
            locked_amount: e(4_000),
        },
        e(3_939),
    )
    .unwrap();
    assert_eq!(delta.settled_now, e(3_921));
    assert_eq!(delta.locked_after, e(20));
    assert!(!delta.completed);

    let zero = settlement_delta(
        constants,
        SettlementState {
            cumulative_settled: e(0),
            locked_amount: e(4_000),
        },
        e(0),
    )
    .unwrap();
    assert_eq!(zero.settled_now, e(0));
    assert_eq!(zero.reimbursement_conversion, e(0));
    assert_eq!(zero.fee_conversion, e(0));
    assert_eq!(zero.locked_after, e(4_000));
}

#[test]
fn el_d8_timeout_vectors_convert_only_reimbursement_and_recovery() {
    assert_eq!(
        timeout_plan(
            worked_constants(),
            SettlementState {
                cumulative_settled: e(0),
                locked_amount: e(4_000),
            }
        ),
        Ok(TimeoutPlan {
            reimbursement_conversion: e(20),
            recovery_conversion: e(20),
            recovery_burn: e(20),
            locked_after_departure: e(3_960),
        })
    );
    assert_eq!(
        timeout_plan(
            worked_constants(),
            SettlementState {
                cumulative_settled: e(2_000),
                locked_amount: e(1_960),
            }
        ),
        Ok(TimeoutPlan {
            reimbursement_conversion: e(0),
            recovery_conversion: e(20),
            recovery_burn: e(20),
            locked_after_departure: e(1_940),
        })
    );
}
