mod components;

use crate::state::{
    BatchStatus, Confirmation, EditorKind, MIN_HEIGHT, MIN_WIDTH, MissionDraft, Modal, Screen,
    TuiState,
};
use components::{panel, selected_line, truncate_cells, unavailable};
use game_app::{
    ActionAvailability, AssetKindView, DevelopmentCondition, DevelopmentRole, KnownFactView,
    MissionView, PreviewStatus, RouteView, SlotCoordinate,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame<'_>, state: &TuiState) {
    let area = frame.area();
    if state.undersized || area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_safety(frame, area, state);
        return;
    }
    let shell = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    if state.is_playing() {
        render_title(frame, shell[0], state);
        match state.screen {
            Screen::Dashboard => render_dashboard(frame, shell[1], state),
            Screen::SystemDetails => render_system_details(frame, shell[1], state),
            Screen::Local => render_local(frame, shell[1], state),
            Screen::Operations => render_operations(frame, shell[1], state),
        }
        frame.render_widget(
            Paragraph::new("[. Advance Tick] [t Advance Many] [? Help] [F2/s Settings] [q Quit]"),
            shell[2],
        );
    } else {
        frame.render_widget(Paragraph::new("4X-TERM / NEW FRONTIER"), shell[0]);
        render_startup(frame, shell[1], state);
        frame.render_widget(
            Paragraph::new("[? Help] [F2/s Settings] [q Quit]"),
            shell[2],
        );
    }
    if let Some(modal) = &state.modal {
        render_modal(frame, area, state, modal);
    }
}

fn render_safety(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let text = vec![
        Line::styled(
            "! TERMINAL TOO SMALL",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw(format!("Required: {MIN_WIDTH} x {MIN_HEIGHT}")),
        Line::raw(format!("Current:  {} x {}", area.width, area.height)),
        Line::raw(""),
        Line::raw("Resize to recover. Gameplay commands are blocked."),
        Line::raw(if state.is_playing() {
            "[q Quit - confirmation required]"
        } else {
            "[q Quit]"
        }),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(panel("SAFETY", true)),
        area,
    );
    if matches!(state.modal, Some(Modal::Confirm(Confirmation::Quit))) {
        render_confirmation_box(
            frame,
            area,
            "QUIT UNSAVED SESSION?",
            "This session cannot be resumed.\n\n[Enter Quit] [Esc Cancel]",
        );
    }
}

fn render_title(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let Some(view) = state.playing_view() else {
        return;
    };
    let label = view
        .systems
        .get(state.selected_system)
        .map_or("--", |row| row.display_label.as_str());
    frame.render_widget(
        Paragraph::new(format!(
            "4X-TERM / {}    Tick {}   Season {}/10",
            truncate_cells(label, 48),
            view.time.tick,
            view.seasonal_position
        )),
        area,
    );
}

fn render_startup(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(60), Constraint::Min(1)])
        .split(area);
    let Some(view) = state.startup_view() else {
        return;
    };
    let mut fields = vec![
        selected_line(
            state.startup_focus == 0,
            format!("Profile  {}", view.profile.display_name),
        ),
        Line::raw(format!("   {}", view.profile.machine_path.display())),
        Line::raw(""),
        selected_line(
            state.startup_focus == 1,
            format!("Seed     {}", view.seed_text),
        ),
    ];
    if let Some(error) = &view.seed_error {
        fields.push(Line::raw(format!("! {error}")));
    }
    fields.extend([
        selected_line(state.startup_focus == 2, "[New random seed]"),
        Line::raw(""),
        selected_line(
            state.startup_focus == 3,
            if view.generate_available {
                "[Generate preview]"
            } else {
                "[UNAVAILABLE] Generate preview"
            },
        ),
        Line::raw(""),
        Line::raw("Choose an explicit profile and unsigned 64-bit seed."),
    ]);
    if let Some(error) = state.startup_failure_text() {
        fields.push(Line::raw(format!("! {error}")));
    }
    fields.push(Line::raw(""));
    fields.push(Line::raw(
        "[Enter Edit/Activate] [Tab/Arrows Next] [n New Seed] [g Generate]",
    ));
    frame.render_widget(
        Paragraph::new(fields)
            .block(panel("NEW WORLD", state.startup_focus < 4))
            .wrap(Wrap { trim: false }),
        columns[0],
    );

    let preview = if let Some(preview) = &view.preview {
        let stale = preview.status == PreviewStatus::Stale;
        let mut lines = vec![
            Line::raw(if stale {
                "! STALE PREVIEW - generate again"
            } else {
                "PRE-PLAY GENERATION SUMMARY"
            }),
            Line::raw(""),
            Line::raw(format!("Seed       {}", preview.seed)),
            Line::raw(format!("Profile    {}", preview.profile_name)),
            Line::raw(""),
            Line::raw("ORIGIN"),
            Line::raw(format!(
                "{} / {}",
                preview.origin_label, preview.origin_community_label
            )),
            Line::raw(format!("Bodies     {}", preview.origin_body_count)),
            Line::raw(""),
            Line::raw("Infrastructure"),
        ];
        lines.extend(
            preview
                .guaranteed_developments
                .iter()
                .map(|row| Line::raw(format!("  {:?}  {:?}", row.role, row.condition))),
        );
        lines.push(Line::raw(""));
        lines.push(Line::raw("Stocks"));
        lines.extend(
            preview
                .initial_origin_stocks
                .iter()
                .map(|row| Line::raw(format!("  {:<24} {:>20}", row.label, row.quantity))),
        );
        lines.push(Line::raw(""));
        lines.push(if preview.start_available {
            selected_line(state.startup_focus == 4, "[Enter Start]")
        } else {
            unavailable(
                preview
                    .start_unavailable_reason
                    .as_deref()
                    .unwrap_or("Generate a current preview"),
            )
        });
        lines
    } else {
        vec![
            Line::raw("[EMPTY] No generated preview."),
            Line::raw("Generate to inspect the origin scaffold."),
        ]
    };
    if let Some(generated) = &view.preview {
        let preview_columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(columns[1]);
        frame.render_widget(
            Paragraph::new(preview)
                .block(panel("WORLD PREVIEW", state.startup_focus == 4))
                .wrap(Wrap { trim: false }),
            preview_columns[0],
        );
        let map_width = usize::from(preview_columns[1].width.saturating_sub(2));
        let map_height = usize::from(preview_columns[1].height.saturating_sub(2));
        frame.render_widget(
            Paragraph::new(preview_fog_map(
                &generated.frontier_fog,
                generated.seed,
                map_width,
                map_height,
            ))
            .block(panel("FRONTIER PREVIEW", false)),
            preview_columns[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(preview)
                .block(panel("WORLD PREVIEW", state.startup_focus == 4))
                .wrap(Wrap { trim: false }),
            columns[1],
        );
    }
}

fn preview_fog_map(
    fog: &[game_app::MapTexturePoint],
    seed: u64,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let mut grid = vec![vec![(" ", Color::Reset); width]; height];
    let center_x = i64::try_from(width / 2).unwrap_or(0);
    let center_y = i64::try_from(height / 2).unwrap_or(0);
    for point in fog {
        place_system_visual(
            &mut grid,
            center_x + point.coordinate.x,
            center_y - point.coordinate.y,
            assignment_hash(seed, point.visual_key),
            false,
        );
    }
    if width > 0 && height > 0 {
        grid[height / 2][width / 2] = ("@", Color::Cyan);
    }
    grid.into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(glyph, color)| Span::styled(glyph, Style::default().fg(color)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SystemVisual {
    Plain,
    Irregular,
    Interference,
    Directional,
    Compact,
}

fn visual_assignment(hash: u64) -> (SystemVisual, u8) {
    let visual = match hash % 5 {
        0 => SystemVisual::Plain,
        1 => SystemVisual::Irregular,
        2 => SystemVisual::Interference,
        3 => SystemVisual::Directional,
        _ => SystemVisual::Compact,
    };
    (visual, ((hash / 5) % 12) as u8)
}

fn assignment_hash(seed: u64, visual_key: u64) -> u64 {
    stable_hash(&format!("{seed}:{visual_key}"))
}

fn transformed(dx: isize, dy: isize, variant: u8) -> (isize, isize) {
    let family = variant / 4;
    let (mut x, y) = match family {
        0 => (dx, dy),
        1 => (-dx + if dy > 0 { 1 } else { 0 }, dy),
        _ => (dx + dy.signum(), dy),
    };
    let mut y = y;
    for _ in 0..variant % 4 {
        (x, y) = (-y, x);
    }
    (x, y)
}

fn selected_glyph(glyph: &'static str, selected: bool) -> &'static str {
    if !selected {
        return glyph;
    }
    match glyph {
        "░" => "▒",
        "▒" => "▓",
        "·" => "•",
        value => value,
    }
}

fn place_system_visual(
    grid: &mut [Vec<(&'static str, Color)>],
    center_x: i64,
    center_y: i64,
    hash: u64,
    selected: bool,
) {
    const A: &[(isize, isize, &str)] = &[
        (1, -3, "░"),
        (2, -3, "░"),
        (-2, -2, "░"),
        (-1, -2, "░"),
        (0, -2, "░"),
        (1, -2, "░"),
        (2, -2, "░"),
        (-3, -1, "░"),
        (-2, -1, "░"),
        (-1, -1, "▒"),
        (0, -1, "▒"),
        (1, -1, "░"),
        (2, -1, "░"),
        (3, -1, "░"),
        (-4, 0, "░"),
        (-3, 0, "▒"),
        (-2, 0, "▒"),
        (-1, 0, "▒"),
        (0, 0, "▒"),
        (1, 0, "▒"),
        (2, 0, "░"),
        (-2, 1, "░"),
        (-1, 1, "▒"),
        (0, 1, "▒"),
        (1, 1, "░"),
        (2, 1, "░"),
        (0, 2, "░"),
        (1, 2, "░"),
    ];
    const C: &[(isize, isize, &str)] = &[
        (-3, -3, "░"),
        (-2, -3, "░"),
        (-1, -3, "░"),
        (5, -3, "░"),
        (-4, -2, "░"),
        (-3, -2, "░"),
        (-2, -2, "▒"),
        (3, -2, "░"),
        (4, -2, "░"),
        (5, -2, "░"),
        (-2, -1, "░"),
        (2, -1, "▒"),
        (3, -1, "▒"),
        (4, -1, "░"),
        (-5, 0, "░"),
        (-4, 0, "░"),
        (-3, 0, "░"),
        (1, 0, "▒"),
        (2, 0, "▒"),
        (3, 0, "▒"),
        (6, 0, "░"),
        (-3, 1, "░"),
        (-2, 1, "░"),
        (-1, 1, "▒"),
        (0, 1, "▒"),
        (4, 1, "░"),
        (5, 1, "░"),
        (1, 2, "░"),
        (5, 2, "░"),
        (6, 2, "░"),
    ];
    const D: &[(isize, isize, &str)] = &[
        (-6, -3, "·"),
        (-4, -2, "·"),
        (-3, -2, "·"),
        (-1, -1, "·"),
        (1, -1, "░"),
        (2, 0, "░"),
        (3, 0, "▒"),
        (4, 0, "░"),
        (4, 1, "░"),
        (5, 1, "▒"),
        (6, 1, "▒"),
        (7, 1, "░"),
        (7, 2, "░"),
        (8, 2, "░"),
        (9, 2, "░"),
        (11, 3, "·"),
    ];
    const E: &[(isize, isize, &str)] = &[
        (-1, -2, "░"),
        (0, -2, "▒"),
        (1, -2, "░"),
        (-3, -1, "░"),
        (-2, -1, "▒"),
        (-1, -1, "▒"),
        (0, -1, "▒"),
        (1, -1, "░"),
        (-4, 0, "░"),
        (-3, 0, "▒"),
        (-2, 0, "▒"),
        (-1, 0, "▒"),
        (0, 0, "▒"),
        (1, 0, "▒"),
        (2, 0, "░"),
        (-3, 1, "░"),
        (-2, 1, "▒"),
        (-1, 1, "▒"),
        (0, 1, "▒"),
        (1, 1, "░"),
        (-1, 2, "░"),
        (0, 2, "▒"),
        (1, 2, "░"),
    ];
    let (visual, variant) = visual_assignment(hash);
    let cells = match visual {
        SystemVisual::Plain => &[(0, 0, "*")][..],
        SystemVisual::Irregular => A,
        SystemVisual::Interference => C,
        SystemVisual::Directional => D,
        SystemVisual::Compact => E,
    };
    let offset_x = i64::try_from((hash >> 16) % 9).unwrap_or(0) - 4;
    let offset_y = i64::try_from((hash >> 24) % 7).unwrap_or(0) - 3;
    let color = if selected {
        Color::White
    } else {
        Color::DarkGray
    };
    let height = i64::try_from(grid.len()).unwrap_or(0);
    let width = i64::try_from(grid.first().map_or(0, Vec::len)).unwrap_or(0);
    for &(dx, dy, glyph) in cells {
        let (dx, dy) = transformed(dx, dy, variant);
        let x = center_x + offset_x + dx as i64;
        let y = center_y + offset_y + dy as i64;
        if x >= 0 && x < width && y >= 0 && y < height {
            grid[y as usize][x as usize] = (selected_glyph(glyph, selected), color);
        }
    }
}

fn selection_window(len: usize, selected: usize, capacity: usize) -> (usize, usize) {
    if len <= capacity {
        return (0, len);
    }
    let selected = selected.min(len - 1);
    let start = selected.saturating_sub(capacity / 2).min(len - capacity);
    (start, start + capacity)
}

fn dashboard_rects(area: Rect) -> [Rect; 3] {
    let extra = area.width.saturating_sub(160);
    let map_width = 100 + (extra * 2 / 3);
    let systems_width = 30 + (extra / 3);
    [
        Rect::new(area.x, area.y, map_width, area.height),
        Rect::new(area.x + map_width, area.y, systems_width, area.height),
        Rect::new(
            area.x + map_width + systems_width,
            area.y,
            area.width.saturating_sub(map_width + systems_width),
            area.height,
        ),
    ]
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let Some(view) = state.playing_view() else {
        return;
    };
    let rects = dashboard_rects(area);
    let selected_id = view
        .systems
        .get(state.selected_system)
        .map(|row| &row.system_id);
    let inner_width = rects[0].width.saturating_sub(2) as usize;
    let inner_height = rects[0].height.saturating_sub(2) as usize;
    let mut grid = vec![vec![(" ", Color::Reset); inner_width]; inner_height];
    let map_center_x = i64::try_from(inner_width / 2).unwrap_or(0);
    let map_center_y = i64::try_from(inner_height / 2).unwrap_or(0);
    for point in &view.frontier_fog {
        place_system_visual(
            &mut grid,
            map_center_x + point.coordinate.x,
            map_center_y - point.coordinate.y,
            assignment_hash(view.seed, point.visual_key),
            false,
        );
    }
    for system in view
        .systems
        .iter()
        .filter(|system| system.chart_position.is_none())
    {
        let hash = assignment_hash(view.seed, system.visual_key);
        let x = u64::try_from(inner_width)
            .ok()
            .filter(|width| *width != 0)
            .and_then(|width| i64::try_from(hash % width).ok())
            .unwrap_or(0);
        let y = u64::try_from(inner_height)
            .ok()
            .filter(|height| *height != 0)
            .and_then(|height| i64::try_from((hash >> 32) % height).ok())
            .unwrap_or(0);
        place_system_visual(
            &mut grid,
            x,
            y,
            hash,
            selected_id == Some(&system.system_id),
        );
    }
    for entry in &view.chart {
        if inner_width == 0 || inner_height == 0 {
            break;
        }
        let center_x = i64::try_from(inner_width / 2).unwrap_or(0);
        let center_y = i64::try_from(inner_height / 2).unwrap_or(0);
        let x = (center_x + entry.coordinate.x)
            .clamp(0, i64::try_from(inner_width - 1).unwrap_or(0)) as usize;
        let y = (center_y - entry.coordinate.y)
            .clamp(0, i64::try_from(inner_height - 1).unwrap_or(0)) as usize;
        let selected = selected_id == Some(&entry.system_id);
        grid[y][x] = (
            if selected { "@" } else { "*" },
            if selected { Color::Cyan } else { Color::Gray },
        );
    }
    let map = grid
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(glyph, color)| Span::styled(glyph, Style::default().fg(color)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(map).block(panel("FRONTIER", false)),
        rects[0],
    );

    let system_capacity = usize::from(rects[1].height.saturating_sub(9)).max(1);
    let (system_start, system_end) =
        selection_window(view.systems.len(), state.selected_system, system_capacity);
    let mut systems = Vec::new();
    if system_start > 0 {
        systems.push(ListItem::new(Line::raw(format!("^ more:{}", system_start))));
    }
    systems.extend(
        view.systems
            .iter()
            .enumerate()
            .skip(system_start)
            .take(system_end.saturating_sub(system_start))
            .map(|(index, row)| {
                let marker = if row.chart_position.is_some() {
                    if index == state.selected_system {
                        "@"
                    } else {
                        "*"
                    }
                } else {
                    " "
                };
                let position = row.chart_position.map_or_else(
                    || "--".into(),
                    |coordinate| format!("{},{}", coordinate.x, coordinate.y),
                );
                ListItem::new(selected_line(
                    index == state.selected_system,
                    format!(
                        "{marker} {}  {position}",
                        truncate_cells(
                            &row.display_label,
                            rects[1].width.saturating_sub(12) as usize
                        )
                    ),
                ))
            }),
    );
    if system_end < view.systems.len() {
        systems.push(ListItem::new(Line::raw(format!(
            "v more:{}",
            view.systems.len() - system_end
        ))));
    }
    systems.push(ListItem::new(Line::raw(format!(
        "  Uncharted: {}",
        view.uncharted_indication_count
    ))));
    systems.push(ListItem::new(Line::raw(format!(
        "  {} / {}",
        state.selected_system + 1,
        view.systems.len()
    ))));
    systems.push(ListItem::new(Line::raw("")));
    let rename_available = view
        .systems
        .get(state.selected_system)
        .is_some_and(|system| system.chart_position.is_some());
    let actions = if rename_available {
        "[Up/Down Select] [Enter Details] [r Rename]"
    } else {
        "[Up/Down Select] [Enter Details]"
    };
    systems.push(ListItem::new(Line::raw(actions)));
    frame.render_widget(List::new(systems).block(panel("SYSTEMS", true)), rects[1]);

    let detail_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(rects[2]);
    if let Some(detail) = view
        .details
        .iter()
        .find(|detail| Some(&detail.system_id) == selected_id)
    {
        let mut lines = vec![Line::raw(detail.display_label.clone())];
        if detail.alias.is_some() {
            lines.push(Line::raw(format!("Catalogue  {}", detail.catalogue_label)));
        }
        lines.push(Line::raw(format!("Knowledge  {:?}", detail.knowledge)));
        if let Some(position) = detail.chart_position {
            lines.push(Line::raw(format!(
                "Position   {}, {}, {}",
                position.x, position.y, position.z
            )));
        }
        for fact in &detail.facts {
            lines.push(Line::raw(match fact {
                KnownFactView::BodyCount(count) => format!("Bodies     {count}"),
                KnownFactView::StellarStrengthHundredths(strength) => {
                    format!("Stellar strength  {}.{:02}", strength / 100, strength % 100)
                }
                KnownFactView::ResourceRichness {
                    resource_label,
                    richness,
                    ..
                } => format!(
                    "{resource_label} richness  {}",
                    content_label(&format!("{richness:?}"))
                ),
                KnownFactView::Inhabited(inhabited) => {
                    format!("Inhabited  {}", if *inhabited { "yes" } else { "no" })
                }
            }));
        }
        if let Some(local) = view
            .local_systems
            .iter()
            .find(|local| local.system_id == detail.system_id)
        {
            lines.push(Line::raw(format!("Population {}", local.population_count)));
            lines.push(Line::raw("Stocks"));
            lines.extend(
                local
                    .stocks
                    .iter()
                    .map(|row| Line::raw(format!("  {:<12} {:>10}", row.label, row.quantity))),
            );
        }
        lines.push(Line::raw(""));
        if !view
            .local_systems
            .iter()
            .any(|local| local.system_id == detail.system_id)
        {
            lines.push(unavailable("No local command access"));
        }
        if let Some(notice) = &state.notice {
            lines.push(Line::raw(""));
            lines.push(Line::raw(format!(
                "{} {}",
                if notice.accepted { ">" } else { "!" },
                notice.message
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).block(panel(&detail.display_label, false)),
            detail_split[0],
        );
    }
    if let Some(local) = selected_id.and_then(|id| {
        view.local_systems
            .iter()
            .find(|local| &local.system_id == id)
    }) {
        let evidence = local.energy.last_completed_tick.as_ref();
        let mut lines = vec![
            Line::raw(format!(
                "Current  {} / {}",
                local.energy.current, local.energy.capacity
            )),
            Line::raw(format!("Headroom {}", local.energy.headroom)),
            Line::raw(format!("Season   {}/10", local.energy.seasonal_position)),
            Line::raw(""),
            Line::raw("Last tick"),
        ];
        if let Some(value) = evidence {
            lines.extend([
                Line::raw(format!("Life support      {}", value.required_life_support)),
                Line::raw(format!(
                    "Paid / unpaid    {} / {}",
                    value.paid_life_support, value.unpaid_life_support
                )),
                Line::raw(format!(
                    "Supported / short {} / {}",
                    value.supported_population, value.underserved_population
                )),
                Line::raw(format!("Overflow          {}", value.retention_overflow)),
            ]);
        } else {
            lines.push(Line::raw("-- no completed tick --"));
        }
        frame.render_widget(
            Paragraph::new(lines).block(panel("ENERGY", false)),
            detail_split[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("[UNAVAILABLE] No commandable local Energy state")
                .block(panel("ENERGY", false)),
            detail_split[1],
        );
    }
}

fn render_system_details(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let Some(view) = state.playing_view() else {
        return;
    };
    let Some(system) = view.systems.get(state.selected_system) else {
        return;
    };
    let Some(detail) = view
        .details
        .iter()
        .find(|detail| detail.system_id == system.system_id)
    else {
        return;
    };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let mut knowledge = vec![
        Line::raw(detail.display_label.clone()),
        Line::raw(format!("Knowledge  {:?}", detail.knowledge)),
    ];
    if let Some(alias) = &detail.alias {
        knowledge.push(Line::raw(format!("Catalogue  {}", detail.catalogue_label)));
        knowledge.push(Line::raw(format!("Alias      {alias}")));
    }
    if let Some(position) = detail.chart_position {
        knowledge.push(Line::raw(format!(
            "Chart position  {}, {}, {}",
            position.x, position.y, position.z
        )));
    } else {
        knowledge.push(Line::raw("Chart position  uncertain"));
    }
    knowledge.extend([
        Line::raw(""),
        Line::raw(format!(
            "Observed  {}",
            system
                .last_observed_tick
                .map_or_else(|| "--".into(), |tick| format!("tick {tick}"))
        )),
        Line::raw(format!(
            "Received  {}",
            system
                .last_received_tick
                .map_or_else(|| "--".into(), |tick| format!("tick {tick}"))
        )),
        Line::raw(""),
    ]);
    let local = view
        .local_systems
        .iter()
        .any(|local| local.system_id == detail.system_id);
    knowledge.push(if local {
        Line::raw("[Enter Manage] [r Rename] [Esc Back]")
    } else {
        unavailable("No local command access — inspection only")
    });
    if !local {
        knowledge.push(Line::raw("[r Rename] [Esc Back]"));
    }
    frame.render_widget(
        Paragraph::new(knowledge).block(panel("SYSTEM KNOWLEDGE", true)),
        columns[0],
    );

    let mut facts = vec![Line::raw("Received observations"), Line::raw("")];
    if detail.facts.is_empty() {
        facts.push(Line::raw("[EMPTY] No detailed observations received"));
    }
    for fact in &detail.facts {
        let (label, value) = match fact {
            KnownFactView::BodyCount(count) => ("Bodies".into(), count.to_string()),
            KnownFactView::StellarStrengthHundredths(strength) => (
                "Stellar strength".into(),
                format!("{}.{:02}", strength / 100, strength % 100),
            ),
            KnownFactView::ResourceRichness {
                resource_label,
                richness,
                ..
            } => (
                format!("{resource_label} richness"),
                content_label(&format!("{richness:?}")),
            ),
            KnownFactView::Inhabited(inhabited) => (
                "Inhabited".into(),
                if *inhabited { "Yes" } else { "No" }.into(),
            ),
        };
        facts.extend([
            Line::raw(label),
            Line::raw(format!("  {value}")),
            Line::raw(""),
        ]);
    }
    frame.render_widget(
        Paragraph::new(facts).block(panel("PROBE / SURVEY INFORMATION", false)),
        columns[1],
    );
}

fn render_local(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);
    let Some(local) = state.playing_view().and_then(|view| {
        view.systems.get(state.selected_system).and_then(|system| {
            view.local_systems
                .iter()
                .find(|local| local.system_id == system.system_id)
        })
    }) else {
        frame.render_widget(
            Paragraph::new("[UNAVAILABLE] This system is not commandable.\n[Esc Back]")
                .block(panel("LOCAL SYSTEM", true)),
            area,
        );
        return;
    };
    let mut body_lines = Vec::new();
    let mut selected_row = 0;
    for (body_index, body) in local.bodies.iter().enumerate() {
        let resources = body
            .resources
            .iter()
            .filter(|resource| resource.quantity > 0)
            .map(|resource| format!("{} {}", resource.label, resource.quantity))
            .collect::<Vec<_>>()
            .join(", ");
        body_lines.push(Line::raw(if resources.is_empty() {
            body.label.clone()
        } else {
            format!("{}  [{}]", body.label, resources)
        }));
        for (slot_index, slot) in body.slots.iter().enumerate() {
            let selected = body_index == state.selected_body && slot_index == state.selected_slot;
            if selected {
                selected_row = body_lines.len();
            }
            let queued = local
                .construction_queue
                .iter()
                .find(|project| project.body_id == body.body_id && project.slot_id == slot.slot_id);
            let value = if let Some(development) = &slot.development {
                format!(
                    "{:?} [{}]",
                    development.role,
                    if development.enabled { "ON" } else { "OFF" }
                )
            } else if let Some(project) = queued {
                format!(
                    "BUILDING {:?} [{}/{}]",
                    project.role, project.work_applied, project.required_work
                )
            } else {
                "[EMPTY]".into()
            };
            let marker_style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let value_style = if slot.development.is_none() && queued.is_none() {
                Style::default().fg(Color::DarkGray)
            } else {
                marker_style
            };
            body_lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{}   {}  ",
                        if selected { ">" } else { " " },
                        slot.slot_label
                    ),
                    marker_style,
                ),
                Span::styled(value, value_style),
            ]));
        }
    }
    let body_line_count = body_lines.len();
    let body_capacity = usize::from(columns[0].height.saturating_sub(6)).max(1);
    let (body_start, body_end) = selection_window(body_line_count, selected_row, body_capacity);
    let mut visible_body_lines = Vec::new();
    if body_start > 0 {
        visible_body_lines.push(Line::raw(format!("^ more:{body_start}")));
    }
    visible_body_lines.extend(
        body_lines
            .into_iter()
            .skip(body_start)
            .take(body_end.saturating_sub(body_start)),
    );
    if body_end < body_line_count {
        visible_body_lines.push(Line::raw(format!("v more:{}", body_line_count - body_end)));
    }
    let mut body_lines = visible_body_lines;
    body_lines.push(Line::raw(""));
    let selected_slot = local
        .bodies
        .get(state.selected_body)
        .and_then(|body| body.slots.get(state.selected_slot));
    let can_build = selected_slot
        .is_some_and(|slot| slot.development.is_none() && !slot.construction_options.is_empty());
    let can_toggle_development = selected_slot
        .and_then(|slot| slot.development.as_ref())
        .is_some_and(|development| matches!(development.toggle, ActionAvailability::Available));
    let can_toggle_habitat = selected_slot
        .and_then(|slot| slot.habitat.as_ref())
        .is_some_and(|habitat| matches!(habitat.toggle, ActionAvailability::Available));
    let mut actions = vec!["[Up/Down Select Slot]"];
    if can_build {
        actions.push("[b Build]");
    }
    if can_toggle_development {
        let enabled = selected_slot
            .and_then(|slot| slot.development.as_ref())
            .is_some_and(|development| development.enabled);
        actions.push(if enabled { "[e Disable]" } else { "[e Enable]" });
    }
    if can_toggle_habitat {
        actions.push("[g Habitat Generation]");
    }
    actions.push("[Esc Back]");
    body_lines.push(Line::raw(actions.join(" ")));
    frame.render_widget(
        Paragraph::new(body_lines).block(panel("BODIES / SLOTS", true)),
        columns[0],
    );
    let body = local.bodies.get(state.selected_body);
    let slot = body.and_then(|body| body.slots.get(state.selected_slot));
    let mut details = vec![
        Line::raw(format!("Population {}", local.population_count)),
        Line::raw(""),
        Line::raw("System stocks"),
    ];
    if local.stocks.is_empty() {
        details.push(Line::raw("  [EMPTY] No stored resources"));
    } else {
        details.extend(
            local
                .stocks
                .iter()
                .map(|row| Line::raw(format!("  {:<18} {:>20}", row.label, row.quantity))),
        );
    }
    if let Some(body) = body {
        details.push(Line::raw(""));
        details.push(Line::raw(format!("Body {}", body.label)));
        details.push(Line::raw("Body resources"));
        details.extend(
            body.resources
                .iter()
                .map(|row| Line::raw(format!("  {:<18} {:>20}", row.label, row.quantity))),
        );
    }
    if let Some(development) = slot.and_then(|slot| slot.development.as_ref()) {
        details.push(Line::raw(format!(
            "Development {:?}  {}",
            development.role,
            if development.enabled {
                "ENABLED"
            } else {
                "DISABLED"
            }
        )));
    }
    if let Some(slot) = slot
        && let Some(habitat) = &slot.habitat
    {
        details.extend([
            Line::raw(""),
            Line::raw("Habitat"),
            Line::raw(format!(
                "Functional {}  Occupied {}",
                habitat.functional, habitat.occupied
            )),
            Line::raw(format!(
                "Generation {}  Progress {}/{}",
                if habitat.generation_enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                habitat.generation_progress,
                habitat.required_energy
            )),
        ]);
        if let ActionAvailability::Unavailable { message, .. } = &habitat.toggle {
            details.push(unavailable(message));
        }
    }
    if let Some(notice) = &state.notice {
        details.push(Line::raw(""));
        details.push(Line::raw(format!(
            "{} {}",
            if notice.accepted { ">" } else { "!" },
            notice.message
        )));
    }
    frame.render_widget(
        Paragraph::new(details).block(panel("DETAIL", false)),
        columns[1],
    );
    let mut operations = vec![Line::raw("Construction queue")];
    if local.construction_queue.is_empty() {
        operations.push(Line::raw("[EMPTY] No construction queued"));
    }
    operations.extend(local.construction_queue.iter().map(|row| {
        Line::raw(format!(
            "{:?} [{}/{}]",
            row.role, row.work_applied, row.required_work
        ))
    }));
    let system_has_shipyard = local.bodies.iter().any(|body| {
        body.slots.iter().any(|slot| {
            slot.development
                .as_ref()
                .is_some_and(|development| development.role == DevelopmentRole::Shipyard)
        })
    });
    let selected_shipyard_enabled =
        slot.and_then(|slot| slot.development.as_ref())
            .is_some_and(|development| {
                development.role == DevelopmentRole::Shipyard
                    && development.condition == DevelopmentCondition::Functional
                    && development.enabled
            });
    let ready_probe = local.has_operational_shipyard
        && local
            .completed_assets
            .iter()
            .any(|asset| asset.ready && matches!(asset.kind, AssetKindView::Probe));
    let ready_expedition = local.has_operational_shipyard
        && local
            .completed_assets
            .iter()
            .any(|asset| asset.ready && matches!(asset.kind, AssetKindView::Expedition { .. }));
    if system_has_shipyard {
        operations.push(Line::raw(""));
        operations.push(Line::raw("Shipyard"));
        if let Some(slot) = slot {
            operations.extend(slot.shipyard_queue.iter().map(|row| {
                Line::raw(format!(
                    "{:?} [{}/{}] {}",
                    row.kind,
                    row.progress,
                    row.required_progress,
                    if row.cancellable { "cancellable" } else { "" }
                ))
            }));
        }
        if slot.is_some_and(|slot| slot.shipyard_queue.iter().any(|row| row.cancellable)) {
            operations.push(Line::raw("[c Cancel first cancellable in selected slot]"));
        }
        if !selected_shipyard_enabled {
            operations.push(Line::raw("Select an enabled Shipyard slot to queue ships"));
        }
    }
    if !local.completed_assets.is_empty() {
        operations.push(Line::raw(""));
        operations.push(Line::raw("Assets"));
        operations.extend(local.completed_assets.iter().map(|asset| {
            Line::raw(format!(
                "{} {}",
                asset_kind_label(&asset.kind),
                if asset.ready { "READY" } else { "pending" }
            ))
        }));
    }
    if selected_shipyard_enabled || ready_probe {
        operations.push(Line::raw("[p Probe queue/launch]"));
    }
    if selected_shipyard_enabled || ready_expedition {
        operations.push(Line::raw("[x Expedition queue/launch]"));
    }
    frame.render_widget(
        Paragraph::new(operations).block(panel("QUEUES / ASSETS", false)),
        columns[2],
    );
}

fn render_operations(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let Some(view) = state.playing_view() else {
        return;
    };
    let mut lines = vec![Line::raw("Active redacted routes")];
    if view.active_routes.is_empty() {
        lines.push(Line::raw("[EMPTY] No active routes"));
    }
    for route in &view.active_routes {
        lines.extend(route_lines(route));
    }
    lines.push(Line::raw(""));
    lines.push(Line::raw("Missions / reports"));
    for mission in &view.missions {
        lines.push(Line::raw(match mission {
            MissionView::AwaitingOutcome { target_label, .. } => {
                format!("Awaiting outcome: {target_label}")
            }
            MissionView::Founded { target_label, .. } => format!("FOUNDED: {target_label}"),
            MissionView::FoundingLost {
                target_label,
                reason,
                ..
            } => format!(
                "! LOST: {target_label} ({})",
                content_label(&format!("{reason:?}"))
            ),
        }));
    }
    for _report in &view.probe_reports {
        lines.push(Line::raw("Probe report awaiting transmission"));
    }
    frame.render_widget(
        Paragraph::new(lines).block(panel("SCOUTING / EXPEDITIONS", true)),
        area,
    );
}

fn asset_kind_label(kind: &AssetKindView) -> String {
    match kind {
        AssetKindView::Probe => "Probe".into(),
        AssetKindView::Expedition { founding_stocks } => {
            let cargo = founding_stocks
                .iter()
                .map(|resource| format!("{} {}", resource.quantity, resource.label))
                .collect::<Vec<_>>()
                .join(", ");
            if cargo.is_empty() {
                "Expedition".into()
            } else {
                format!("Expedition — founding cargo: {cargo}")
            }
        }
    }
}

fn availability_text(availability: &ActionAvailability) -> String {
    match availability {
        ActionAvailability::Available => "Ready".into(),
        ActionAvailability::Unavailable { message, .. } => format!("Unavailable — {message}"),
    }
}

fn coordinate_label(coordinate: &SlotCoordinate) -> String {
    format!(
        "{} / {}",
        content_label(coordinate.body.as_str()),
        content_label(coordinate.slot.as_str())
    )
}

fn content_label(value: &str) -> String {
    value
        .rsplit(':')
        .next()
        .unwrap_or(value)
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + characters.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn route_lines(route: &RouteView) -> Vec<Line<'static>> {
    let mut result = vec![Line::raw(format!(
        "Route distance {}",
        route.total_distance
    ))];
    result.extend(route.stops.iter().map(|stop| {
        Line::raw(format!(
            "  {} {}",
            if stop.reached { "reached" } else { "en route" },
            stop.label.as_deref().unwrap_or("-- hidden stop --")
        ))
    }));
    result
}

fn render_modal(frame: &mut Frame<'_>, area: Rect, state: &TuiState, modal: &Modal) {
    match modal {
        Modal::Help => {
            let (context, commands) = if !state.is_playing() {
                (
                    "Startup: Arrows/Tab select a visible field; Enter activates it.",
                    "n new seed   g generate",
                )
            } else {
                match state.screen {
                    Screen::Dashboard => (
                        "Dashboard: Up/Down selects a system; Enter manages it when local.",
                        "r alias   o operations",
                    ),
                    Screen::SystemDetails => (
                        "System details: inspect received knowledge; Enter manages when local; Esc returns.",
                        "r alias   o operations",
                    ),
                    Screen::Local => (
                        "Local system: Up/Down selects a visible slot; Esc returns to dashboard.",
                        "b construct   e enable/disable development   g Habitat generation\nc cancel first cancellable in selected slot   o operations\nShip actions appear in Queues / Assets only when available.",
                    ),
                    Screen::Operations => (
                        "Operations: Esc returns to dashboard.",
                        "No panel-specific commands.",
                    ),
                }
            };
            let text = format!(
                "CONTEXTUAL HELP\n\n{context}\n{} directional keys\n\n. one tick   t paced ticks\n{commands}\n\n[? or Esc Close]",
                state.layout.directional_hint()
            );
            render_confirmation_box(frame, area, "HELP", &text);
        }
        Modal::Settings => render_confirmation_box(
            frame,
            area,
            "GLOBAL SETTINGS",
            &format!(
                "Keyboard mode\n> {}\n\n[Arrows/Enter Change] [Esc Close]",
                state.layout.name()
            ),
        ),
        Modal::Editor(editor) => {
            let title = match editor.kind {
                EditorKind::Profile => "PROFILE PATH",
                EditorKind::Seed => "SEED",
                EditorKind::Alias => "SYSTEM ALIAS",
                EditorKind::Batch => "ADVANCE TICKS",
            };
            let mut text = if editor.kind == EditorKind::Alias {
                let catalogue = state
                    .playing_view()
                    .and_then(|view| view.systems.get(state.selected_system))
                    .map_or("--", |system| system.catalogue_label.as_str());
                format!("Catalogue  {catalogue}\n> {}", editor.value)
            } else {
                format!("> {}", editor.value)
            };
            if editor.kind == EditorKind::Alias {
                text.push_str("\n\nEmpty + Enter clears the alias.");
            }
            if editor.kind == EditorKind::Batch {
                text.push_str(&format!(
                    "\n\nRate {} ticks/sec [Left/Right: 1/5/10]",
                    editor.rate
                ));
            }
            if let Some(error) = &editor.error {
                text.push_str(&format!("\n\n! {error}"));
            }
            text.push_str("\n\n[Enter Apply] [Esc Back]");
            render_confirmation_box(frame, area, title, &text);
        }
        Modal::Confirm(value) => {
            let (title, text) = match value {
                Confirmation::Quit => ("QUIT UNSAVED SESSION?", "This session cannot be resumed.\n\n[Enter Quit] [Esc Cancel]".into()),
                Confirmation::Development { label, enabled, .. } => (
                    "DEVELOPMENT CONTROL",
                    format!(
                        "Set {label} {}? Existing progress and queues are preserved.\n\n[Enter Confirm] [Esc Cancel]",
                        if *enabled { "enabled" } else { "disabled" }
                    ),
                ),
                Confirmation::Habitat { enabled, progress, .. } => ("HABITAT CONTROL", format!("Set generation {}? Existing progress {} is preserved.\n\n[Enter Confirm] [Esc Cancel]", if *enabled { "enabled" } else { "disabled" }, progress)),
                Confirmation::Construction => {
                    let text = state.construction.as_ref().and_then(|draft| draft.options.get(draft.selected)).map_or_else(|| "Draft unavailable".into(), |option| {
                        let target = option.extractor_resource_label.as_ref().map_or_else(String::new, |label| format!(" targeting {label}"));
                        let costs = option.cost.iter().map(|row| format!("{} {}", row.quantity, row.label)).collect::<Vec<_>>().join(", ");
                        format!("Slot-first draft\n{:?}{target}\nCost: {costs}\nWork: {}\n\n[Up/Down Choice] [Enter Queue] [Esc Cancel]", option.role, option.required_work)
                    });
                    ("CONSTRUCTION DRAFT", text)
                }
                Confirmation::Probe => ("LAUNCH PROBE?", "Launch along the displayed redacted route?\n\n[Enter Launch] [Esc Cancel]".into()),
                Confirmation::Expedition => ("LAUNCH EXPEDITION?", "Commit stocks and resident population? Outcome remains hidden until report.\n\n[Enter Launch] [Esc Cancel]".into()),
            };
            render_confirmation_box(frame, area, title, &text);
        }
        Modal::Rejection(outcome) => render_confirmation_box(
            frame,
            area,
            "! COMMAND REJECTED",
            &format!(
                "! {}\n\nDraft: {:?}\n\n[Enter Edit Draft] [Esc Back]",
                outcome.message, outcome.draft_disposition
            ),
        ),
        Modal::Batch(batch) => {
            let mut lines = vec![
                Line::raw(format!(
                    "Completed {} of {}    Rate {}/sec    {:?}",
                    batch.completed(),
                    batch.requested,
                    batch.rate,
                    batch.status
                )),
                Line::raw(""),
                Line::raw("TICK   CHANGES"),
            ];
            for (index, step) in batch.history.iter().enumerate() {
                let mut visible = step
                    .delta
                    .stock_changes
                    .iter()
                    .map(|change| format!("{} {}->{}", change.label, change.before, change.after))
                    .chain(
                        step.delta.population_changes.iter().map(|change| {
                            format!("Population {}->{}", change.before, change.after)
                        }),
                    )
                    .collect::<Vec<_>>();
                if !step.delta.newly_identified_systems.is_empty() {
                    visible.push(format!(
                        "{} system(s) identified",
                        step.delta.newly_identified_systems.len()
                    ));
                }
                if step.delta.mission_changes > 0 {
                    visible.push(format!("{} mission change(s)", step.delta.mission_changes));
                }
                let changes = if visible.is_empty() {
                    "no visible change".into()
                } else {
                    visible.join("; ")
                };
                lines.push(selected_line(
                    index == batch.selected_history,
                    format!("{:>4}   {changes}", step.delta.to_tick),
                ));
            }
            if let Some(outcome) = &batch.rejection {
                lines.push(Line::raw(format!("! Rejected: {}", outcome.message)));
            }
            lines.push(Line::raw(""));
            lines.push(Line::raw(match batch.status {
                BatchStatus::Running => "[Space Pause] [Esc Stop]",
                BatchStatus::Paused => "[Space Resume] [Enter Step] [Esc Stop]",
                _ => "[Enter/Esc Close]",
            }));
            render_overlay_lines(frame, area, "MANUAL TICK BATCH", lines, 134, 37);
        }
        Modal::Mission(mission) => {
            let text = match &**mission {
                MissionDraft::Probe(value) => {
                    let route = value.route.as_ref().map_or_else(
                        || "-- no route --".into(),
                        |route| {
                            route_lines(route)
                                .into_iter()
                                .map(|line| line.to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        },
                    );
                    let jump_input = state
                        .mission_jump_input
                        .as_deref()
                        .map_or_else(|| value.requested_jump_limit.to_string(), str::to_owned);
                    let jump_error = state
                        .mission_jump_error
                        .as_ref()
                        .map_or_else(String::new, |error| format!("\n! {error}"));
                    let travel_energy = value
                        .travel_energy
                        .map_or_else(|| "--".into(), |energy| energy.to_string());
                    format!(
                        "Probe destination\n  {}\n\nJump distance ({}..={})\n> {}{}\n\nTravel energy\n  {}\n\nStatus\n  {}\n\n{route}\n\n[Type Distance] [Backspace Edit] [Up/Down Target] [Enter Review Launch] [Esc Back]",
                        value.target_label,
                        value.minimum_jump_limit,
                        value.maximum_jump_limit,
                        jump_input,
                        jump_error,
                        travel_energy,
                        availability_text(&value.availability),
                    )
                }
                MissionDraft::Expedition(value) => {
                    let reservations = if let Some(selected) = &value.reservations {
                        format!(
                            "Habitat slot: {}\nCollector slot: {}",
                            coordinate_label(&selected.habitat),
                            coordinate_label(&selected.collector)
                        )
                    } else if value.reservation_choices.len() >= 2 {
                        "Select Habitat and Collector slots with Left/Right.".into()
                    } else {
                        "Reservations will be selected automatically from summary knowledge.".into()
                    };
                    let commitment = value
                        .complete_commitment
                        .iter()
                        .map(|resource| format!("  {} {}", resource.quantity, resource.label))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "Expedition destination\n  {}\n\nResident population\n  {} available / {} required\n\nCommitted resources\n{}\n\nReservations\n{}\n\nStatus\n  {}\n\n[Up/Down Target] [Left/Right Reservations] [Enter Review Launch] [Esc Back]",
                        value.target_label,
                        value.resident_population_available,
                        value.resident_population_required,
                        if commitment.is_empty() {
                            "  None"
                        } else {
                            &commitment
                        },
                        reservations,
                        availability_text(&value.availability),
                    )
                }
            };
            render_confirmation_box(frame, area, "MISSION DRAFT", &text);
        }
    }
}

fn render_confirmation_box(frame: &mut Frame<'_>, area: Rect, title: &str, text: &str) {
    let lines = text
        .lines()
        .map(|line| Line::raw(line.to_owned()))
        .collect();
    render_overlay_lines(frame, area, title, lines, 112, 25);
}

fn render_overlay_lines(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    width: u16,
    height: u16,
) {
    let rect = centered(
        area,
        width.min(area.width.saturating_sub(4)),
        height.min(area.height.saturating_sub(4)),
    );
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!(" {title} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Action;
    use game_app::ProfileDescriptor;
    use ratatui::{Terminal, backend::TestBackend};
    use std::{collections::HashSet, path::PathBuf, time::Duration};

    fn has_color(backend: &TestBackend, color: Color) -> bool {
        let buffer = backend.buffer();
        (0..buffer.area.height).any(|y| (0..buffer.area.width).any(|x| buffer[(x, y)].fg == color))
    }

    fn has_text_with_color(backend: &TestBackend, needle: &str, color: Color) -> bool {
        let buffer = backend.buffer();
        (0..buffer.area.height).any(|y| {
            let row = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>();
            row.find(needle).is_some_and(|byte_index| {
                let x = row[..byte_index].chars().count() as u16;
                buffer[(x, y)].fg == color
            })
        })
    }

    fn text(backend: &TestBackend) -> String {
        let buffer = backend.buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_cloud_family_has_twelve_question_mark_free_variants() {
        for family in 1_u64..=4 {
            let mut variants = HashSet::new();
            for variant in 0_u64..12 {
                let hash = family + 5 * variant;
                let mut grid = vec![vec![(" ", Color::Reset); 80]; 35];
                place_system_visual(&mut grid, 40, 17, hash, false);
                let rendered = grid
                    .into_iter()
                    .flat_map(|row| row.into_iter().map(|cell| cell.0))
                    .collect::<String>();
                assert!(!rendered.contains('?'));
                variants.insert(rendered);
            }
            assert_eq!(variants.len(), 12);
        }
        assert_eq!(visual_assignment(0).0, SystemVisual::Plain);
    }

    #[test]
    fn minimum_canvas_renders_startup_with_textual_cues() {
        let mut terminal = Terminal::new(TestBackend::new(160, 45)).unwrap();
        let state = TuiState::new(
            ProfileDescriptor::new(PathBuf::from("starter.ron"), "starter"),
            u64::MAX,
        );
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let output = text(terminal.backend());
        assert!(output.contains("NEW WORLD"));
        assert!(output.contains("> Profile"));
        assert!(output.contains("[EMPTY] No generated preview"));
        assert!(output.contains("[F2/s Settings]"));
    }

    #[test]
    fn help_and_settings_modals_render_above_startup() {
        let mut terminal = Terminal::new(TestBackend::new(160, 45)).unwrap();
        let mut state = TuiState::new(
            ProfileDescriptor::new(PathBuf::from("starter.ron"), "starter"),
            0,
        );

        state.handle_action(Action::Help, Duration::ZERO).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let output = text(terminal.backend());
        assert!(output.contains(" HELP "));
        assert!(output.contains("NEW FRONTIER"));

        state.handle_action(Action::Cancel, Duration::ZERO).unwrap();
        state
            .handle_action(Action::Settings, Duration::ZERO)
            .unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        assert!(text(terminal.backend()).contains(" GLOBAL SETTINGS "));
    }

    #[test]
    fn playing_dashboard_has_synchronized_map_list_and_energy_cues() {
        let profile =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/profiles/starter.ron");
        let mut state = TuiState::new(ProfileDescriptor::new(profile, "starter"), 23);
        state.startup_focus = 3;
        state
            .handle_action(Action::Confirm, Duration::ZERO)
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(160, 45)).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let startup = text(terminal.backend());
        assert!(startup.contains("FRONTIER PREVIEW"));
        assert!(startup.contains('@'));

        state.startup_focus = 4;
        state
            .handle_action(Action::Confirm, Duration::ZERO)
            .unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let output = text(terminal.backend());
        assert!(output.contains("FRONTIER"));
        assert!(output.contains("> SYSTEMS"));
        assert!(!output.contains("> FRONTIER"));
        assert!(output.contains("@ Origin"));
        assert!(output.contains('░'));
        assert!(output.contains("ENERGY"));
        assert!(output.contains("-- no completed tick --"));

        state.selected_system = state
            .playing_view()
            .unwrap()
            .systems
            .iter()
            .position(|system| system.chart_position.is_none())
            .expect("starter world has an uncertain identified system");
        terminal.draw(|frame| render(frame, &state)).unwrap();
        assert!(has_color(terminal.backend(), Color::White));
        state.selected_system = 0;

        state
            .handle_action(Action::Confirm, Duration::ZERO)
            .unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let output = text(terminal.backend());
        assert!(output.contains("> SYSTEM KNOWLEDGE"));
        assert!(output.contains("PROBE / SURVEY INFORMATION"));

        state
            .handle_action(Action::Confirm, Duration::ZERO)
            .unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let output = text(terminal.backend());
        assert!(output.contains("> BODIES / SLOTS"));
        assert!(output.contains("[Up/Down Select Slot]"));
        assert!(!output.contains("[Tab Slot]"));
        assert!(output.contains("[Ore "));
        assert!(output.contains("System stocks"));
        assert!(output.contains("Energy"));
        assert!(output.contains("Alloy"));
        assert!(output.contains("Ore"));
        assert!(has_text_with_color(
            terminal.backend(),
            "[EMPTY]",
            Color::DarkGray
        ));
    }

    #[test]
    fn both_undersized_boundaries_are_safe_and_explicit() {
        for (width, height) in [(159, 45), (160, 44)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut state = TuiState::new(ProfileDescriptor::new("starter.ron", "starter"), 1);
            state.resize(width, height);
            terminal.draw(|frame| render(frame, &state)).unwrap();
            let output = text(terminal.backend());
            assert!(output.contains("TERMINAL TOO SMALL"));
            assert!(output.contains("Gameplay commands are blocked"));
        }
    }

    #[test]
    fn larger_canvas_uses_the_same_shell() {
        let mut terminal = Terminal::new(TestBackend::new(200, 55)).unwrap();
        let state = TuiState::new(ProfileDescriptor::new("starter.ron", "starter"), 7);
        terminal.draw(|frame| render(frame, &state)).unwrap();
        assert!(text(terminal.backend()).contains("4X-TERM / NEW FRONTIER"));
    }
}
