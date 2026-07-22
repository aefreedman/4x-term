use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProfile(PathBuf);

impl TempProfile {
    fn starter() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "4x-term-game-app-{}-{sequence}.ron",
            std::process::id()
        ));
        fs::write(&path, include_str!("../../../content/profiles/starter.ron"))
            .expect("write temporary profile");
        Self(path)
    }
}

impl Drop for TempProfile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn started_session(seed: u64) -> Session {
    let profile = TempProfile::starter();
    let descriptor = ProfileDescriptor {
        machine_path: profile.0.clone(),
        logical_source_id: "profiles/starter.ron".into(),
        display_name: "starter".into(),
    };
    let mut startup = StartupCoordinator::new(descriptor, seed);
    startup.generate_preview().expect("generate");
    startup.request_start_current_preview().expect("request");
    startup.confirm_start_current_preview().expect("start")
}

#[test]
fn profile_descriptor_derives_a_player_facing_file_stem() {
    let descriptor =
        ProfileDescriptor::new("some/machine/path/custom-profile.ron", "profile:custom");
    assert_eq!(descriptor.display_name, "custom-profile");
    assert_eq!(descriptor.logical_source_id, "profile:custom");
}

#[test]
fn preview_is_allowlisted_stale_after_edit_and_exactly_consumed() {
    let profile = TempProfile::starter();
    let descriptor = ProfileDescriptor::new(&profile.0, "profiles/starter.ron");
    let mut startup = StartupCoordinator::new(descriptor, 41);

    let preview = startup.generate_preview().expect("preview");
    assert_eq!(preview.seed, 41);
    assert_eq!(
        preview.profile_name,
        profile.0.file_stem().unwrap().to_string_lossy()
    );
    assert_eq!(preview.origin_label, "Origin");
    assert!(preview.origin_body_count >= 4);
    assert!(
        preview
            .guaranteed_developments
            .iter()
            .any(|row| row.role == DevelopmentRole::Collector)
    );

    startup.edit_seed("42");
    let stale = startup.view().preview.expect("old preview retained");
    assert_eq!(stale.status, PreviewStatus::Stale);
    assert!(!stale.start_available);
    assert!(startup.request_start_current_preview().is_err());

    let current = startup.generate_preview().expect("regenerated");
    assert_eq!(current.seed, 42);
    let preview_visuals = current
        .frontier_fog
        .iter()
        .map(|point| (point.visual_key, point.coordinate))
        .collect::<std::collections::BTreeMap<_, _>>();
    startup.request_start_current_preview().expect("request");
    let session = startup
        .confirm_start_current_preview()
        .expect("exact start");
    let playing = session.playing_view().expect("playing view");
    assert_eq!(playing.seed, 42);
    assert!(playing.probe_maximum_jump_limit > 0);
    assert!(
        playing.systems.len() > 1,
        "identified summaries are player knowledge"
    );
    assert!(playing.uncharted_indication_count > 0);
    assert!(
        playing
            .frontier_fog
            .iter()
            .all(|point| { preview_visuals.get(&point.visual_key) == Some(&point.coordinate) })
    );
    assert!(playing.systems.iter().all(|system| {
        preview_visuals.get(&system.visual_key) == Some(&system.visual_coordinate)
    }));
    assert_eq!(
        playing.local_systems.len(),
        1,
        "neutral runtime remains hidden"
    );
    assert_eq!(playing.local_systems[0].system_id, id("core:origin"));
}

#[test]
fn map_visual_pivots_stay_within_four_units_of_actual_positions() {
    let actual = Position3::from_quanta(17, -9, 4);
    for seed in [0, 1, u64::MAX] {
        for visual_key in 0..128 {
            let visual = map_visual_coordinate(actual, 1, seed, visual_key);
            let dx = visual.x - actual.x.0;
            let dy = visual.y - actual.y.0;
            assert!(dx * dx + dy * dy <= 16, "offset ({dx}, {dy})");
        }
    }
}

#[test]
fn content_read_failures_are_logical_source_aware() {
    let missing = std::env::temp_dir().join("does-not-exist-4x-term-profile.ron");
    let mut startup = StartupCoordinator::new(
        ProfileDescriptor {
            machine_path: missing.clone(),
            logical_source_id: "logical/starter.ron".into(),
            display_name: "starter".into(),
        },
        1,
    );
    let StartupFailure::Content(diagnostics) = startup.generate_preview().unwrap_err() else {
        panic!("expected content diagnostics");
    };
    assert_eq!(diagnostics[0].logical_source_id, "logical/starter.ron");
    assert!(
        !diagnostics[0]
            .message
            .contains(&missing.display().to_string())
    );
}

#[test]
fn playing_view_has_tick_zero_energy_and_no_machine_path_or_mutable_world() {
    let session = started_session(7);
    let view = session.playing_view().expect("view");
    let origin = &view.local_systems[0];
    assert_eq!(origin.energy.current, 10);
    assert_eq!(origin.energy.capacity, 10);
    assert_eq!(origin.energy.last_completed_tick, None);
    assert_eq!(view.seasonal_position, 1);
    assert_eq!(origin.population_count, 0);
    assert!(origin.bodies.iter().any(|body| !body.slots.is_empty()));
    let rendered = format!("{view:?}");
    assert!(!rendered.contains(".ron"));
    assert_eq!(
        view.chart.len(),
        1,
        "unpositioned summaries are not plotted"
    );
}

#[test]
fn aliases_are_session_owned_charted_trimmed_and_display_cell_validated() {
    let mut session = started_session(8);
    let origin = id("core:origin");
    let applied = session
        .dispatch(SessionIntent::SetSystemAlias {
            system_id: origin.clone(),
            alias: Some("  Haven  ".into()),
        })
        .expect("dispatch");
    let SessionOutcome::Applied { view, .. } = applied else {
        panic!("alias should apply");
    };
    let entry = view
        .systems
        .iter()
        .find(|entry| entry.system_id == origin)
        .unwrap();
    assert_eq!(entry.alias.as_deref(), Some("Haven"));
    assert_eq!(entry.display_label, "Haven");
    assert_eq!(entry.catalogue_label, "Origin");

    let too_wide = "界".repeat(17);
    let rejected = session
        .dispatch(SessionIntent::SetSystemAlias {
            system_id: origin.clone(),
            alias: Some(too_wide),
        })
        .expect("dispatch");
    assert!(matches!(
        rejected,
        SessionOutcome::Rejected(ApplicationOutcome {
            limiting_reason: Some(LimitingReason::AliasTooWide {
                cells: 34,
                maximum: 32
            }),
            draft_disposition: Some(DraftDisposition::Retain),
            ..
        })
    ));
    assert_eq!(
        session.playing_view().unwrap().systems[0].alias.as_deref(),
        Some("Haven")
    );

    session
        .dispatch(SessionIntent::SetSystemAlias {
            system_id: origin,
            alias: None,
        })
        .unwrap();
    assert_eq!(
        session.playing_view().unwrap().systems[0].display_label,
        "Origin"
    );
}

#[test]
fn construction_rejections_classify_draft_recovery_and_do_not_advance_time() {
    let mut session = started_session(9);
    let view = session.playing_view().unwrap();
    let origin = &view.local_systems[0];
    let empty = origin
        .bodies
        .iter()
        .flat_map(|body| {
            body.slots
                .iter()
                .map(move |slot| (body.body_id.clone(), slot))
        })
        .find(|(_, slot)| slot.development.is_none() && !slot.reserved)
        .expect("generated origin has an empty slot");
    let before = view.time;

    let retain = session
        .dispatch(SessionIntent::EnqueueConstruction {
            system_id: id("core:origin"),
            body_id: empty.0.clone(),
            slot_id: empty.1.slot_id.clone(),
            role: DevelopmentRole::Battery,
            extractor_resource_id: None,
        })
        .unwrap();
    assert!(matches!(
        retain,
        SessionOutcome::Rejected(ApplicationOutcome {
            limiting_reason: Some(LimitingReason::InsufficientResource { .. }),
            draft_disposition: Some(DraftDisposition::Retain),
            ..
        })
    ));
    assert_eq!(session.playing_view().unwrap().time, before);

    let invalidate = session
        .dispatch(SessionIntent::EnqueueConstruction {
            system_id: id("core:origin"),
            body_id: id("core:no_body"),
            slot_id: id("core:no_slot"),
            role: DevelopmentRole::Refinery,
            extractor_resource_id: None,
        })
        .unwrap();
    assert!(matches!(
        invalidate,
        SessionOutcome::Rejected(ApplicationOutcome {
            draft_disposition: Some(DraftDisposition::InvalidateRoot),
            ..
        })
    ));
}

#[test]
fn valid_construction_and_one_tick_return_immutable_intermediate_views() {
    let mut session = started_session(10);
    let view = session.playing_view().unwrap();
    let origin = &view.local_systems[0];
    let (body_id, slot_id) = origin
        .bodies
        .iter()
        .flat_map(|body| {
            body.slots.iter().map(move |slot| {
                (
                    body.body_id.clone(),
                    slot.slot_id.clone(),
                    slot.development.is_none() && !slot.reserved,
                )
            })
        })
        .find(|(_, _, empty)| *empty)
        .map(|(body, slot, _)| (body, slot))
        .unwrap();

    let queued = session
        .dispatch(SessionIntent::EnqueueConstruction {
            system_id: id("core:origin"),
            body_id,
            slot_id,
            role: DevelopmentRole::Refinery,
            extractor_resource_id: None,
        })
        .unwrap();
    let SessionOutcome::Applied {
        view: queued_view,
        outcome,
    } = queued
    else {
        panic!("Refinery should use available Energy and Ore");
    };
    assert!(outcome.accepted);
    assert_eq!(queued_view.local_systems[0].construction_queue.len(), 1);

    let step = session.dispatch(SessionIntent::AdvanceOneTick).unwrap();
    let SessionOutcome::Tick(step) = step else {
        panic!("tick accepted")
    };
    assert_eq!(step.delta.from_tick, 0);
    assert_eq!(step.delta.to_tick, 1);
    assert_eq!(step.view.time.tick, 1);
    assert!(
        step.view.local_systems[0]
            .energy
            .last_completed_tick
            .is_some()
    );
}

#[test]
fn probe_and_expedition_assessments_are_typed_and_non_mutating() {
    let mut session = started_session(11);
    let before = session.playing_view().unwrap();
    let source = id("core:origin");
    let target = id("generated:system_000000");
    let missing_probe = ShipId::new(source.clone(), 999);

    let probe = session
        .dispatch(SessionIntent::AssessProbeLaunch {
            source_id: source.clone(),
            ship_id: missing_probe,
            target_id: target.clone(),
            jump_limit: 1,
        })
        .unwrap();
    let SessionOutcome::ProbeAssessment(probe) = probe else {
        panic!("assessment")
    };
    assert!(matches!(
        probe.availability,
        ActionAvailability::Unavailable { .. }
    ));

    let expedition = session
        .dispatch(SessionIntent::AssessExpeditionLaunch {
            source_id: source.clone(),
            ship_id: ShipId::new(source, 1000),
            target_id: target,
            reservations: None,
        })
        .unwrap();
    let SessionOutcome::ExpeditionAssessment(expedition) = expedition else {
        panic!("assessment")
    };
    assert!(matches!(
        expedition.availability,
        ActionAvailability::Unavailable { .. }
    ));
    assert_eq!(session.playing_view().unwrap().time, before.time);
    assert_eq!(
        session.playing_view().unwrap().local_systems[0].stocks,
        before.local_systems[0].stocks
    );
}

fn id(value: &str) -> ContentId {
    ContentId::new(value).unwrap()
}
