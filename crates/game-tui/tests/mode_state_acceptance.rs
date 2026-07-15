use crossterm::event::KeyCode;
use game_core::{ContentId, Energy, PopulationTrend};
use game_tui::{
    Activity, InputAction, InputLayer, LayoutClass, SortDirection, SystemOrderItem, SystemSortKey,
    UiState, classify_layout, order_systems, route_key,
};

fn id(value: &str) -> ContentId {
    ContentId::new(value).unwrap()
}

fn system(
    value: &str,
    name: &str,
    risk: u8,
    runway: u32,
    fill: u32,
    population: u64,
    route_ticks: Option<u32>,
) -> SystemOrderItem {
    SystemOrderItem {
        id: id(value),
        name: name.into(),
        risk,
        runway_ticks: runway,
        energy_fill_percent: fill,
        population,
        population_trend: PopulationTrend::Stable,
        route_ticks,
        energy_stock: Energy(fill as i64),
    }
}

#[test]
fn layout_classifier_uses_both_cell_dimensions_at_exact_edges() {
    assert_eq!(classify_layout(79, 30), LayoutClass::Unsupported);
    assert_eq!(classify_layout(80, 29), LayoutClass::Unsupported);
    assert_eq!(classify_layout(80, 30), LayoutClass::Compact);
    assert_eq!(classify_layout(159, 44), LayoutClass::Compact);
    assert_eq!(classify_layout(160, 44), LayoutClass::Compact);
    assert_eq!(classify_layout(159, 45), LayoutClass::Compact);
    assert_eq!(classify_layout(160, 45), LayoutClass::Regular);
    assert_eq!(classify_layout(200, 60), LayoutClass::Regular);
}

#[test]
fn every_system_sort_is_deterministic_in_both_directions() {
    let rows = vec![
        system("core:b", "Beta", 2, 20, 60, 200, Some(9)),
        system("core:a", "Alpha", 3, 10, 80, 100, Some(5)),
        system("core:c", "Alpha", 1, 30, 40, 300, None),
    ];
    let expected = [
        (
            SystemSortKey::Name,
            ["core:a", "core:c", "core:b"],
            ["core:b", "core:a", "core:c"],
        ),
        (
            SystemSortKey::Risk,
            ["core:c", "core:b", "core:a"],
            ["core:a", "core:b", "core:c"],
        ),
        (
            SystemSortKey::Runway,
            ["core:a", "core:b", "core:c"],
            ["core:c", "core:b", "core:a"],
        ),
        (
            SystemSortKey::EnergyFillPercent,
            ["core:c", "core:b", "core:a"],
            ["core:a", "core:b", "core:c"],
        ),
        (
            SystemSortKey::Population,
            ["core:a", "core:b", "core:c"],
            ["core:c", "core:b", "core:a"],
        ),
        (
            SystemSortKey::RouteTicks,
            ["core:a", "core:b", "core:c"],
            ["core:b", "core:a", "core:c"],
        ),
    ];
    assert_eq!(SystemSortKey::ALL.len(), expected.len());
    for (key, ascending_ids, descending_ids) in expected {
        let ascending = order_systems(&rows, key, SortDirection::Ascending);
        let descending = order_systems(&rows, key, SortDirection::Descending);
        assert_eq!(
            ascending
                .iter()
                .map(|row| row.id.to_string())
                .collect::<Vec<_>>(),
            ascending_ids
        );
        assert_eq!(
            descending
                .iter()
                .map(|row| row.id.to_string())
                .collect::<Vec<_>>(),
            descending_ids
        );
        assert_eq!(
            order_systems(&rows, key, SortDirection::Ascending),
            ascending
        );
    }
}

#[test]
fn stable_system_selection_survives_reorder_and_reconciles_missing_rows() {
    let mut ui = UiState {
        selected_system: Some(id("core:b")),
        ..UiState::default()
    };
    let reordered = vec![
        system("core:c", "C", 0, 0, 0, 0, None),
        system("core:b", "B", 0, 0, 0, 0, None),
        system("core:a", "A", 0, 0, 0, 0, None),
    ];
    ui.reconcile_system_selection(&reordered);
    assert_eq!(ui.selected_system, Some(id("core:b")));
    assert_eq!(ui.system_row(&reordered), Some(1));

    ui.reconcile_system_selection(&reordered[..1]);
    assert_eq!(ui.selected_system, Some(id("core:c")));
    ui.reconcile_system_selection(&[]);
    assert_eq!(ui.selected_system, None);
    assert_eq!(ui.system_row(&[]), None);
}

#[test]
fn input_precedence_is_unsupported_then_overlay_then_global_then_activity() {
    let mut ui = UiState {
        activity: Activity::Trade,
        ..UiState::default()
    };

    assert_eq!(route_key(KeyCode::F(1), &ui, false), InputAction::None);
    assert_eq!(route_key(KeyCode::Char('q'), &ui, false), InputAction::Quit);

    ui.input_layer = InputLayer::Quantity;
    assert_eq!(route_key(KeyCode::F(1), &ui, true), InputAction::None);
    assert_eq!(route_key(KeyCode::Esc, &ui, true), InputAction::CloseLayer);

    ui.input_layer = InputLayer::Root;
    assert_eq!(
        route_key(KeyCode::F(1), &ui, true),
        InputAction::Switch(Activity::Systems)
    );
    assert_eq!(route_key(KeyCode::Char('b'), &ui, true), InputAction::Buy);
    assert_eq!(route_key(KeyCode::Char('s'), &ui, true), InputAction::Sell);
    assert_eq!(route_key(KeyCode::Char('.'), &ui, true), InputAction::Step);
    assert_eq!(
        route_key(KeyCode::F(5), &ui, true),
        InputAction::Switch(Activity::Encyclopedia)
    );
    ui.activity = Activity::Encyclopedia;
    assert_eq!(route_key(KeyCode::PageUp, &ui, true), InputAction::PageUp);
    assert_eq!(
        route_key(KeyCode::PageDown, &ui, true),
        InputAction::PageDown
    );
    ui.activity = Activity::Systems;
    assert_eq!(route_key(KeyCode::Char('b'), &ui, true), InputAction::None);
}

#[test]
fn activity_switching_has_no_focus_cycle_or_tab_requirement() {
    let ui = UiState::default();
    assert_eq!(
        route_key(KeyCode::F(1), &ui, true),
        InputAction::Switch(Activity::Systems)
    );
    assert_eq!(
        route_key(KeyCode::F(2), &ui, true),
        InputAction::Switch(Activity::Trade)
    );
    assert_eq!(
        route_key(KeyCode::F(3), &ui, true),
        InputAction::Switch(Activity::Governance)
    );
    assert_eq!(
        route_key(KeyCode::F(4), &ui, true),
        InputAction::Switch(Activity::Intelligence)
    );
    assert_eq!(
        route_key(KeyCode::F(5), &ui, true),
        InputAction::Switch(Activity::Encyclopedia)
    );
    assert_eq!(route_key(KeyCode::Tab, &ui, true), InputAction::None);
}
