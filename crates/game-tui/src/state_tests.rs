use crate::input::{Action, Direction};
use crate::state::{BatchStatus, Confirmation, ConstructionDraft, Modal, Screen, TuiState};
use game_app::{
    ActionAvailability, ApplicationOutcome, ContentId, DraftDisposition, IntentKind, PreviewStatus,
    ProfileDescriptor,
};
use std::{path::PathBuf, time::Duration};

fn playing_state() -> TuiState {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
    let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter-profile"), 17);
    state.startup_focus = 3;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(
        state.selected_preview().is_some(),
        "starter profile must generate: {:?}",
        state.startup_failure_text()
    );
    state.startup_focus = 4;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.is_playing());
    state
}

#[test]
fn startup_seed_can_be_edited_randomized_and_started_from_preview_panel() {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
    let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter-profile"), 17);

    state.startup_focus = 1;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    state.handle_action(Action::Delete, Duration::ZERO).unwrap();
    state
        .handle_action(Action::Character('4'), Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Character('2'), Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(state.startup_view().unwrap().seed_text, "42");

    state.startup_focus = 2;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_ne!(state.startup_view().unwrap().seed_text, "42");

    state.startup_focus = 3;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(state.startup_focus, 4);
    assert!(state.selected_preview().is_some());
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.is_playing());
}

#[test]
fn generate_shortcut_works_on_startup_and_r_renames_in_play() {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
    let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter-profile"), 17);
    state
        .handle_action(Action::Character('g'), Duration::ZERO)
        .unwrap();
    assert!(state.selected_preview().is_some());

    let mut state = playing_state();
    state
        .handle_action(Action::Character('r'), Duration::ZERO)
        .unwrap();
    assert!(matches!(state.modal, Some(Modal::Editor(_))));
}

#[test]
fn frame_pacing_never_discards_non_batch_modals() {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
    let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter-profile"), 17);

    state.handle_action(Action::Help, Duration::ZERO).unwrap();
    state.advance_due(Duration::from_secs(1)).unwrap();
    assert!(matches!(state.modal, Some(Modal::Help)));

    state.handle_action(Action::Cancel, Duration::ZERO).unwrap();
    state.startup_focus = 1;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    state.advance_due(Duration::from_secs(2)).unwrap();
    assert!(matches!(state.modal, Some(Modal::Editor(_))));
}

#[test]
fn paced_batch_is_deterministic_and_preserves_intermediate_views() {
    let mut state = playing_state();
    state
        .handle_action(Action::AdvanceMany, Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();

    state.advance_due(Duration::ZERO).unwrap();
    let first_tick = state.playing_view().unwrap().time.tick;
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.rate, 5);
    assert_eq!(batch.completed(), 1);

    state.advance_due(Duration::from_millis(199)).unwrap();
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.completed(), 1);

    state.advance_due(Duration::from_millis(200)).unwrap();
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.completed(), 2);
    assert_eq!(batch.history[0].delta.to_tick, first_tick);
    assert_eq!(batch.history[1].delta.to_tick, first_tick + 1);

    state
        .handle_action(Action::Pause, Duration::from_millis(200))
        .unwrap();
    state.advance_due(Duration::from_secs(10)).unwrap();
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.status, BatchStatus::Paused);
    assert_eq!(batch.completed(), 2);

    state
        .handle_action(Action::Confirm, Duration::from_secs(10))
        .unwrap();
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.status, BatchStatus::Paused);
    assert_eq!(batch.completed(), 3);
}

#[test]
fn undersized_resize_stops_batch_and_blocks_gameplay_until_recovery() {
    let mut state = playing_state();
    state
        .handle_action(Action::AdvanceMany, Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    state.advance_due(Duration::ZERO).unwrap();
    let tick = state.playing_view().unwrap().time.tick;

    state.resize(159, 45);
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("batch expected")
    };
    assert_eq!(batch.status, BatchStatus::Stopped);
    assert_eq!(batch.completed(), 1);

    state
        .handle_action(Action::AdvanceOne, Duration::from_secs(1))
        .unwrap();
    assert_eq!(state.playing_view().unwrap().time.tick, tick);
    state.resize(160, 45);
    let Some(Modal::Batch(batch)) = &state.modal else {
        panic!("history must survive recovery")
    };
    assert_eq!(batch.status, BatchStatus::Stopped);
}

#[test]
fn batch_count_validation_retains_the_editor() {
    let mut state = playing_state();
    state
        .handle_action(Action::AdvanceMany, Duration::ZERO)
        .unwrap();
    state.handle_action(Action::Delete, Duration::ZERO).unwrap();
    state
        .handle_action(Action::Character('0'), Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    let Some(Modal::Editor(editor)) = &state.modal else {
        panic!("invalid editor must remain")
    };
    assert_eq!(editor.error.as_deref(), Some("Count must be 1..100"));
}

#[test]
fn startup_edit_marks_preview_stale_and_disables_start() {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
    let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter-profile"), 17);
    state.startup_focus = 3;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(
        state.selected_preview().unwrap().status,
        PreviewStatus::Current
    );

    state.startup_focus = 1;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    state.handle_action(Action::Delete, Duration::ZERO).unwrap();
    state
        .handle_action(Action::Character('9'), Duration::ZERO)
        .unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    let preview = state.selected_preview().unwrap();
    assert_eq!(preview.status, PreviewStatus::Stale);
    assert!(!preview.start_available);
}

#[test]
fn navigation_only_changes_the_selection_visible_on_the_current_screen() {
    let mut state = playing_state();
    let initial_system = state.selected_system;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(state.screen, Screen::Local);

    let positions = {
        let view = state.playing_view().unwrap();
        let system_id = &view.systems[state.selected_system].system_id;
        let local = view
            .local_systems
            .iter()
            .find(|local| &local.system_id == system_id)
            .unwrap();
        local
            .bodies
            .iter()
            .enumerate()
            .flat_map(|(body, value)| (0..value.slots.len()).map(move |slot| (body, slot)))
            .collect::<Vec<_>>()
    };
    assert!(positions.len() > 1);
    state
        .handle_action(Action::Navigate(Direction::Down), Duration::ZERO)
        .unwrap();
    assert_eq!(state.selected_system, initial_system);
    assert_eq!((state.selected_body, state.selected_slot), positions[1]);

    state
        .handle_action(Action::Navigate(Direction::Up), Duration::ZERO)
        .unwrap();
    assert_eq!((state.selected_body, state.selected_slot), positions[0]);
}

#[test]
fn dashboard_opens_read_only_details_for_a_remote_system() {
    let mut state = playing_state();
    let remote = state
        .playing_view()
        .unwrap()
        .systems
        .iter()
        .position(|system| {
            !state
                .playing_view()
                .unwrap()
                .local_systems
                .iter()
                .any(|local| local.system_id == system.system_id)
        })
        .expect("starter world has a known remote system");
    state.selected_system = remote;

    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(state.screen, Screen::SystemDetails);
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert_eq!(state.screen, Screen::SystemDetails);
}

#[test]
fn every_installed_development_can_be_enabled_and_disabled() {
    let mut state = playing_state();
    state.screen = Screen::Local;
    let (body_index, slot_index, was_enabled) = {
        let view = state.playing_view().unwrap();
        let system_id = &view.systems[state.selected_system].system_id;
        let local = view
            .local_systems
            .iter()
            .find(|local| &local.system_id == system_id)
            .unwrap();
        local
            .bodies
            .iter()
            .enumerate()
            .find_map(|(body_index, body)| {
                body.slots
                    .iter()
                    .enumerate()
                    .find_map(|(slot_index, slot)| {
                        slot.development
                            .as_ref()
                            .map(|development| (body_index, slot_index, development.enabled))
                    })
            })
            .expect("generated origin has an installed development")
    };
    state.selected_body = body_index;
    state.selected_slot = slot_index;

    state
        .handle_action(Action::Character('e'), Duration::ZERO)
        .unwrap();
    assert!(matches!(
        state.modal,
        Some(Modal::Confirm(Confirmation::Development { .. }))
    ));
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();

    let view = state.playing_view().unwrap();
    let system_id = &view.systems[state.selected_system].system_id;
    let local = view
        .local_systems
        .iter()
        .find(|local| &local.system_id == system_id)
        .unwrap();
    assert_eq!(
        local.bodies[body_index].slots[slot_index]
            .development
            .as_ref()
            .unwrap()
            .enabled,
        !was_enabled
    );
}

#[test]
fn slot_first_construction_uses_application_options_and_closes_on_acceptance() {
    let mut state = playing_state();
    state.screen = Screen::Local;
    let (body_index, slot_index, option_index) = {
        let view = state.playing_view().unwrap();
        let system_id = &view.systems[state.selected_system].system_id;
        let local = view
            .local_systems
            .iter()
            .find(|local| &local.system_id == system_id)
            .unwrap();
        local
            .bodies
            .iter()
            .enumerate()
            .find_map(|(body_index, body)| {
                body.slots
                    .iter()
                    .enumerate()
                    .find_map(|(slot_index, slot)| {
                        slot.construction_options
                            .iter()
                            .position(|option| {
                                matches!(option.availability, ActionAvailability::Available)
                            })
                            .map(|option_index| (body_index, slot_index, option_index))
                    })
            })
            .expect("generated origin has a constructible empty slot")
    };
    state.selected_body = body_index;
    state.selected_slot = slot_index;
    state
        .handle_action(Action::Character('b'), Duration::ZERO)
        .unwrap();
    state.construction.as_mut().unwrap().selected = option_index;
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.construction.is_none());
    assert!(
        state
            .notice
            .as_ref()
            .is_some_and(|outcome| outcome.accepted)
    );
}

#[test]
fn typed_rejection_disposition_controls_draft_retention() {
    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }
    fn draft() -> ConstructionDraft {
        ConstructionDraft {
            system_id: id("test:system"),
            body_id: id("test:body"),
            slot_id: id("test:slot"),
            options: Vec::new(),
            selected: 0,
        }
    }
    fn rejection(disposition: DraftDisposition) -> ApplicationOutcome {
        ApplicationOutcome {
            accepted: false,
            intent: IntentKind::Construction,
            message: "rejected".into(),
            limiting_reason: None,
            draft_disposition: Some(disposition),
            project_id: None,
            ship_id: None,
        }
    }
    let mut state = TuiState::new(ProfileDescriptor::new("unused.ron", "unused"), 1);
    state.construction = Some(draft());
    state.modal = Some(Modal::Rejection(rejection(DraftDisposition::Retain)));
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.construction.is_some());

    state.modal = Some(Modal::Rejection(rejection(
        DraftDisposition::InvalidateRoot,
    )));
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.construction.is_none());
    assert_eq!(state.screen, Screen::Local);
}

#[test]
fn all_approved_tick_rates_are_selectable() {
    for (action, expected) in [
        (Action::Navigate(crate::input::Direction::Left), 1),
        (Action::Navigate(crate::input::Direction::Right), 10),
    ] {
        let mut state = playing_state();
        state
            .handle_action(Action::AdvanceMany, Duration::ZERO)
            .unwrap();
        state.handle_action(action, Duration::ZERO).unwrap();
        state
            .handle_action(Action::Confirm, Duration::ZERO)
            .unwrap();
        let Some(Modal::Batch(batch)) = &state.modal else {
            panic!("batch expected")
        };
        assert_eq!(batch.rate, expected);
    }
}

#[test]
fn live_session_quit_defaults_to_cancel() {
    let mut state = playing_state();
    state.handle_action(Action::Quit, Duration::ZERO).unwrap();
    assert!(!state.should_quit);
    state.handle_action(Action::Cancel, Duration::ZERO).unwrap();
    assert!(!state.should_quit);
    state.handle_action(Action::Quit, Duration::ZERO).unwrap();
    state
        .handle_action(Action::Confirm, Duration::ZERO)
        .unwrap();
    assert!(state.should_quit);
}
