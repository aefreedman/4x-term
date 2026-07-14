//! Ratatui input/render adapter. This crate never accesses the ECS world.

pub mod input;
pub mod state;

pub use input::{InputAction, route_key};
pub use state::{
    Activity, InputLayer, LayoutClass, SortDirection, SystemOrderItem, SystemSortKey, UiState,
    classify_layout, order_systems,
};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use game_app::{
    AppHandle, AppRequest, ApplicationView, InvestmentKind, InvestmentStatus, RunState,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use std::io::stdout;

trait TerminalOps {
    fn enable_raw(&mut self) -> Result<()>;
    fn enter_alternate(&mut self) -> Result<()>;
    fn hide_cursor(&mut self) -> Result<()>;
    fn show_cursor(&mut self) -> Result<()>;
    fn leave_alternate(&mut self) -> Result<()>;
    fn disable_raw(&mut self) -> Result<()>;
}

struct RealTerminal;

impl TerminalOps for RealTerminal {
    fn enable_raw(&mut self) -> Result<()> {
        enable_raw_mode().map_err(Into::into)
    }
    fn enter_alternate(&mut self) -> Result<()> {
        execute!(stdout(), EnterAlternateScreen).map_err(Into::into)
    }
    fn hide_cursor(&mut self) -> Result<()> {
        execute!(stdout(), crossterm::cursor::Hide).map_err(Into::into)
    }
    fn show_cursor(&mut self) -> Result<()> {
        execute!(stdout(), crossterm::cursor::Show).map_err(Into::into)
    }
    fn leave_alternate(&mut self) -> Result<()> {
        execute!(stdout(), LeaveAlternateScreen).map_err(Into::into)
    }
    fn disable_raw(&mut self) -> Result<()> {
        disable_raw_mode().map_err(Into::into)
    }
}

struct TerminalGuard<O: TerminalOps> {
    ops: O,
    raw: bool,
    alternate: bool,
    cursor_hidden: bool,
}

impl<O: TerminalOps> TerminalGuard<O> {
    fn enter(ops: O) -> Result<Self> {
        let mut guard = Self {
            ops,
            raw: false,
            alternate: false,
            cursor_hidden: false,
        };
        guard.ops.enable_raw()?;
        guard.raw = true;
        guard.ops.enter_alternate()?;
        guard.alternate = true;
        guard.ops.hide_cursor()?;
        guard.cursor_hidden = true;
        Ok(guard)
    }
}

impl<O: TerminalOps> Drop for TerminalGuard<O> {
    fn drop(&mut self) {
        if self.cursor_hidden {
            let _ = self.ops.show_cursor();
        }
        if self.alternate {
            let _ = self.ops.leave_alternate();
        }
        if self.raw {
            let _ = self.ops.disable_raw();
        }
    }
}

fn restore_terminal() {
    let mut terminal = RealTerminal;
    let _ = terminal.show_cursor();
    let _ = terminal.leave_alternate();
    let _ = terminal.disable_raw();
}

fn install_terminal_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

pub async fn run(mut app: AppHandle) -> Result<()> {
    install_terminal_panic_hook();
    let guard = TerminalGuard::enter(RealTerminal)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;
    let mut events = EventStream::new();
    let mut ui = UiState::default();
    let mut view = app.views.borrow().clone();
    terminal.draw(|frame| render(frame, &view, &ui))?;

    loop {
        tokio::select! {
            input = events.next() => {
                match input {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        if handle_key(key.code, &mut ui, &view, &app).await? { break; }
                        view = app.views.borrow().clone();
                        terminal.draw(|frame| render(frame, &view, &ui))?;
                    }
                    Some(Ok(Event::Resize(_, _))) => { terminal.draw(|frame| render(frame, &view, &ui))?; }
                    Some(Err(error)) => return Err(error.into()),
                    None => break,
                    _ => {}
                }
            }
            changed = app.views.changed() => {
                if changed.is_err() { break; }
                view = app.views.borrow_and_update().clone();
                clamp_selection(&mut ui, &view);
                terminal.draw(|frame| render(frame, &view, &ui))?;
            }
        }
    }
    drop(terminal);
    drop(guard);
    app.shutdown().await?;
    Ok(())
}

async fn handle_key(
    code: KeyCode,
    ui: &mut UiState,
    view: &ApplicationView,
    app: &AppHandle,
) -> Result<bool> {
    if let Some(input) = &mut ui.quantity_input {
        match code {
            KeyCode::Char(digit) if digit.is_ascii_digit() && input.len() < 9 => input.push(digit),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Enter => {
                ui.trade_quantity = input.parse::<u32>().unwrap_or(1).max(1);
                ui.quantity_input = None;
                ui.input_layer = InputLayer::Root;
            }
            KeyCode::Esc => {
                ui.quantity_input = None;
                ui.input_layer = InputLayer::Root;
            }
            _ => {}
        }
        return Ok(false);
    }
    if ui.input_layer == InputLayer::Help {
        if matches!(code, KeyCode::Esc | KeyCode::Char('?')) {
            ui.help_visible = false;
            ui.input_layer = InputLayer::Root;
        }
        return Ok(false);
    }
    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::F(1) => ui.activity = Activity::Systems,
        KeyCode::F(2) => ui.activity = Activity::Trade,
        KeyCode::F(3) => ui.activity = Activity::Governance,
        KeyCode::F(4) => ui.activity = Activity::Intelligence,
        KeyCode::Char('?') => {
            ui.help_visible = !ui.help_visible;
            ui.input_layer = if ui.help_visible {
                InputLayer::Help
            } else {
                InputLayer::Root
            };
        }
        KeyCode::Char('n') if ui.activity == Activity::Trade => {
            ui.quantity_input = Some(String::new());
            ui.input_layer = InputLayer::Quantity;
        }
        KeyCode::Up | KeyCode::Char('k') => move_selection(ui, view, -1),
        KeyCode::Down | KeyCode::Char('j') => move_selection(ui, view, 1),
        KeyCode::Char('o') if ui.activity == Activity::Systems => {
            ui.system_sort = ui.system_sort.next();
            sync_system_row(ui, view);
        }
        KeyCode::Char('d') if ui.activity == Activity::Systems => {
            ui.sort_direction = ui.sort_direction.toggled();
            sync_system_row(ui, view);
        }
        KeyCode::Char(' ') => {
            let state = if view.run_state == RunState::Paused {
                RunState::Running
            } else {
                RunState::Paused
            };
            app.request(AppRequest::SetRunState(state)).await?;
        }
        KeyCode::Char('s') => {
            app.request(AppRequest::Step).await?;
        }
        KeyCode::Char('r') => {
            app.request(AppRequest::SetTickRate(view.tick_rate.next()))
                .await?;
        }
        KeyCode::Char('[') | KeyCode::Char(']') if ui.activity == Activity::Governance => {
            if !view.inspection.governor.governed {
                ui.message = "Selected market is read-only".into();
            } else {
                let mut policy = view.inspection.governor.policy.clone();
                policy.operating_reserve_ticks = if code == KeyCode::Char(']') {
                    policy.operating_reserve_ticks.saturating_add(1)
                } else {
                    policy.operating_reserve_ticks.saturating_sub(1)
                };
                if let Err(error) = app
                    .request(AppRequest::SetMarketPolicy {
                        system: view.selected_system.clone(),
                        policy,
                    })
                    .await
                {
                    ui.message = error.to_string();
                }
            }
        }
        KeyCode::Char(',') | KeyCode::Char('.') if ui.activity == Activity::Governance => {
            if !view.inspection.governor.governed {
                ui.message = "Selected market is read-only".into();
            } else {
                let mut policy = view.inspection.governor.policy.clone();
                policy.producer_margin_percent = if code == KeyCode::Char('.') {
                    policy.producer_margin_percent.saturating_add(1).min(10_000)
                } else {
                    policy.producer_margin_percent.saturating_sub(1)
                };
                if let Err(error) = app
                    .request(AppRequest::SetMarketPolicy {
                        system: view.selected_system.clone(),
                        policy,
                    })
                    .await
                {
                    ui.message = error.to_string();
                }
            }
        }
        KeyCode::Char('i') | KeyCode::Char('I')
            if ui.activity == Activity::Governance && !view.inspection.market.is_empty() =>
        {
            if !view.inspection.governor.governed {
                ui.message = "Selected market is read-only".into();
            } else {
                let mut policy = view.inspection.governor.policy.clone();
                let good = view.inspection.market[ui.market_index].good_id.clone();
                let current = policy.import_priorities.get(&good).copied().unwrap_or(100);
                let next = if code == KeyCode::Char('i') {
                    current.saturating_add(10).min(10_000)
                } else {
                    current.saturating_sub(10).max(1)
                };
                policy.import_priorities.insert(good, next);
                if let Err(error) = app
                    .request(AppRequest::SetMarketPolicy {
                        system: view.selected_system.clone(),
                        policy,
                    })
                    .await
                {
                    ui.message = error.to_string();
                }
            }
        }
        KeyCode::Char('-') | KeyCode::Char('+') | KeyCode::Char('=')
            if ui.activity == Activity::Governance
                && !view.inspection.governor.investments.is_empty() =>
        {
            if !view.inspection.governor.governed {
                ui.message = "Selected market is read-only".into();
            } else {
                let investment = &view.inspection.governor.investments[ui.investment_index];
                let mut policy = view.inspection.governor.investment_policy.clone();
                let current = policy
                    .allocation_percent
                    .get(&investment.kind)
                    .copied()
                    .unwrap_or(0);
                let other_total = policy
                    .allocation_percent
                    .iter()
                    .filter(|(kind, _)| **kind != investment.kind)
                    .map(|(_, value)| *value)
                    .sum::<u32>();
                let increase = code != KeyCode::Char('-');
                let next = if increase {
                    current
                        .saturating_add(5)
                        .min(100_u32.saturating_sub(other_total))
                } else {
                    current.saturating_sub(5)
                };
                policy.allocation_percent.insert(investment.kind, next);
                if let Err(error) = app
                    .request(AppRequest::SetInvestmentPolicy {
                        system: view.selected_system.clone(),
                        policy,
                    })
                    .await
                {
                    ui.message = error.to_string();
                }
            }
        }
        KeyCode::Char('b')
            if ui.activity == Activity::Trade && !view.local_trade.market.is_empty() =>
        {
            if !view.local_trade.available {
                ui.message = view
                    .local_trade
                    .unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "Trading is unavailable".into());
                return Ok(false);
            }
            let good = view.local_trade.market[ui.market_index].good_id.clone();
            if let Err(error) = app
                .request(AppRequest::Buy {
                    good,
                    quantity: ui.trade_quantity,
                })
                .await
            {
                ui.message = error.to_string();
            }
        }
        KeyCode::Char('x')
            if ui.activity == Activity::Trade && !view.local_trade.market.is_empty() =>
        {
            if !view.local_trade.available {
                ui.message = view
                    .local_trade
                    .unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "Trading is unavailable".into());
                return Ok(false);
            }
            let good = view.local_trade.market[ui.market_index].good_id.clone();
            if let Err(error) = app
                .request(AppRequest::Sell {
                    good,
                    quantity: ui.trade_quantity,
                })
                .await
            {
                ui.message = error.to_string();
            }
        }
        KeyCode::Enter if ui.activity == Activity::Systems && !view.systems.is_empty() => {
            let ordered =
                order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
            if let Some(destination) = selected_system_id(view, ui, &ordered)
                && let Err(error) = app.request(AppRequest::BeginTravel { destination }).await
            {
                ui.message = error.to_string();
            }
        }
        _ => {}
    }
    if ui.activity == Activity::Systems && !view.systems.is_empty() {
        let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
        if let Some(selected) = selected_system_id(view, ui, &ordered) {
            ui.selected_system = Some(selected.clone());
            ui.system_index = ordered
                .iter()
                .position(|system| system.id == selected)
                .unwrap_or(0);
            app.request(AppRequest::SelectSystem(selected)).await?;
        }
    }
    Ok(false)
}

fn move_selection(ui: &mut UiState, view: &ApplicationView, delta: isize) {
    match ui.activity {
        Activity::Systems => {
            let ordered =
                order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
            let current = selected_system_id(view, ui, &ordered)
                .and_then(|selected| ordered.iter().position(|system| system.id == selected))
                .unwrap_or(0);
            ui.system_index = shifted(current, ordered.len(), delta);
            ui.selected_system = ordered.get(ui.system_index).map(|system| system.id.clone());
        }
        Activity::Governance => {
            ui.investment_index = shifted(
                ui.investment_index,
                view.inspection.governor.investments.len(),
                delta,
            )
        }
        Activity::Trade => {
            ui.market_index = shifted(ui.market_index, view.local_trade.market.len(), delta);
        }
        Activity::Intelligence => {
            ui.event_scroll = ui.event_scroll.saturating_add_signed(delta as i16)
        }
    }
}

fn shifted(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        0
    } else {
        current.saturating_add_signed(delta).min(len - 1)
    }
}

fn sync_system_row(ui: &mut UiState, view: &ApplicationView) {
    let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
    let selected = selected_system_id(view, ui, &ordered);
    ui.system_index = selected
        .as_ref()
        .and_then(|selected| ordered.iter().position(|system| &system.id == selected))
        .unwrap_or(0);
    ui.selected_system = selected;
}

fn clamp_selection(ui: &mut UiState, view: &ApplicationView) {
    sync_system_row(ui, view);
    ui.market_index = ui
        .market_index
        .min(view.local_trade.market.len().saturating_sub(1));
    ui.investment_index = ui
        .investment_index
        .min(view.inspection.governor.investments.len().saturating_sub(1));
}

pub fn render(frame: &mut Frame<'_>, view: &ApplicationView, ui: &UiState) {
    let area = frame.area();
    let layout_class = classify_layout(area.width, area.height);
    if layout_class == LayoutClass::Unsupported {
        frame.render_widget(
            Paragraph::new(
                "Unsupported terminal size\n4x-term requires at least 80x30 cells\nResize the terminal or press q to quit",
            )
            .alignment(Alignment::Center)
            .block(Block::bordered().title("4x-term")),
            centered_rect(area.width.min(52), area.height.min(7), area),
        );
        return;
    }

    let shell = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);
    render_activity_bar(frame, shell[0], ui.activity);
    render_global_status(frame, shell[1], view);
    match ui.activity {
        Activity::Systems => render_systems_activity(frame, shell[2], view, ui, layout_class),
        Activity::Trade => {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(6)])
                .split(shell[2]);
            render_market(frame, panes[0], view, ui);
            render_player(frame, panes[1], view, ui);
        }
        Activity::Governance => render_details(frame, shell[2], view, ui),
        Activity::Intelligence => render_events(frame, shell[2], view, ui),
    }
    render_footer(frame, shell[3], view, ui);

    if let Some(input) = &ui.quantity_input {
        let popup = centered_rect(38, 5, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(format!("Quantity: {input}_\nEnter confirm · Esc cancel"))
                .block(Block::bordered().title("Trade Quantity")),
            popup,
        );
    } else if ui.help_visible {
        let popup = centered_rect(68, 11, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new("F1 Systems · F2 Trade · F3 Governance · F4 Intelligence\n↑/↓ or j/k: selection\nSpace: pause/run   s: single step   r: rate\nSystems: o cycle sort   d reverse sort   Enter travel\nTrade: n quantity   b buy   x sell\nGovernor: [/] reserve, ,/. margin, I/i import priority, -/+ allocation\n?: close help   q: quit")
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title("Help")),
            popup,
        );
    }
}

fn render_activity_bar(frame: &mut Frame<'_>, area: Rect, active: Activity) {
    let entries = [
        (Activity::Systems, "F1 Systems"),
        (Activity::Trade, "F2 Trade"),
        (Activity::Governance, "F3 Governance"),
        (Activity::Intelligence, "F4 Intelligence"),
    ];
    let spans = entries.into_iter().flat_map(|(activity, label)| {
        let style = if activity == active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        [Span::styled(format!(" {label} "), style), Span::raw(" ")]
    });
    frame.render_widget(Paragraph::new(Line::from_iter(spans)), area);
}

fn render_global_status(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView) {
    let status = format!(
        "{} · Tick {} · Rate {} · Location {} · Tank {}/{} E",
        if view.run_state == RunState::Paused {
            "PAUSED"
        } else {
            "RUNNING"
        },
        view.tick,
        view.tick_rate.label(),
        view.player.location_name,
        view.player.tank_energy.0,
        view.player.tank_capacity.0,
    );
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(Color::Cyan)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let local = match ui.activity {
        Activity::Systems => format!(
            "↑/↓ select · Enter travel · o sort ({}) · d direction ({})",
            ui.system_sort.label(),
            ui.sort_direction.symbol()
        ),
        Activity::Trade => format!(
            "↑/↓ good · n quantity ({}) · b buy · x sell",
            ui.trade_quantity
        ),
        Activity::Governance => "↑/↓ investment · [/] reserve · ,/. margin · -/+ allocation".into(),
        Activity::Intelligence => "↑/↓ scroll events".into(),
    };
    let message = if ui.message.is_empty() {
        String::new()
    } else {
        format!(" · {}", ui.message)
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{local} · Space run · s step · r rate · ? help · q quit{message} · fleet {}",
            view.fleet.active_npcs
        ))
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}

fn focused(ui: &UiState, activity: Activity) -> Style {
    if ui.activity == activity {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn system_order_items(view: &ApplicationView) -> Vec<SystemOrderItem> {
    view.systems
        .iter()
        .map(|system| {
            let capacity = system.energy_capacity.0.max(0) as u64;
            let stock = system.energy_stock.0.max(0) as u64;
            let energy_fill_percent = stock
                .saturating_mul(100)
                .checked_div(capacity)
                .unwrap_or(0)
                .min(100) as u32;
            let risk = match system.health {
                game_app::EnergyHealth::Deficit => 3,
                game_app::EnergyHealth::Low => 2,
                game_app::EnergyHealth::Healthy | game_app::EnergyHealth::Full => {
                    u8::from(system.brownout_stage.label() != "normal")
                }
            };
            SystemOrderItem {
                id: system.id.clone(),
                name: system.name.clone(),
                risk,
                runway_ticks: system.runway_ticks,
                energy_fill_percent,
                population: system.population.current,
                population_trend: system.population.trend,
                route_ticks: system.route_ticks_from_player,
                energy_stock: system.energy_stock,
            }
        })
        .collect()
}

fn selected_system_id(
    view: &ApplicationView,
    ui: &UiState,
    ordered: &[SystemOrderItem],
) -> Option<game_app::ContentId> {
    ui.selected_system
        .as_ref()
        .filter(|selected| ordered.iter().any(|system| &system.id == *selected))
        .cloned()
        .or_else(|| {
            ordered
                .iter()
                .find(|system| system.id == view.selected_system)
                .map(|system| system.id.clone())
        })
        .or_else(|| ordered.first().map(|system| system.id.clone()))
}

fn render_systems_activity(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let panes = match layout_class {
        LayoutClass::Regular => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(area),
        LayoutClass::Compact => Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area),
        LayoutClass::Unsupported => unreachable!("unsupported layouts return before composition"),
    };
    render_systems_table(frame, panes[0], view, ui);
    render_system_inspector(frame, panes[1], view, ui, layout_class);
}

fn render_systems_table(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
    let selected = selected_system_id(view, ui, &ordered);
    let rows = ordered.iter().filter_map(|ordered_system| {
        let system = view
            .systems
            .iter()
            .find(|system| system.id == ordered_system.id)?;
        let marker = if selected.as_ref() == Some(&system.id) {
            ">"
        } else {
            " "
        };
        let mut flags = Vec::new();
        if system.player_location {
            flags.push("LOC");
        }
        if system.player_governed {
            flags.push("GOV");
        }
        if ordered_system.risk > 0 {
            flags.push("WARN");
        }
        let energy = format!(
            "{} {}/{}",
            energy_gauge(ordered_system.energy_fill_percent, 6),
            system.energy_stock.0,
            system.energy_capacity.0
        );
        let population = format!(
            "{} {}",
            system.population.current,
            population_trend_marker(system.population.trend)
        );
        let route = system
            .route_ticks_from_player
            .map_or_else(|| "—".into(), |ticks| ticks.to_string());
        let style = if marker == ">" {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else if ordered_system.risk > 0 {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        };
        Some(
            Row::new(vec![
                Cell::from(marker),
                Cell::from(system.name.clone()),
                Cell::from(flags.join(" ")),
                right_cell(energy),
                right_cell(format!("{}t", system.runway_ticks)),
                right_cell(population),
                right_cell(route),
            ])
            .style(style),
        )
    });
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("Name"),
        Cell::from("LOC/GOV/WARN"),
        right_cell("Energy"),
        right_cell("Runway"),
        right_cell("Population"),
        right_cell("Route"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(12),
                Constraint::Length(12),
                Constraint::Length(20),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title(format!(
            "Systems — {} {}",
            ui.system_sort.label(),
            ui.sort_direction.symbol()
        ))),
        area,
    );
}

fn right_cell(value: impl Into<String>) -> Cell<'static> {
    Cell::from(Line::from(value.into()).alignment(Alignment::Right))
}

fn energy_gauge(percent: u32, width: usize) -> String {
    let filled = (percent.min(100) as usize * width).div_ceil(100);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(width - filled))
}

fn population_trend_marker(trend: game_app::PopulationTrend) -> &'static str {
    match trend {
        game_app::PopulationTrend::Growing => "↑",
        game_app::PopulationTrend::Stable => "→",
        game_app::PopulationTrend::Declining => "↓",
    }
}

fn render_system_inspector(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
    let selected = selected_system_id(view, ui, &ordered);
    let system = selected
        .as_ref()
        .and_then(|selected| view.systems.iter().find(|system| &system.id == selected));
    let title = if layout_class == LayoutClass::Regular {
        "Selected System Overview"
    } else {
        "System Detail"
    };
    let Some(system) = system else {
        frame.render_widget(
            Paragraph::new("No systems available").block(Block::bordered().title(title)),
            area,
        );
        return;
    };
    let fill = (system.energy_stock.0.max(0) as u64)
        .saturating_mul(100)
        .checked_div(system.energy_capacity.0.max(0) as u64)
        .unwrap_or(0)
        .min(100) as u32;
    let history = system
        .population
        .sufficiency_trajectory
        .iter()
        .rev()
        .take(12)
        .rev()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(" → ");
    let flags = [
        system.player_location.then_some("LOC"),
        system.player_governed.then_some("GOV"),
        (system.health.label() == "low" || system.health.label() == "deficit").then_some("WARNING"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let connections = system
        .connections
        .iter()
        .map(|connection| format!("{} {}t", connection.system_name, connection.travel_ticks))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![
        Line::from(format!("{}  {}", system.name, flags)),
        Line::from(format!(
            "Coordinates {:.1}, {:.1}, {:.1}",
            system.coordinates.0, system.coordinates.1, system.coordinates.2
        )),
        Line::from(format!(
            "Energy {} {}/{} E ({}%) · {} · {}",
            energy_gauge(fill, 10),
            system.energy_stock.0,
            system.energy_capacity.0,
            fill,
            system.health.label(),
            system.brownout_stage.label()
        )),
        Line::from(format!("Runway {} ticks", system.runway_ticks)),
        Line::from(format!(
            "Population {} {} · cap {} · tier {}",
            system.population.current,
            population_trend_marker(system.population.trend),
            system.population.carrying_capacity,
            system.population.tier
        )),
        Line::from(format!(
            "Population sufficiency {}% · history [{}]",
            system.population.sufficiency_average_percent, history
        )),
        Line::from(format!(
            "Route {} · Connections {}",
            system
                .route_ticks_from_player
                .map_or_else(|| "unreachable".into(), |ticks| format!("{ticks} ticks")),
            if connections.is_empty() {
                "none"
            } else {
                &connections
            }
        )),
    ];
    if system.id == view.inspection.system.id {
        let energy = &view.inspection.market_energy;
        lines.push(Line::from(format!(
            "Flow +{} / -{} · curtailed {} · life-support deficit {}",
            energy.generated.0,
            energy.burned.0,
            energy.curtailed.0,
            energy.unsupplied_life_support.0
        )));
        lines.push(Line::from(format!(
            "Season {}/{} base/effective · phase {}/{} {} · turn {} ({}t)",
            energy.seasonal_generation.base_output.0,
            energy.seasonal_generation.effective_output.0,
            energy.seasonal_generation.phase_ticks,
            energy.seasonal_generation.period_ticks,
            energy.seasonal_generation.trend.label(),
            energy
                .seasonal_generation
                .next_turning_point_tick
                .map_or_else(|| "beyond".into(), |tick| tick.to_string()),
            energy.seasonal_generation.ticks_until_turning_point
        )));
        lines.push(Line::from(format!(
            "History population changes {} · milestones {} · stage transitions {}",
            view.dynamics.population_changes,
            view.dynamics.population_milestones,
            view.dynamics.stage_transitions
        )));
        if energy.bootstrap_risk_acknowledged {
            lines.push(Line::from("Bootstrap risk: ACKNOWLEDGED"));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(title)),
        area,
    );
}

fn render_details(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, _ui: &UiState) {
    let system = view
        .systems
        .iter()
        .find(|system| system.id == view.selected_system);
    let mut lines = system.map_or_else(Vec::new, |system| {
        vec![
            Line::from(format!(
                "{} ({:.1}, {:.1}, {:.1})",
                system.name, system.coordinates.0, system.coordinates.1, system.coordinates.2
            )),
            Line::from(format!(
                "Direct: {}",
                system
                    .connections
                    .iter()
                    .map(|connection| format!(
                        "{} ({:.1} · {}t)",
                        connection.system_name, connection.distance, connection.travel_ticks
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ]
    });
    let population = &view.inspection.population;
    let recent_trajectory = population
        .sufficiency_trajectory
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let energy = &view.inspection.market_energy;
    lines.push(Line::from(format!(
        "Energy {}/{} · {} · {} · {}t runway",
        energy.stock.0,
        energy.capacity.0,
        energy.health.label(),
        energy.brownout_stage.label(),
        energy.runway_ticks
    )));
    lines.push(Line::from(format!(
        "Claims {} · operating {} · protected {} · purchasing {}",
        energy.reserved_claims.0,
        energy.operating_reserve.0,
        energy.protected_liquidation_budget.0,
        energy.unreserved_purchasing_energy.0
    )));
    lines.push(Line::from(format!(
        "Flow +{} / -{} · curtailed {} · life-support deficit {}",
        energy.generated.0, energy.burned.0, energy.curtailed.0, energy.unsupplied_life_support.0,
    )));
    if energy.bootstrap_risk_acknowledged {
        lines.push(Line::from("Bootstrap risk: ACKNOWLEDGED"));
    }
    lines.push(Line::from(format!(
        "Population {} · {} · cap {} · tier {} · sufficiency {}% [{}]",
        population.current,
        population.trend.label(),
        population.carrying_capacity,
        population.tier,
        population.sufficiency_average_percent,
        recent_trajectory,
    )));
    lines.push(Line::from(format!(
        "History population changes {} · milestones {} · stage transitions {}",
        view.dynamics.population_changes,
        view.dynamics.population_milestones,
        view.dynamics.stage_transitions,
    )));
    let season = energy.seasonal_generation;
    lines.push(Line::from(format!(
        "Season {}/{} base/effective · phase {}/{} {} · turn {} ({}t)",
        season.base_output.0,
        season.effective_output.0,
        season.phase_ticks,
        season.period_ticks,
        season.trend.label(),
        season
            .next_turning_point_tick
            .map_or_else(|| "beyond".into(), |tick| tick.to_string()),
        season.ticks_until_turning_point,
    )));
    lines.push(Line::from(format!(
        "Governor: {} · reserve {}t · margin {}% · tier {} · ladder {:?}/{}",
        if view.inspection.governor.governed {
            "PLAYER CONTROL"
        } else {
            "READ-ONLY AI"
        },
        view.inspection.governor.policy.operating_reserve_ticks,
        view.inspection.governor.policy.producer_margin_percent,
        view.inspection.governor.population_tier,
        view.inspection.governor.ladder_occupancy_ticks,
        view.inspection.governor.ladder_transitions,
    )));
    let priorities = view
        .inspection
        .governor
        .policy
        .import_priorities
        .iter()
        .filter_map(|(good, percent)| {
            view.inspection
                .market
                .iter()
                .find(|row| &row.good_id == good)
                .map(|row| format!("{} {}%", row.name, percent))
        })
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(Line::from(format!(
        "Import priorities: {}",
        if priorities.is_empty() {
            "defaults"
        } else {
            &priorities
        }
    )));
    let investment_lines = view
        .inspection
        .governor
        .investments
        .iter()
        .map(|investment| {
            format!(
                "{} {}% L{}/{} next {} cd {} {}",
                investment_kind_label(investment.kind),
                investment.allocation_percent,
                investment.level,
                investment.maximum_level,
                investment
                    .next_cost
                    .map_or_else(|| "max".into(), |cost| cost.0.to_string()),
                investment.cooldown_until,
                investment_status_label(&investment.status),
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    lines.push(Line::from(format!("Investments: {investment_lines}")));
    lines.push(Line::from(format!(
        "Route subsidy {}% · {}",
        view.inspection.governor.route_subsidy_percent,
        if view.inspection.governor.route_subsidy_active {
            "active"
        } else {
            "suppressed/inactive"
        }
    )));
    if let Some(route) = &view.selected_route {
        let mode = if route.current_leg.is_some() {
            format!("Traveling to {}", route.destination_name)
        } else {
            format!("Proposed route to {}", route.destination_name)
        };
        lines.push(Line::from(mode));
        lines.push(Line::from(format!(
            "{} jumps · {:.1} distance · {} ticks{}",
            route.legs.len(),
            route.total_distance,
            route.total_ticks,
            route
                .remaining_ticks
                .map_or(String::new(), |ticks| format!(" · {ticks} remaining"))
        )));
        if let Some(index) = route.current_leg {
            if let Some(leg) = route.legs.get(index) {
                lines.push(Line::from(format!(
                    "Leg {}/{}: {} → {}",
                    index + 1,
                    route.legs.len(),
                    leg.from_name,
                    leg.to_name
                )));
            }
        } else {
            lines.push(Line::from(format_route_chain(route)));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::bordered()
                .title("System / Route / Governor")
                .border_style(focused(_ui, Activity::Governance)),
        ),
        area,
    );
}

fn investment_kind_label(kind: InvestmentKind) -> &'static str {
    match kind {
        InvestmentKind::Collector => "Collector",
        InvestmentKind::Storage => "Storage",
        InvestmentKind::PopulationSupport => "Population",
        InvestmentKind::RouteSubsidy => "Subsidy",
    }
}

fn investment_status_label(status: &InvestmentStatus) -> String {
    match status {
        InvestmentStatus::Disabled => "disabled".into(),
        InvestmentStatus::DisabledByStage(stage) => format!("blocked:{}", stage.label()),
        InvestmentStatus::Unallocated => "unallocated".into(),
        InvestmentStatus::CoolingDown { until_tick } => format!("cooldown:{until_tick}"),
        InvestmentStatus::MaximumLevel => "maximum".into(),
        InvestmentStatus::InsufficientFunds { available, cost } => {
            format!("needs {}/{}", available.0, cost.0)
        }
        InvestmentStatus::Ready { .. } => "ready".into(),
        InvestmentStatus::Completed { tick, .. } => format!("completed@{tick}"),
    }
}

fn format_route_chain(route: &game_app::RouteView) -> String {
    let mut names = route
        .legs
        .first()
        .map(|leg| vec![leg.from_name.clone()])
        .unwrap_or_default();
    names.extend(route.legs.iter().map(|leg| leg.to_name.clone()));
    if names.len() > 6 {
        format!(
            "{} → {} → … → {} → {}",
            names[0],
            names[1],
            names[names.len() - 2],
            names[names.len() - 1]
        )
    } else {
        names.join(" → ")
    }
}

fn render_market(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let rows = view
        .local_trade
        .market
        .iter()
        .enumerate()
        .map(|(index, row)| {
            Row::new(vec![
                Cell::from(row.name.clone()),
                Cell::from(row.inventory.to_string()),
                Cell::from(row.target.to_string()),
                Cell::from(row.unit_cost.0.to_string()),
                Cell::from(row.funded_demand.to_string()),
                Cell::from(format!("{} E", row.buy_quote.0)),
                Cell::from(format!("{} E", row.sell_quote.0)),
            ])
            .style(if index == ui.market_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            })
        });
    let header = Row::new([
        "Good",
        "Stock",
        "Target",
        "Cost",
        "Funded",
        "Market buys",
        "Market sells",
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(28),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                "Local Market · {}{}",
                view.local_trade.system.name,
                if view.local_trade.available {
                    ""
                } else {
                    " · UNAVAILABLE"
                }
            ))
            .border_style(focused(ui, Activity::Trade)),
    );
    frame.render_widget(table, area);
}

fn render_player(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let p = &view.player;
    let cargo = if p.cargo.is_empty() {
        "empty".into()
    } else {
        p.cargo
            .iter()
            .map(|item| format!("{} x{}", item.good_name, item.quantity))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lines = vec![
        Line::from(vec![
            Span::raw(format!(
                "Location: {}{}  ",
                p.location_name,
                if p.traveling { " (traveling)" } else { "" }
            )),
            Span::raw(format!(
                "Tank {}/{} E  Total value {} E  Rank #{}",
                p.tank_energy.0, p.tank_capacity.0, p.total_energy_value.0, p.energy_value_rank
            )),
        ]),
        Line::from(format!(
            "Cargo bay {}/{} (bay energy {}, value {} E): {}",
            p.cargo_used, p.cargo_capacity, p.bay_energy, p.cargo_energy_value.0, cargo
        )),
        Line::from(format!(
            "Sales {} E | Gain {} E | Trades {} | Energy share {:.1}% | Route {} | Runway {}",
            p.sales_energy.0,
            p.realized_energy_gain.0,
            p.transactions,
            p.energy_value_share_percent,
            p.route_energy_required
                .map_or_else(|| "—".into(), |value| format!("{} E", value.0)),
            p.runway_jumps
                .map_or_else(|| "—".into(), |value| format!("{value} jumps"))
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Player / Trade")
                .border_style(focused(ui, Activity::Trade)),
        ),
        area,
    );
}

fn render_events(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let visible = area.height.saturating_sub(2) as usize;
    let start = view
        .events
        .len()
        .saturating_sub(visible + usize::from(ui.event_scroll));
    let end = (start + visible).min(view.events.len());
    let text = view.events[start..end]
        .iter()
        .map(|event| Line::from(event.text.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Recent Events")
                .border_style(focused(ui, Activity::Intelligence)),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_app::{
        AggregateDynamicsView, CargoItemView, ConnectionView, EnergyHealth,
        GovernorInvestmentPolicy, GovernorMarketPolicy, GovernorView, InvestmentView,
        LocalTradeView, MarketEnergyView, MarketRow, PlayerStatusView, PopulationView,
        PresentationEvent, RouteLegView, RouteView, SeasonalGenerationView, SystemIdentityView,
        SystemInspectionView, SystemListItem, TickRate,
    };
    use game_core::{
        BrownoutStage, ContentId, ENERGY_ID, EconomyConfig, Energy, FleetDynamics, FleetMode,
        GameDefinition, GameSession, GoodCategory, GoodDefinition, Governance, InvestmentPolicy,
        InvestmentStatus, MarketAuthority, MarketPolicy, PopulationState, PopulationTrend,
        Position3, RefuelPolicy, SeasonalGenerationState, SeasonalTrend, SystemDefinition,
        TraderDefinition,
    };
    use ratatui::backend::TestBackend;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    struct FakeOps {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail: Option<&'static str>,
    }

    impl FakeOps {
        fn call(&self, name: &'static str) -> Result<()> {
            self.calls.borrow_mut().push(name);
            if self.fail == Some(name) {
                anyhow::bail!("forced {name} failure");
            }
            Ok(())
        }
    }

    impl TerminalOps for FakeOps {
        fn enable_raw(&mut self) -> Result<()> {
            self.call("enable_raw")
        }
        fn enter_alternate(&mut self) -> Result<()> {
            self.call("enter_alternate")
        }
        fn hide_cursor(&mut self) -> Result<()> {
            self.call("hide_cursor")
        }
        fn show_cursor(&mut self) -> Result<()> {
            self.call("show_cursor")
        }
        fn leave_alternate(&mut self) -> Result<()> {
            self.call("leave_alternate")
        }
        fn disable_raw(&mut self) -> Result<()> {
            self.call("disable_raw")
        }
    }

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn test_session() -> GameSession {
        let ore = id("core:ore");
        let energy = id(ENERGY_ID);
        let systems = (0..2)
            .map(|index| SystemDefinition {
                id: id(&format!("core:s{index}")),
                name: format!("System {index}"),
                position: Position3 {
                    x: f64::from(index),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([(energy.clone(), 1_000), (ore.clone(), 10)]),
                targets: BTreeMap::from([(ore.clone(), 10)]),
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
                energy_storage_cap: Energy(2_000),
                population: 1,
                population_state: PopulationState {
                    current: 1,
                    reference: 1,
                    carrying_capacity: 1,
                    ..PopulationState::default()
                },
                investment_policy: InvestmentPolicy::default(),
                governance: if index == 0 {
                    Governance {
                        authority: MarketAuthority::Player(id("core:player")),
                    }
                } else {
                    Governance::default()
                },
                policy: MarketPolicy::default(),
                protected_liquidation_budget: Energy(10),
                bootstrap_risk_acknowledged: false,
            })
            .collect();
        GameSession::new(GameDefinition {
            goods: vec![
                GoodDefinition {
                    id: energy,
                    name: "Energy".into(),
                    category: GoodCategory::Energy,
                    bootstrap_cost: Energy(1),
                },
                GoodDefinition {
                    id: ore,
                    name: "Ore".into(),
                    category: GoodCategory::Raw,
                    bootstrap_cost: Energy(10),
                },
            ],
            recipes: vec![],
            systems,
            traders: vec![TraderDefinition {
                id: id("core:player"),
                name: "Player".into(),
                system: id("core:s0"),
                energy_tank: Energy(100),
                energy_tank_capacity: Energy(1_000),
                cargo_capacity: 10,
                speed: 1.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: true,
            }],
            fleet: FleetDynamics {
                mode: Some(FleetMode::Fixed { count: 0 }),
                ..FleetDynamics::default()
            },
            economy: EconomyConfig::default(),
        })
        .unwrap()
    }

    fn test_view() -> ApplicationView {
        let system = SystemIdentityView {
            id: id("core:s0"),
            name: "Aster Reach".into(),
        };
        let population = PopulationView {
            current: 5,
            reference: 5,
            carrying_capacity: 6,
            trend: PopulationTrend::Growing,
            tier: 2,
            sufficiency_average_percent: 94,
            sufficiency_trajectory: vec![90, 94, 98],
            settled_changes: 1,
        };
        let season = SeasonalGenerationView {
            base_output: Energy(40),
            effective_output: Energy(32),
            phase_ticks: 0,
            period_ticks: 20,
            trend: SeasonalTrend::Rising,
            ticks_until_turning_point: 10,
            next_turning_point_tick: Some(10),
        };
        let market_energy = MarketEnergyView {
            stock: Energy(800),
            capacity: Energy(1_000),
            reserved_claims: Energy(20),
            operating_reserve: Energy(100),
            protected_liquidation_budget: Energy(50),
            unreserved_purchasing_energy: Energy(630),
            generated: Energy(40),
            burned: Energy(25),
            curtailed: Energy(0),
            unsupplied_life_support: Energy(0),
            bootstrap_risk_acknowledged: false,
            health: EnergyHealth::Healthy,
            brownout_stage: BrownoutStage::Normal,
            runway_ticks: 80,
            seasonal_generation: season,
        };
        let market = vec![MarketRow {
            good_id: id("core:ore"),
            name: "Ore".into(),
            inventory: 10,
            target: 10,
            buy_quote: Energy(9),
            sell_quote: Energy(11),
            unit_cost: Energy(8),
            funded_demand: 3,
        }];
        let governor = GovernorView {
            governed: true,
            policy: GovernorMarketPolicy {
                producer_margin_percent: 15,
                operating_reserve_ticks: 3,
                import_priorities: BTreeMap::new(),
            },
            investment_policy: GovernorInvestmentPolicy {
                allocation_percent: BTreeMap::from([
                    (InvestmentKind::Collector, 30),
                    (InvestmentKind::Storage, 25),
                    (InvestmentKind::PopulationSupport, 25),
                    (InvestmentKind::RouteSubsidy, 20),
                ]),
            },
            investments: [
                InvestmentKind::Collector,
                InvestmentKind::Storage,
                InvestmentKind::PopulationSupport,
                InvestmentKind::RouteSubsidy,
            ]
            .into_iter()
            .map(|kind| InvestmentView {
                kind,
                allocation_percent: match kind {
                    InvestmentKind::Collector => 30,
                    InvestmentKind::Storage | InvestmentKind::PopulationSupport => 25,
                    InvestmentKind::RouteSubsidy => 20,
                },
                level: u32::from(kind == InvestmentKind::RouteSubsidy),
                maximum_level: 4,
                next_cost: Some(Energy(300)),
                cooldown_until: 0,
                status: InvestmentStatus::Ready { cost: Energy(300) },
                effect_per_level: 10,
            })
            .collect(),
            route_subsidy_percent: 10,
            route_subsidy_active: true,
            ladder_occupancy_ticks: [10, 2, 1, 0],
            ladder_transitions: 3,
            population_tier: 2,
        };
        ApplicationView {
            tick: 0,
            run_state: RunState::Paused,
            tick_rate: TickRate::Normal,
            systems: vec![SystemListItem {
                id: system.id.clone(),
                name: "Aster".into(),
                coordinates: (0.0, 0.0, 0.0),
                player_location: true,
                player_governed: true,
                route_distance_from_player: Some(0.0),
                route_ticks_from_player: Some(0),
                population: population.clone(),
                energy_stock: Energy(800),
                energy_capacity: Energy(1_000),
                health: EnergyHealth::Healthy,
                brownout_stage: BrownoutStage::Normal,
                runway_ticks: 80,
                seasonal_generation: season,
                connections: vec![ConnectionView {
                    system_id: id("core:s1"),
                    system_name: "Brasshaven".into(),
                    distance: 3.5,
                    travel_ticks: 4,
                }],
            }],
            selected_system: system.id.clone(),
            selected_route: None,
            governed_system: Some(system.clone()),
            inspection: SystemInspectionView {
                system: system.clone(),
                read_only_market: false,
                market_energy,
                population,
                market: market.clone(),
                governor,
            },
            local_trade: LocalTradeView {
                system,
                available: true,
                unavailable_reason: None,
                market,
            },
            dynamics: AggregateDynamicsView {
                stage_occupancy_ticks: [10, 2, 1, 0],
                stage_transitions: 3,
                population_changes: 1,
                population_milestones: 1,
            },
            player: PlayerStatusView {
                location: id("core:s0"),
                location_name: "Aster Reach".into(),
                tank_energy: Energy(100),
                tank_capacity: Energy(250),
                bay_energy: 0,
                cargo: vec![CargoItemView {
                    good_id: id("core:ore"),
                    good_name: "Ferrite Ore".into(),
                    quantity: 2,
                }],
                cargo_used: 2,
                cargo_capacity: 10,
                cargo_energy_value: Energy(18),
                total_energy_value: Energy(118),
                purchase_energy: Energy(18),
                sales_energy: Energy(0),
                realized_energy_gain: Energy(-18),
                units_moved: 0,
                transactions: 0,
                energy_value_rank: 1,
                energy_value_share_percent: 100.0,
                sales_share_percent: 0.0,
                route_energy_required: None,
                runway_jumps: Some(5),
                traveling: false,
            },
            fleet: game_app::FleetView::default(),
            events: vec![],
        }
    }

    #[test]
    fn partial_terminal_setup_is_cleaned_up() {
        for (failure, expected) in [
            (
                "enter_alternate",
                vec!["enable_raw", "enter_alternate", "disable_raw"],
            ),
            (
                "hide_cursor",
                vec![
                    "enable_raw",
                    "enter_alternate",
                    "hide_cursor",
                    "leave_alternate",
                    "disable_raw",
                ],
            ),
        ] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let result = TerminalGuard::enter(FakeOps {
                calls: Rc::clone(&calls),
                fail: Some(failure),
            });
            assert!(result.is_err());
            assert_eq!(*calls.borrow(), expected);
        }
    }

    #[test]
    fn complete_terminal_setup_is_cleaned_up_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let guard = TerminalGuard::enter(FakeOps {
            calls: Rc::clone(&calls),
            fail: None,
        })
        .unwrap();
        drop(guard);
        assert_eq!(
            *calls.borrow(),
            vec![
                "enable_raw",
                "enter_alternate",
                "hide_cursor",
                "show_cursor",
                "leave_alternate",
                "disable_raw"
            ]
        );
    }

    fn rendered_view(view: &ApplicationView) -> String {
        rendered_activity(view, Activity::Systems)
    }

    fn rendered_activity(view: &ApplicationView, activity: Activity) -> String {
        let backend = TestBackend::new(100, 35);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let ui = UiState {
            activity,
            ..UiState::default()
        };
        terminal.draw(|frame| render(frame, view, &ui)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn test_backend_displays_normal_full_low_and_deficit_energy_states() {
        for (stock, health, deficit, label) in [
            (Energy(800), EnergyHealth::Healthy, Energy(0), "healthy"),
            (Energy(1_000), EnergyHealth::Full, Energy(0), "full"),
            (Energy(120), EnergyHealth::Low, Energy(0), "low"),
            (Energy(0), EnergyHealth::Deficit, Energy(7), "deficit"),
        ] {
            let mut view = test_view();
            view.systems[0].energy_stock = stock;
            view.systems[0].health = health;
            view.inspection.market_energy.stock = stock;
            view.inspection.market_energy.health = health;
            view.inspection.market_energy.unsupplied_life_support = deficit;
            let rendered = rendered_view(&view);
            assert!(
                rendered.contains(&format!("{}/1000 E", stock.0)),
                "missing {label} exact energy display"
            );
            assert!(rendered.contains(label), "missing {label} health display");
            assert!(rendered.contains(&format!("life-support deficit {}", deficit.0)));
        }
    }

    #[test]
    fn test_backend_renders_every_brownout_stage_and_transition_text() {
        for stage in [
            BrownoutStage::Normal,
            BrownoutStage::Throttled,
            BrownoutStage::Emergency,
            BrownoutStage::Starvation,
        ] {
            let mut view = test_view();
            view.systems[0].brownout_stage = stage;
            view.inspection.market_energy.brownout_stage = stage;
            view.inspection.market_energy.runway_ticks = match stage {
                BrownoutStage::Normal => 80,
                BrownoutStage::Throttled => 12,
                BrownoutStage::Emergency => 6,
                BrownoutStage::Starvation => 0,
            };
            view.systems[0].runway_ticks = view.inspection.market_energy.runway_ticks;
            view.local_trade.market[0].buy_quote = if stage >= BrownoutStage::Emergency {
                Energy::ZERO
            } else {
                Energy(9)
            };
            view.events = vec![PresentationEvent {
                sequence: 1,
                text: format!(
                    "Aster Reach brownout stage Normal → {} at tick 7 ({} ticks runway)",
                    stage.label(),
                    view.inspection.market_energy.runway_ticks
                ),
            }];
            let rendered = rendered_view(&view);
            assert!(
                rendered.contains(stage.label()),
                "missing stage {}",
                stage.label()
            );
            assert!(rendered.contains(&format!(
                "Runway {} ticks",
                view.inspection.market_energy.runway_ticks
            )));
            assert!(rendered.contains("Season 40/32 base/effective"));
            assert!(rendered.contains("phase 0/20 rising · turn 10 (10t)"));
            assert!(rendered.contains("Population 5 ↑ · cap 6 · tier 2"));
            assert!(rendered.contains("History population changes 1 · milestones 1"));

            let intelligence = rendered_activity(&view, Activity::Intelligence);
            assert!(intelligence.contains(&format!("Normal → {}", stage.label())));
            if stage >= BrownoutStage::Emergency {
                let trade = rendered_activity(&view, Activity::Trade);
                assert!(trade.contains("0 E"), "suppressed distress bid missing");
            }
        }
    }

    #[test]
    fn test_backend_renders_governor_status_and_read_only_markets() {
        let governed = rendered_activity(&test_view(), Activity::Governance);
        assert!(governed.contains("Governor: PLAYER CONTROL"));
        assert!(governed.contains("Investments:"));
        assert!(governed.contains("Route subsidy 10% · active"));
        assert!(governed.contains("Collector 30% L0/4 next 300 cd 0 ready"));

        let mut readonly = test_view();
        readonly.inspection.governor.governed = false;
        readonly.inspection.governor.route_subsidy_active = false;
        let readonly = rendered_activity(&readonly, Activity::Governance);
        assert!(readonly.contains("Governor: READ-ONLY AI"));
        assert!(readonly.contains("Route subsidy 10% · suppressed/inactive"));
    }

    #[test]
    fn renders_normal_constrained_and_edge_case_views() {
        let base = test_view();
        let mut edge = base.clone();
        edge.systems[0].name =
            "A very long frontier system name that must be clipped safely".into();
        edge.player.tank_energy = Energy(i64::MAX);
        edge.player.total_energy_value = Energy(i64::MAX);
        edge.inspection.market_energy.health = EnergyHealth::Deficit;
        edge.inspection.market_energy.unsupplied_life_support = Energy(5);
        edge.inspection.market_energy.bootstrap_risk_acknowledged = true;
        edge.player.traveling = true;
        edge.selected_route = Some(RouteView {
            destination_id: id("core:s1"),
            destination_name: "Brasshaven".into(),
            legs: vec![RouteLegView {
                from_id: id("core:s0"),
                from_name: "Aster Reach".into(),
                to_id: id("core:s1"),
                to_name: "Brasshaven".into(),
                distance: 42.5,
                travel_ticks: 8,
            }],
            current_leg: Some(0),
            total_distance: 42.5,
            total_ticks: 8,
            remaining_ticks: Some(7),
        });
        edge.events = (1..=20)
            .map(|sequence| PresentationEvent {
                sequence,
                text: "Rejected: insufficient cargo capacity".into(),
            })
            .collect();

        for (width, height, view) in [
            (100, 35, &base),
            (70, 24, &edge),
            (100, 35, &edge),
            (40, 10, &base),
            (100, 12, &base),
        ] {
            let backend = TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| render(frame, view, &UiState::default()))
                .unwrap();
        }

        let systems = rendered_at(160, 45, &edge, &UiState::default());
        for label in [
            "F1 Systems",
            "Systems — Name ↑",
            "Selected System Overview",
            "Energy",
            "Population",
            "deficit",
            "Bootstrap risk: ACKNOWLEDGED",
        ] {
            assert!(
                systems.contains(label),
                "missing Systems surface label {label}"
            );
        }
        assert!(!systems.contains("Local Market"));
        assert!(!systems.contains("Recent Events"));
        assert!(!systems.contains("core:"));

        let trade = rendered_activity(&base, Activity::Trade);
        for label in ["Local Market", "Funded", "Player / Trade", "Cargo bay 2/10"] {
            assert!(trade.contains(label), "missing Trade surface label {label}");
        }
        assert!(!trade.contains("Funds:"));
        assert!(!trade.contains('¤'));

        let governance = rendered_activity(&base, Activity::Governance);
        assert!(governance.contains("Governor: PLAYER CONTROL"));
        assert!(governance.contains("Investments:"));

        let intelligence = rendered_activity(&edge, Activity::Intelligence);
        assert!(intelligence.contains("Recent Events"));
        assert!(intelligence.contains("Rejected: insufficient cargo capacity"));

        for (width, height) in [(79, 30), (80, 29), (40, 10), (100, 12)] {
            let rendered = rendered_at(width, height, &base, &UiState::default());
            assert!(rendered.contains("Unsupported terminal size"));
        }

        let help = UiState {
            help_visible: true,
            ..UiState::default()
        };
        assert!(rendered_at(100, 35, &edge, &help).contains("Help"));
        let quantity = UiState {
            quantity_input: Some("123".into()),
            ..UiState::default()
        };
        assert!(rendered_at(100, 35, &edge, &quantity).contains("Trade Quantity"));
    }

    #[tokio::test]
    async fn governor_keys_edit_through_app_requests_and_show_feedback_for_read_only_or_rejection()
    {
        let app = game_app::spawn(test_session());
        let mut ui = UiState {
            activity: Activity::Governance,
            ..UiState::default()
        };
        let mut view = app.views.borrow().clone();
        handle_key(KeyCode::Char(']'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.governor.policy.operating_reserve_ticks, 4);
        handle_key(KeyCode::Char('.'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.governor.policy.producer_margin_percent, 16);
        handle_key(KeyCode::Char('+'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(
            view.inspection.governor.investments[0].allocation_percent,
            5
        );

        let mut invalid = view.clone();
        invalid
            .inspection
            .governor
            .policy
            .import_priorities
            .insert(invalid.inspection.market[0].good_id.clone(), 10_000);
        handle_key(KeyCode::Char('i'), &mut ui, &invalid, &app)
            .await
            .unwrap();
        assert!(ui.message.contains("invalid market policy"));

        app.request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let readonly = app.views.borrow().clone();
        assert!(!readonly.inspection.governor.governed);
        handle_key(KeyCode::Char(']'), &mut ui, &readonly, &app)
            .await
            .unwrap();
        assert_eq!(ui.message, "Selected market is read-only");
        app.shutdown().await.unwrap();
    }

    fn rendered_at(width: u16, height: u16, view: &ApplicationView, ui: &UiState) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, view, ui)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn responsive_mode_shell_uses_exact_breakpoints_and_only_relevant_surfaces() {
        let view = test_view();
        for (width, height) in [(79, 30), (80, 29)] {
            let rendered = rendered_at(width, height, &view, &UiState::default());
            assert!(rendered.contains("Unsupported terminal size"));
            assert!(rendered.contains("80x30"));
            assert!(!rendered.contains("Local Market"));
        }

        for (width, height) in [(80, 30), (159, 44)] {
            let rendered = rendered_at(width, height, &view, &UiState::default());
            assert!(rendered.contains("F1 Systems"));
            assert!(rendered.contains("Systems — Name ↑"));
            assert!(!rendered.contains("Local Market"));
            assert!(!rendered.contains("Recent Events"));
        }

        for (width, height) in [(160, 45), (200, 60)] {
            let rendered = rendered_at(width, height, &view, &UiState::default());
            assert!(rendered.contains("F1 Systems"));
            assert!(rendered.contains("Systems — Name ↑"));
            assert!(rendered.contains("Selected System Overview"));
            assert!(!rendered.contains("Local Market"));
        }
    }

    #[test]
    fn systems_table_marks_stable_selection_identities_and_sort_without_leaking_ids() {
        let view = test_view();
        let ui = UiState {
            selected_system: Some(id("core:s1")),
            system_sort: SystemSortKey::Risk,
            sort_direction: SortDirection::Descending,
            ..UiState::default()
        };
        let rendered = rendered_at(160, 45, &view, &ui);
        assert!(rendered.contains("Systems — Risk ↓"));
        assert!(rendered.contains(">"));
        assert!(rendered.contains("LOC"));
        assert!(rendered.contains("GOV"));
        assert!(rendered.contains("Energy"));
        assert!(rendered.contains("Population"));
        assert!(!rendered.contains("core:"));
    }

    #[tokio::test]
    async fn required_keys_map_to_local_state_and_application_requests() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState::default();
        let mut view = app.views.borrow().clone();

        handle_key(KeyCode::Char('s'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.tick, 1);

        handle_key(KeyCode::Char('r'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.tick_rate, TickRate::Fast);
        handle_key(KeyCode::Char(' '), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.run_state, RunState::Running);
        handle_key(KeyCode::Char(' '), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.run_state, RunState::Paused);

        handle_key(KeyCode::F(2), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('n'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('2'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.trade_quantity, 2);
        handle_key(KeyCode::Char('b'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.player.cargo_used, 2);
        handle_key(KeyCode::Char('x'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.player.cargo_used, 0);

        handle_key(KeyCode::F(1), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('?'), &mut ui, &view, &app)
            .await
            .unwrap();
        assert!(ui.help_visible);
        handle_key(KeyCode::Char('?'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s1"));
        assert_eq!(view.tick, 1, "UI navigation must not advance simulation");
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert!(view.player.traveling);

        handle_key(KeyCode::F(3), &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.activity, Activity::Governance);
        handle_key(KeyCode::F(2), &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.activity, Activity::Trade);
        handle_key(KeyCode::Up, &mut ui, &view, &app).await.unwrap();
        assert!(
            handle_key(KeyCode::Char('q'), &mut ui, &view, &app)
                .await
                .unwrap()
        );
        app.shutdown().await.unwrap();
    }
}
