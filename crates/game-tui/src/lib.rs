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
                        let size = terminal.size()?;
                        let layout_supported = classify_layout(size.width, size.height) != LayoutClass::Unsupported;
                        if handle_key_for_layout(key.code, &mut ui, &view, &app, layout_supported).await? { break; }
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

#[cfg(test)]
async fn handle_key(
    code: KeyCode,
    ui: &mut UiState,
    view: &ApplicationView,
    app: &AppHandle,
) -> Result<bool> {
    handle_key_for_layout(code, ui, view, app, true).await
}

async fn handle_key_for_layout(
    code: KeyCode,
    ui: &mut UiState,
    view: &ApplicationView,
    app: &AppHandle,
    layout_supported: bool,
) -> Result<bool> {
    let action = route_key(code, ui, layout_supported);
    match action {
        InputAction::None => return Ok(false),
        InputAction::Quit => return Ok(true),
        InputAction::CloseLayer => {
            match ui.input_layer {
                InputLayer::Quantity => {
                    ui.quantity_input = None;
                    ui.message = "Quantity unchanged".into();
                }
                InputLayer::Help | InputLayer::Detail | InputLayer::Root => {}
            }
            ui.input_layer = InputLayer::Root;
        }
        InputAction::QuantityDigit(digit) => {
            if let Some(input) = &mut ui.quantity_input
                && input.len() < 9
            {
                input.push(digit);
            }
        }
        InputAction::QuantityBackspace => {
            if let Some(input) = &mut ui.quantity_input {
                input.pop();
            }
        }
        InputAction::ConfirmQuantity => {
            let quantity = ui
                .quantity_input
                .as_deref()
                .unwrap_or_default()
                .parse::<u32>()
                .unwrap_or(1)
                .max(1);
            ui.trade_quantity = quantity;
            ui.quantity_input = None;
            ui.input_layer = InputLayer::Root;
            ui.message = format!("Quantity set to {quantity}");
        }
        InputAction::Switch(Activity::Systems) => ui.activity = Activity::Systems,
        InputAction::Switch(Activity::Trade) => {
            if ui.activity == Activity::Systems && !view.player.traveling {
                let target = ui
                    .selected_system
                    .clone()
                    .unwrap_or_else(|| view.selected_system.clone());
                let reachable = view.systems.iter().any(|system| {
                    system.id == target
                        && system.id != view.player.location
                        && system.route_ticks_from_player.is_some()
                });
                if reachable {
                    ui.route_proposal = Some(target);
                } else {
                    ui.message = "Selected system has no available travel route".into();
                }
            }
            if !view.player.traveling
                && let Some(proposal) = ui.route_proposal.clone()
                && proposal != view.selected_system
                && let Err(error) = app.request(AppRequest::SelectSystem(proposal)).await
            {
                ui.message = error.to_string();
            }
            ui.activity = Activity::Trade;
        }
        InputAction::Switch(Activity::Governance) => {
            ui.activity = Activity::Governance;
            ui.governance_inspection = None;
            ui.governance_index = 0;
            if let Some(governed) = &view.governed_system {
                if let Err(error) = app
                    .request(AppRequest::SelectSystem(governed.id.clone()))
                    .await
                {
                    ui.message = error.to_string();
                }
            } else {
                ui.message = "No player-governed system is available".into();
            }
        }
        InputAction::Switch(Activity::Intelligence) => {
            ui.activity = Activity::Intelligence;
            ui.reconcile_events(&view.events);
        }
        InputAction::ToggleHelp => ui.input_layer = InputLayer::Help,
        InputAction::ToggleRun => {
            let state = if view.run_state == RunState::Paused {
                RunState::Running
            } else {
                RunState::Paused
            };
            app.request(AppRequest::SetRunState(state)).await?;
        }
        InputAction::Step => {
            app.request(AppRequest::Step).await?;
        }
        InputAction::CycleTickRate => {
            app.request(AppRequest::SetTickRate(view.tick_rate.next()))
                .await?;
        }
        InputAction::MoveUp | InputAction::MoveDown => {
            let delta = if action == InputAction::MoveUp { -1 } else { 1 };
            move_selection(ui, view, delta);
            if ui.activity == Activity::Systems
                && let Some(selected) = ui.selected_system.clone()
            {
                app.request(AppRequest::SelectSystem(selected)).await?;
            }
        }
        InputAction::Sort => {
            ui.system_sort = ui.system_sort.next();
            sync_system_row(ui, view);
        }
        InputAction::ToggleSortDirection => {
            ui.sort_direction = ui.sort_direction.toggled();
            sync_system_row(ui, view);
        }
        InputAction::OpenDetail => ui.input_layer = InputLayer::Detail,
        InputAction::OpenQuantity => {
            if view.local_trade.market.is_empty() {
                ui.message = "No local market goods are available".into();
            } else {
                ui.quantity_input = Some(String::new());
                ui.input_layer = InputLayer::Quantity;
            }
        }
        InputAction::Buy => {
            if let Some(row) = view.local_trade.market.get(ui.market_index) {
                if let Some(reason) = buy_unavailable_reason(view, row, ui.trade_quantity) {
                    ui.message = reason;
                } else {
                    match app
                        .request(AppRequest::Buy {
                            good: row.good_id.clone(),
                            quantity: ui.trade_quantity,
                        })
                        .await
                    {
                        Ok(()) => {
                            ui.message = format!("Bought {} ×{}", row.name, ui.trade_quantity)
                        }
                        Err(error) => ui.message = error.to_string(),
                    }
                }
            } else {
                ui.message = "No local market goods are available".into();
            }
        }
        InputAction::Sell => {
            if let Some(row) = view.local_trade.market.get(ui.market_index) {
                if let Some(reason) = sell_unavailable_reason(view, row, ui.trade_quantity) {
                    ui.message = reason;
                } else {
                    match app
                        .request(AppRequest::Sell {
                            good: row.good_id.clone(),
                            quantity: ui.trade_quantity,
                        })
                        .await
                    {
                        Ok(()) => ui.message = format!("Sold {} ×{}", row.name, ui.trade_quantity),
                        Err(error) => ui.message = error.to_string(),
                    }
                }
            } else {
                ui.message = "No local market goods are available".into();
            }
        }
        InputAction::BeginTravel => {
            if view.player.traveling {
                ui.message =
                    "Already in transit; local trading and new travel are unavailable".into();
            } else if let Some(destination) = ui.route_proposal.clone() {
                let route = view
                    .selected_route
                    .as_ref()
                    .filter(|route| route.destination_id == destination);
                if let Some(route) = route {
                    if route.required_energy > view.player.tank_energy {
                        ui.message = format!(
                            "Travel unavailable: needs {} E but tank holds {} E",
                            route.required_energy.0, view.player.tank_energy.0
                        );
                    } else {
                        match app
                            .request(AppRequest::BeginTravel {
                                destination: destination.clone(),
                            })
                            .await
                        {
                            Ok(()) => {
                                ui.message = format!(
                                    "Travel begun toward {}",
                                    system_name(view, &destination)
                                );
                            }
                            Err(error) => ui.message = error.to_string(),
                        }
                    }
                } else {
                    ui.message =
                        "Travel unavailable: exact route details are not selected in the app view"
                            .into();
                }
            } else {
                ui.message = "No route proposal; select a system and enter Trade with F2".into();
            }
        }
        InputAction::ClearContext if ui.activity == Activity::Trade => {
            ui.route_proposal = None;
            ui.message = "Route proposal cleared".into();
        }
        InputAction::Inspect => {
            if let Some(target) = ui.selected_system.clone() {
                match app.request(AppRequest::SelectSystem(target.clone())).await {
                    Ok(()) => {
                        ui.governance_inspection = Some(target);
                        ui.governance_index = 0;
                        ui.message =
                            "Inspecting Systems selection (read-only if autonomous)".into();
                    }
                    Err(error) => ui.message = error.to_string(),
                }
            } else {
                ui.message = "No stable Systems selection to inspect".into();
            }
        }
        InputAction::ClearContext => {
            if ui.governance_inspection.is_some()
                && let Some(governed) = &view.governed_system
            {
                match app
                    .request(AppRequest::SelectSystem(governed.id.clone()))
                    .await
                {
                    Ok(()) => {
                        ui.governance_inspection = None;
                        ui.governance_index = 0;
                        ui.message = "Returned to governed system".into();
                    }
                    Err(error) => ui.message = error.to_string(),
                }
            }
        }
        InputAction::Decrease => edit_governance(ui, view, app, -1, None).await,
        InputAction::Increase => edit_governance(ui, view, app, 1, None).await,
    }
    Ok(false)
}

async fn edit_governance(
    ui: &mut UiState,
    view: &ApplicationView,
    app: &AppHandle,
    delta: isize,
    row_override: Option<usize>,
) {
    if !view.inspection.governor.governed || ui.governance_inspection.is_some() {
        ui.message = "Selected market is read-only".into();
        return;
    }
    let row = row_override.unwrap_or(ui.governance_index);
    let system = view.inspection.system.id.clone();
    if row == 0 {
        let mut policy = view.inspection.governor.policy.clone();
        policy.operating_reserve_ticks = policy
            .operating_reserve_ticks
            .saturating_add_signed(delta as i32)
            .min(10_000);
        let value = policy.operating_reserve_ticks;
        match app
            .request(AppRequest::SetMarketPolicy { system, policy })
            .await
        {
            Ok(()) => ui.message = format!("Operating reserve updated to {value} ticks"),
            Err(error) => ui.message = error.to_string(),
        }
        return;
    }
    if row == 1 {
        let mut policy = view.inspection.governor.policy.clone();
        policy.producer_margin_percent = policy
            .producer_margin_percent
            .saturating_add_signed(delta as i32)
            .min(10_000);
        let value = policy.producer_margin_percent;
        match app
            .request(AppRequest::SetMarketPolicy { system, policy })
            .await
        {
            Ok(()) => ui.message = format!("Producer margin updated to {value}%"),
            Err(error) => ui.message = error.to_string(),
        }
        return;
    }

    let import_index = row - 2;
    if let Some(market) = view.inspection.market.get(import_index) {
        let mut policy = view.inspection.governor.policy.clone();
        let current = policy
            .import_priorities
            .get(&market.good_id)
            .copied()
            .unwrap_or(100);
        let amount = (delta as i32).saturating_mul(10);
        let next = current.saturating_add_signed(amount).clamp(1, 10_000);
        policy
            .import_priorities
            .insert(market.good_id.clone(), next);
        match app
            .request(AppRequest::SetMarketPolicy { system, policy })
            .await
        {
            Ok(()) => ui.message = format!("{} import priority updated to {next}%", market.name),
            Err(error) => ui.message = error.to_string(),
        }
        return;
    }

    let investment_index = import_index - view.inspection.market.len();
    let Some(investment) = view.inspection.governor.investments.get(investment_index) else {
        return;
    };
    ui.investment_index = investment_index;
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
        .fold(0_u32, |total, (_, value)| total.saturating_add(*value));
    let next = if delta > 0 {
        current
            .saturating_add(5)
            .min(100_u32.saturating_sub(other_total))
    } else {
        current.saturating_sub(5)
    };
    if next == current {
        ui.message = if delta > 0 {
            "Allocation total is limited to 100%".into()
        } else {
            "Allocation is already 0%".into()
        };
        return;
    }
    policy.allocation_percent.insert(investment.kind, next);
    match app
        .request(AppRequest::SetInvestmentPolicy { system, policy })
        .await
    {
        Ok(()) => {
            ui.message = format!(
                "{} allocation updated to {next}%",
                investment_kind_label(investment.kind)
            );
        }
        Err(error) => ui.message = error.to_string(),
    }
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
            let policy_rows = 2;
            let import_rows = view.inspection.market.len();
            let total = policy_rows + import_rows + view.inspection.governor.investments.len();
            ui.governance_index = shifted(ui.governance_index, total, delta);
            if ui.governance_index >= policy_rows + import_rows {
                ui.investment_index = ui.governance_index - policy_rows - import_rows;
            }
        }
        Activity::Trade => {
            ui.market_index = shifted(ui.market_index, view.local_trade.market.len(), delta);
        }
        Activity::Intelligence => ui.scroll_events(&view.events, delta),
    }
}

fn shifted(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        0
    } else {
        current.saturating_add_signed(delta).min(len - 1)
    }
}

fn viewport(total: usize, selected: usize, capacity: usize) -> (usize, usize) {
    if total == 0 || capacity == 0 {
        return (0, 0);
    }
    let capacity = capacity.min(total);
    let start = selected
        .saturating_add(1)
        .saturating_sub(capacity)
        .min(total - capacity);
    (start, start + capacity)
}

fn viewport_label(start: usize, end: usize, total: usize) -> String {
    if total == 0 {
        return "empty".into();
    }
    let before = if start > 0 { " ↑more" } else { "" };
    let after = if end < total { " ↓more" } else { "" };
    format!("{}-{end}/{total}{before}{after}", start + 1)
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
    let governance_rows =
        2 + view.inspection.market.len() + view.inspection.governor.investments.len();
    ui.governance_index = ui.governance_index.min(governance_rows.saturating_sub(1));
    ui.investment_index = ui
        .investment_index
        .min(view.inspection.governor.investments.len().saturating_sub(1));
    ui.reconcile_events(&view.events);
    if !view.player.traveling && ui.route_proposal.as_ref() == Some(&view.player.location) {
        ui.route_proposal = None;
    }
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
            render_trade_activity(frame, shell[2], view, ui, layout_class);
        }
        Activity::Governance => {
            render_governance_activity(frame, shell[2], view, ui, layout_class);
        }
        Activity::Intelligence => {
            render_intelligence_activity(frame, shell[2], view, ui, layout_class);
        }
    }
    render_footer(frame, shell[3], view, ui);

    if ui.input_layer == InputLayer::Quantity {
        let input = ui.quantity_input.as_deref().unwrap_or_default();
        let popup = centered_rect(54, 8, area);
        let (good, buy_total, sell_total) =
            view.local_trade.market.get(ui.market_index).map_or_else(
                || ("No good selected".into(), "—".into(), "—".into()),
                |row| {
                    let quantity = input.parse::<u32>().unwrap_or(1).max(1);
                    (
                        row.name.clone(),
                        total_label(row.sell_quote, quantity),
                        total_label(row.buy_quote, quantity),
                    )
                },
            );
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(format!(
                "Good: {good}\nQuantity: {input}_\nBuy total: {buy_total} · Sell total: {sell_total}\nEnter confirm · Esc cancel"
            ))
            .block(Block::bordered().title("Trade Quantity Preview")),
            popup,
        );
    } else if ui.input_layer == InputLayer::Help {
        let popup = centered_rect(72, 13, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(help_text(ui.activity))
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title("Contextual Help")),
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
        let is_active = activity == active;
        let style = if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let marker = if is_active { "* " } else { "  " };
        [
            Span::styled(format!(" {marker}{label} "), style),
            Span::raw(" "),
        ]
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
    let mut spans = Vec::new();
    match ui.activity {
        Activity::Systems => {
            if ui.input_layer == InputLayer::Detail {
                spans.push(Span::raw("System detail · Esc return"));
            } else {
                spans.push(Span::raw("↑/↓ Select · Enter detail · S("));
                spans.push(shortcut_span("o"));
                spans.push(Span::raw(format!(")rt {} · (", ui.system_sort.label())));
                spans.push(shortcut_span("D"));
                spans.push(Span::raw(format!(
                    ")irection {} · ",
                    ui.sort_direction.symbol()
                )));
                let selected = ui.selected_system.as_ref().unwrap_or(&view.selected_system);
                let route_available = view.systems.iter().any(|system| {
                    &system.id == selected
                        && system.id != view.player.location
                        && system.route_ticks_from_player.is_some()
                });
                if view.player.traveling {
                    spans.push(Span::raw("F2 route disabled: in transit"));
                } else if route_available {
                    spans.push(Span::raw("F2 propose selected route"));
                } else {
                    spans.push(Span::raw("F2 route disabled: unreachable/already here"));
                }
            }
        }
        Activity::Trade => {
            spans.push(Span::raw("↑/↓ Good · ("));
            spans.push(shortcut_span("N"));
            spans.push(Span::raw(format!(") Qty {} · ", ui.trade_quantity)));
            if let Some(row) = view.local_trade.market.get(ui.market_index) {
                let buy_reason = buy_unavailable_reason(view, row, ui.trade_quantity);
                let sell_reason = sell_unavailable_reason(view, row, ui.trade_quantity);
                spans.push(Span::raw("("));
                spans.push(shortcut_span("B"));
                spans.push(Span::raw(buy_reason.map_or_else(
                    || ")uy · ".into(),
                    |reason| format!(")uy disabled: {} · ", action_reason(&reason)),
                )));
                spans.push(Span::raw("("));
                spans.push(shortcut_span("X"));
                spans.push(Span::raw(sell_reason.map_or_else(
                    || ") sell · ".into(),
                    |reason| format!(") sell disabled: {} · ", action_reason(&reason)),
                )));
            } else {
                spans.push(Span::raw("("));
                spans.push(shortcut_span("B"));
                spans.push(Span::raw(")uy / ("));
                spans.push(shortcut_span("X"));
                spans.push(Span::raw(") sell disabled: no good · "));
            }
            spans.push(Span::raw("("));
            spans.push(shortcut_span("T"));
            let matching_route = ui.route_proposal.as_ref().and_then(|proposal| {
                view.selected_route
                    .as_ref()
                    .filter(|route| &route.destination_id == proposal)
            });
            if view.player.traveling {
                spans.push(Span::raw(")ravel disabled: in transit"));
            } else if ui.route_proposal.is_none() {
                spans.push(Span::raw(")ravel disabled: no route"));
            } else if let Some(route) = matching_route {
                if route.required_energy > view.player.tank_energy {
                    spans.push(Span::raw(format!(
                        ")ravel disabled: needs {} E",
                        route.required_energy.0
                    )));
                } else {
                    spans.push(Span::raw(")ravel · Esc clear route"));
                }
            } else {
                spans.push(Span::raw(")ravel disabled: route details unavailable"));
            }
        }
        Activity::Governance => {
            spans.push(Span::raw("↑/↓ Row · "));
            if view.inspection.governor.governed && ui.governance_inspection.is_none() {
                spans.push(Span::raw("←/→ Edit · "));
            } else {
                spans.push(Span::raw("Edit disabled: read-only · "));
            }
            spans.push(Span::raw("("));
            spans.push(shortcut_span("I"));
            spans.push(Span::raw(")nspect Systems selection · Esc governed target"));
        }
        Activity::Intelligence => {
            spans.push(Span::raw("↑/↓ Scroll events · newest resumes tail-follow"));
        }
    }
    spans.push(Span::raw(" · Space run · "));
    spans.push(shortcut_span("s"));
    spans.push(Span::raw(" step · "));
    spans.push(shortcut_span("r"));
    spans.push(Span::raw(" rate · "));
    spans.push(shortcut_span("?"));
    spans.push(Span::raw(" help · "));
    spans.push(shortcut_span("q"));
    spans.push(Span::raw(format!(
        " quit · fleet {}",
        view.fleet.active_npcs
    )));
    if !ui.message.is_empty() {
        spans.push(Span::raw(format!(" · {}", ui.message)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn shortcut_span(label: &'static str) -> Span<'static> {
    Span::styled(
        label,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

fn action_reason(reason: &str) -> &str {
    reason
        .strip_prefix("Buy unavailable: ")
        .or_else(|| reason.strip_prefix("Sell unavailable: "))
        .unwrap_or(reason)
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
        LayoutClass::Compact => {
            if ui.input_layer == InputLayer::Detail {
                render_system_inspector(frame, area, view, ui, layout_class);
            } else {
                render_systems_table(frame, area, view, ui);
            }
            return;
        }
        LayoutClass::Unsupported => unreachable!("unsupported layouts return before composition"),
    };
    render_systems_table(frame, panes[0], view, ui);
    render_system_inspector(frame, panes[1], view, ui, layout_class);
}

fn render_systems_table(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
    let selected = selected_system_id(view, ui, &ordered);
    let selected_index = selected
        .as_ref()
        .and_then(|selected| ordered.iter().position(|system| &system.id == selected))
        .unwrap_or(0);
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let (start, end) = viewport(ordered.len(), selected_index, capacity);
    let mut rows = ordered[start..end]
        .iter()
        .filter_map(|ordered_system| {
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
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No systems available"),
        ]));
    }
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
    let widths = if area.width >= 100 {
        [
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(12),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(6),
        ]
    } else {
        [
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(5),
        ]
    };
    frame.render_widget(
        Table::new(rows, widths)
            .header(header)
            .column_spacing(1)
            .block(Block::bordered().title(format!(
                "Systems — {} {} · {}",
                ui.system_sort.label(),
                ui.sort_direction.symbol(),
                viewport_label(start, end, ordered.len())
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

fn render_governance_activity(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(8),
        ])
        .split(area);
    let governor = &view.inspection.governor;
    let editable = governor.governed && ui.governance_inspection.is_none();
    let authority = if ui.governance_inspection.is_some() {
        "READ-ONLY INSPECTION"
    } else if governor.governed {
        "PLAYER CONTROL"
    } else {
        "READ-ONLY AI"
    };
    let inspection = if ui.governance_inspection.is_some() {
        " · explicit Systems inspection"
    } else {
        " · governed target"
    };
    let feedback = if ui.message.is_empty() {
        format!(
            "Governor: {authority}{inspection} · Route subsidy {}% · {}",
            governor.route_subsidy_percent,
            if governor.route_subsidy_active {
                "active"
            } else {
                "suppressed/inactive"
            }
        )
    } else {
        format!("Governor: {authority}{inspection} · {}", ui.message)
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(feedback),
            Line::from(format!(
                "Energy {} E purchasing · Population tier {} · ladder {:?}/{}",
                view.inspection.market_energy.unreserved_purchasing_energy.0,
                governor.population_tier,
                governor.ladder_occupancy_ticks,
                governor.ladder_transitions,
            )),
        ])
        .block(
            Block::bordered()
                .title(format!("Governance — {}", view.inspection.system.name))
                .border_style(focused(ui, Activity::Governance)),
        ),
        panes[0],
    );

    let policy_rows = [
        (
            "Operating reserve",
            format!("{} ticks", governor.policy.operating_reserve_ticks),
        ),
        (
            "Producer margin",
            format!("{}%", governor.policy.producer_margin_percent),
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, value))| {
        let selected = ui.governance_index == index;
        Row::new(vec![
            Cell::from(if selected { ">" } else { " " }),
            Cell::from(name),
            right_cell(value),
            Cell::from(if editable {
                "←/→ edit"
            } else {
                "read-only"
            }),
        ])
        .style(selected_style(selected))
    });
    frame.render_widget(
        Table::new(
            policy_rows,
            [
                Constraint::Length(1),
                Constraint::Percentage(40),
                Constraint::Length(12),
                Constraint::Min(10),
            ],
        )
        .header(bold_row(["", "Policy", "Value", "Control"]))
        .column_spacing(1)
        .block(Block::bordered().title("Policy")),
        panes[1],
    );

    let import_capacity = usize::from(panes[2].height.saturating_sub(3)).max(1);
    let import_selected = ui
        .governance_index
        .checked_sub(2)
        .filter(|index| *index < view.inspection.market.len())
        .unwrap_or(0);
    let (import_start, import_end) = viewport(
        view.inspection.market.len(),
        import_selected,
        import_capacity,
    );
    let mut import_rows = view.inspection.market[import_start..import_end]
        .iter()
        .enumerate()
        .map(|(offset, market)| {
            let index = import_start + offset;
            let selected = ui.governance_index == index + 2;
            let priority = governor
                .policy
                .import_priorities
                .get(&market.good_id)
                .copied()
                .unwrap_or(100);
            Row::new(vec![
                Cell::from(if selected { ">" } else { " " }),
                Cell::from(market.name.clone()),
                right_cell(format!("{priority}%")),
                right_cell(market.funded_demand.to_string()),
                Cell::from(if editable {
                    "←/→ ±10%"
                } else {
                    "read-only"
                }),
            ])
            .style(selected_style(selected))
        })
        .collect::<Vec<_>>();
    if import_rows.is_empty() {
        import_rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No import priorities"),
        ]));
    }
    frame.render_widget(
        Table::new(
            import_rows,
            [
                Constraint::Length(1),
                Constraint::Percentage(35),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(10),
            ],
        )
        .header(bold_row(["", "Import", "Priority", "Funded", "Control"]))
        .column_spacing(1)
        .block(Block::bordered().title(format!(
            "Import Priorities · {}",
            viewport_label(import_start, import_end, view.inspection.market.len())
        ))),
        panes[2],
    );

    let investment_start = 2 + view.inspection.market.len();
    let total = governor
        .investments
        .iter()
        .fold(0_u32, |total, investment| {
            total.saturating_add(investment.allocation_percent)
        });
    let investment_capacity = usize::from(panes[3].height.saturating_sub(3)).max(1);
    let investment_selected = ui
        .governance_index
        .checked_sub(investment_start)
        .filter(|index| *index < governor.investments.len())
        .unwrap_or_else(|| {
            ui.investment_index
                .min(governor.investments.len().saturating_sub(1))
        });
    let (investment_view_start, investment_view_end) = viewport(
        governor.investments.len(),
        investment_selected,
        investment_capacity,
    );
    let mut investment_rows = governor.investments[investment_view_start..investment_view_end]
        .iter()
        .enumerate()
        .map(|(offset, investment)| {
            let index = investment_view_start + offset;
            let selected = ui.governance_index == investment_start + index;
            let marker = Cell::from(if selected { ">" } else { " " });
            let name = Cell::from(investment_kind_label(investment.kind));
            let allocation = format!("{}%", investment.allocation_percent);
            let bar = allocation_bar(investment.allocation_percent, 10);
            let cost = investment
                .next_cost
                .map_or_else(|| "MAX".into(), |cost| format!("{} E", cost.0));
            let status = investment_status_label(&investment.status);
            let cells = if layout_class == LayoutClass::Regular {
                vec![
                    marker,
                    name,
                    right_cell(allocation),
                    Cell::from(bar),
                    right_cell(investment.level.to_string()),
                    right_cell(investment.maximum_level.to_string()),
                    right_cell(cost),
                    right_cell(investment.cooldown_until.to_string()),
                    Cell::from(status),
                ]
            } else {
                vec![
                    marker,
                    name,
                    Cell::from(format!("{allocation} {bar}")),
                    right_cell(format!("{}/{}", investment.level, investment.maximum_level)),
                    right_cell(cost),
                    Cell::from(status),
                ]
            };
            Row::new(cells).style(selected_style(selected))
        })
        .collect::<Vec<_>>();
    if investment_rows.is_empty() {
        investment_rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No investments configured"),
        ]));
    }
    let title = format!(
        "Investments — Allocation Total {total}% / 100% maximum{} · {}",
        if editable { "" } else { " — read-only" },
        viewport_label(
            investment_view_start,
            investment_view_end,
            governor.investments.len()
        )
    );
    if layout_class == LayoutClass::Regular {
        frame.render_widget(
            Table::new(
                investment_rows,
                [
                    Constraint::Length(1),
                    Constraint::Min(12),
                    Constraint::Length(10),
                    Constraint::Length(12),
                    Constraint::Length(6),
                    Constraint::Length(5),
                    Constraint::Length(10),
                    Constraint::Length(9),
                    Constraint::Min(12),
                ],
            )
            .header(bold_row([
                "",
                "Investment",
                "Allocation",
                "Bar",
                "Level",
                "Max",
                "Cost",
                "Cooldown",
                "Status",
            ]))
            .column_spacing(1)
            .block(Block::bordered().title(title)),
            panes[3],
        );
    } else {
        frame.render_widget(
            Table::new(
                investment_rows,
                [
                    Constraint::Length(1),
                    Constraint::Min(10),
                    Constraint::Length(18),
                    Constraint::Length(7),
                    Constraint::Length(9),
                    Constraint::Min(10),
                ],
            )
            .header(bold_row([
                "",
                "Investment",
                "Allocation",
                "Level",
                "Cost",
                "Status",
            ]))
            .column_spacing(1)
            .block(Block::bordered().title(title)),
            panes[3],
        );
    }
}

fn selected_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn bold_row<const N: usize>(cells: [&'static str; N]) -> Row<'static> {
    Row::new(cells).style(Style::default().add_modifier(Modifier::BOLD))
}

fn allocation_bar(percent: u32, width: usize) -> String {
    let filled = (percent.min(100) as usize * width).div_ceil(100);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(width - filled))
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

fn render_trade_activity(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    if layout_class == LayoutClass::Regular {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(area);
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(7)])
            .split(columns[0]);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(columns[1]);
        render_local_market(frame, left[0], view, ui, layout_class);
        render_trade_action(frame, left[1], view, ui, layout_class);
        render_trade_route(frame, right[0], view, ui);
        render_trade_player(frame, right[1], view, ui);
    } else {
        let panes = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(38),
                Constraint::Length(7),
                Constraint::Length(6),
                Constraint::Min(4),
            ])
            .split(area);
        render_local_market(frame, panes[0], view, ui, layout_class);
        render_trade_action(frame, panes[1], view, ui, layout_class);
        render_trade_route(frame, panes[2], view, ui);
        render_trade_player(frame, panes[3], view, ui);
    }
}

fn render_local_market(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let selected_index = ui
        .market_index
        .min(view.local_trade.market.len().saturating_sub(1));
    let (start, end) = viewport(view.local_trade.market.len(), selected_index, capacity);
    let mut rows = view.local_trade.market[start..end]
        .iter()
        .enumerate()
        .map(|(offset, row)| {
            let index = start + offset;
            let selected = index == ui.market_index;
            let common = vec![
                Cell::from(if selected { ">" } else { " " }),
                Cell::from(row.name.clone()),
                right_cell(row.inventory.to_string()),
                right_cell(row.funded_demand.to_string()),
                right_cell(format!("{} E", row.buy_quote.0)),
                right_cell(format!("{} E", row.sell_quote.0)),
            ];
            let cells = if layout_class == LayoutClass::Regular {
                let mut cells = common;
                cells.insert(3, right_cell(row.target.to_string()));
                cells.insert(4, right_cell(format!("{} E", row.unit_cost.0)));
                cells
            } else {
                common
            };
            Row::new(cells).style(selected_style(selected))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No market goods available"),
        ]));
    }
    let title = format!(
        "Local Market — {}{} · {}",
        view.local_trade.system.name,
        if view.local_trade.available {
            ""
        } else {
            " — UNAVAILABLE"
        },
        viewport_label(start, end, view.local_trade.market.len())
    );
    if layout_class == LayoutClass::Regular {
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(1),
                    Constraint::Min(12),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(9),
                    Constraint::Length(9),
                    Constraint::Length(12),
                    Constraint::Length(12),
                ],
            )
            .header(bold_row([
                "",
                "Good",
                "Stock",
                "Target",
                "Cost",
                "Funded",
                "Market buys",
                "Market sells",
            ]))
            .column_spacing(1)
            .block(Block::bordered().title(title)),
            area,
        );
    } else {
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(1),
                    Constraint::Min(12),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(12),
                    Constraint::Length(12),
                ],
            )
            .header(bold_row([
                "",
                "Good",
                "Stock",
                "Funded",
                "Market buys",
                "Market sells",
            ]))
            .column_spacing(1)
            .block(Block::bordered().title(title)),
            area,
        );
    }
}

fn render_trade_action(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let lines = view.local_trade.market.get(ui.market_index).map_or_else(
        || {
            vec![
                Line::from("No goods are listed at the local market."),
                Line::from("(B)uy unavailable · sell (X) unavailable"),
            ]
        },
        |row| {
            let held = held_quantity(view, row);
            let buy_total = quote_total(row.sell_quote, ui.trade_quantity);
            let sell_total = quote_total(row.buy_quote, ui.trade_quantity);
            let buy_reason = buy_unavailable_reason(view, row, ui.trade_quantity);
            let sell_reason = sell_unavailable_reason(view, row, ui.trade_quantity);
            if layout_class == LayoutClass::Compact {
                vec![
                    Line::from(format!(
                        "> {} · Qty {} · Held {} · Stock {}",
                        row.name, ui.trade_quantity, held, row.inventory
                    )),
                    Line::from(format!(
                        "Buy {} E/unit · Total {}",
                        row.sell_quote.0,
                        buy_total
                            .map_or_else(|| "overflow".into(), |total| format!("{} E", total.0))
                    )),
                    Line::from(format!(
                        "(B)uy {}",
                        availability_label(buy_reason.as_deref())
                    )),
                    Line::from(format!(
                        "Sell {} E/unit · Total {}",
                        row.buy_quote.0,
                        sell_total
                            .map_or_else(|| "overflow".into(), |total| format!("{} E", total.0))
                    )),
                    Line::from(format!(
                        "Sell (X) {}",
                        availability_label(sell_reason.as_deref())
                    )),
                ]
            } else {
                vec![
                    Line::from(format!(
                        "> {} · Qty {} · Held {} · Market stock {}",
                        row.name, ui.trade_quantity, held, row.inventory
                    )),
                    Line::from(format!(
                        "Buy {} E/unit · Total {} · Tank {}→{} E · Cargo {}→{}/{}",
                        row.sell_quote.0,
                        buy_total
                            .map_or_else(|| "overflow".into(), |total| format!("{} E", total.0)),
                        view.player.tank_energy.0,
                        buy_total.map_or(view.player.tank_energy.0, |total| {
                            view.player.tank_energy.0.saturating_sub(total.0)
                        }),
                        view.player.cargo_used,
                        view.player
                            .cargo_used
                            .saturating_add(u64::from(ui.trade_quantity)),
                        view.player.cargo_capacity,
                    )),
                    Line::from(format!(
                        "Sell {} E/unit · Total {} · Tank after {} E",
                        row.buy_quote.0,
                        sell_total
                            .map_or_else(|| "overflow".into(), |total| format!("{} E", total.0)),
                        sell_total.map_or(view.player.tank_energy.0, |total| {
                            view.player.tank_energy.0.saturating_add(total.0)
                        }),
                    )),
                    Line::from(format!(
                        "(B)uy {}",
                        availability_label(buy_reason.as_deref())
                    )),
                    Line::from(format!(
                        "Sell (X) {}",
                        availability_label(sell_reason.as_deref())
                    )),
                ]
            }
        },
    );
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Selected Good / Action Preview")),
        area,
    );
}

fn render_trade_route(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let target = if view.player.traveling {
        view.selected_route
            .as_ref()
            .map(|route| route.destination_id.clone())
    } else {
        ui.route_proposal.clone()
    };
    let remote = target
        .as_ref()
        .filter(|target| *target != &view.player.location)
        .or_else(|| {
            (view.inspection.system.id != view.player.location)
                .then_some(&view.inspection.system.id)
        });
    let title = remote.map_or_else(
        || "Route".into(),
        |target| {
            format!(
                "Remote Inspection — {} (read-only)",
                system_name(view, target)
            )
        },
    );
    let lines = if view.player.traveling {
        view.selected_route.as_ref().map_or_else(
            || vec![Line::from("In Transit · route progress unavailable")],
            |route| {
                let remaining = route.remaining_ticks.unwrap_or(route.total_ticks);
                let elapsed = route.total_ticks.saturating_sub(remaining);
                let leg = route.current_leg.and_then(|index| route.legs.get(index));
                vec![
                    Line::from(format!("In Transit to {}", route.destination_name)),
                    Line::from(format!(
                        "Progress {elapsed}/{} ticks · {remaining} remaining · {} jumps",
                        route.total_ticks,
                        route.legs.len()
                    )),
                    Line::from(leg.map_or_else(
                        || format_route_chain(route),
                        |leg| format!("Current leg: {} → {}", leg.from_name, leg.to_name),
                    )),
                    Line::from(
                        view.local_trade
                            .unavailable_reason
                            .clone()
                            .unwrap_or_else(|| "Local trading disabled during transit".into()),
                    ),
                ]
            },
        )
    } else if let Some(destination) = &ui.route_proposal {
        let route = view
            .selected_route
            .as_ref()
            .filter(|route| &route.destination_id == destination);
        let summary = route.map_or_else(
            || route_summary_from_system(view, destination),
            |route| {
                format!(
                    "{} jumps · {:.1} distance · {} ticks",
                    route.legs.len(),
                    route.total_distance,
                    route.total_ticks
                )
            },
        );
        let energy = route.map_or_else(
            || "Required energy unavailable for this proposal".into(),
            |route| {
                format!(
                    "Requires {} E · after arrival {} E",
                    route.required_energy.0,
                    view.player
                        .tank_energy
                        .0
                        .saturating_sub(route.required_energy.0)
                )
            },
        );
        let command = route.map_or_else(
            || "(T)ravel disabled: exact route details unavailable · Esc clears proposal".into(),
            |route| {
                if route.required_energy > view.player.tank_energy {
                    format!(
                        "Travel disabled: needs {} E; tank holds {} E · Esc clears proposal",
                        route.required_energy.0, view.player.tank_energy.0
                    )
                } else {
                    "(T)ravel / Enter to commit · Esc clears proposal".into()
                }
            },
        );
        vec![
            Line::from(format!(
                "Route Proposal: {} → {}",
                view.player.location_name,
                system_name(view, destination)
            )),
            Line::from(summary),
            Line::from(energy),
            Line::from(command),
        ]
    } else {
        vec![
            Line::from("No Route Proposal"),
            Line::from("Select a destination in Systems, then press F2."),
        ]
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(title)),
        area,
    );
}

fn render_trade_player(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
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
        Line::from(format!(
            "Location {}{} · Tank {}/{} E · Value {} E · Rank #{}",
            p.location_name,
            if p.traveling { " (traveling)" } else { "" },
            p.tank_energy.0,
            p.tank_capacity.0,
            p.total_energy_value.0,
            p.energy_value_rank
        )),
        Line::from(format!(
            "Cargo bay {}/{} · bay energy {} · value {} E: {}",
            p.cargo_used, p.cargo_capacity, p.bay_energy, p.cargo_energy_value.0, cargo
        )),
        Line::from(format!(
            "Sales {} E · Gain {} E · Trades {} · Runway {}",
            p.sales_energy.0,
            p.realized_energy_gain.0,
            p.transactions,
            p.runway_jumps
                .map_or_else(|| "—".into(), |value| format!("{value} jumps"))
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::bordered()
                .title("Player / Trade")
                .border_style(focused(ui, Activity::Trade)),
        ),
        area,
    );
}

fn render_intelligence_activity(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    if layout_class == LayoutClass::Regular {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(area);
        let summaries = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(columns[1]);
        render_event_table(frame, columns[0], view, ui);
        render_player_summary(frame, summaries[0], view);
        render_fleet_world_summary(frame, summaries[1], view);
    } else {
        let panes = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(52),
                Constraint::Length(6),
                Constraint::Min(6),
            ])
            .split(area);
        render_event_table(frame, panes[0], view, ui);
        render_player_summary(frame, panes[1], view);
        render_fleet_world_summary(frame, panes[2], view);
    }
}

fn render_event_table(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let visible = usize::from(area.height.saturating_sub(3)).max(1);
    let anchor_index = if view.events.is_empty() {
        0
    } else {
        ui.event_anchor
            .and_then(|anchor| {
                view.events
                    .iter()
                    .position(|event| event.sequence == anchor)
                    .or_else(|| {
                        view.events
                            .iter()
                            .position(|event| event.sequence >= anchor)
                    })
            })
            .unwrap_or(view.events.len() - 1)
    };
    let start = anchor_index.saturating_add(1).saturating_sub(visible);
    let end = if view.events.is_empty() {
        0
    } else {
        anchor_index + 1
    };
    let newer = view.events.len().saturating_sub(end);
    let range = if view.events.is_empty() {
        "0-0 / 0".into()
    } else {
        format!("{}-{} / {}", start + 1, end, view.events.len())
    };
    let newer_label = if newer > 0 || ui.newer_events_available {
        format!(" · {newer} newer events")
    } else {
        " · newest".into()
    };
    let rows = if view.events.is_empty() {
        vec![Row::new(vec![
            Cell::from("—"),
            Cell::from("No events recorded"),
        ])]
    } else {
        view.events[start..end]
            .iter()
            .map(|event| {
                Row::new(vec![
                    right_cell(event.sequence.to_string()),
                    Cell::from(event.text.clone()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Table::new(rows, [Constraint::Length(8), Constraint::Min(20)])
            .header(bold_row(["Sequence", "Event"]))
            .column_spacing(1)
            .block(
                Block::bordered()
                    .title(format!("Recent Events — {range}{newer_label}"))
                    .border_style(focused(ui, Activity::Intelligence)),
            ),
        area,
    );
}

fn render_player_summary(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView) {
    let player = &view.player;
    let cargo = if player.cargo.is_empty() {
        "empty".into()
    } else {
        player
            .cargo
            .iter()
            .map(|item| format!("{} x{}", item.good_name, item.quantity))
            .collect::<Vec<_>>()
            .join(", ")
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{} · Tank {}/{} E · Cargo {}/{}",
                player.location_name,
                player.tank_energy.0,
                player.tank_capacity.0,
                player.cargo_used,
                player.cargo_capacity
            )),
            Line::from(format!("Cargo: {cargo}")),
            Line::from(format!(
                "Value {} E · Rank #{} · Sales {} E · Gain {} E · Trades {}",
                player.total_energy_value.0,
                player.energy_value_rank,
                player.sales_energy.0,
                player.realized_energy_gain.0,
                player.transactions
            )),
        ])
        .wrap(Wrap { trim: true })
        .block(Block::bordered().title("Player Summary")),
        area,
    );
}

fn render_fleet_world_summary(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "Fleet active {} · spawns {} · retirements {}",
                view.fleet.active_npcs, view.fleet.total_spawns, view.fleet.total_retirements
            )),
            Line::from(format!(
                "Unserved opportunity {} · persistence {} ticks",
                view.fleet.normalized_unserved_opportunity, view.fleet.opportunity_persistence
            )),
            Line::from(format!(
                "World stage occupancy {:?} · transitions {}",
                view.dynamics.stage_occupancy_ticks, view.dynamics.stage_transitions
            )),
            Line::from(format!(
                "Population changes {} · milestones {}",
                view.dynamics.population_changes, view.dynamics.population_milestones
            )),
        ])
        .wrap(Wrap { trim: true })
        .block(Block::bordered().title("Fleet / World")),
        area,
    );
}

fn quote_total(quote: game_app::Energy, quantity: u32) -> Option<game_app::Energy> {
    quote
        .0
        .checked_mul(i64::from(quantity))
        .map(game_app::Energy)
}

fn total_label(quote: game_app::Energy, quantity: u32) -> String {
    quote_total(quote, quantity).map_or_else(|| "overflow".into(), |total| format!("{} E", total.0))
}

fn held_quantity(view: &ApplicationView, row: &game_app::MarketRow) -> u64 {
    view.player
        .cargo
        .iter()
        .find(|cargo| cargo.good_id == row.good_id)
        .map_or(0, |cargo| cargo.quantity)
}

fn buy_unavailable_reason(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    quantity: u32,
) -> Option<String> {
    if !view.local_trade.available {
        return Some(
            view.local_trade
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "Trading is unavailable".into()),
        );
    }
    if row.inventory < u64::from(quantity) {
        return Some(format!(
            "Buy unavailable: market has {} but Qty {quantity} was requested",
            row.inventory
        ));
    }
    if row.sell_quote.0 <= 0 {
        return Some("Buy unavailable: market has no sell quote".into());
    }
    let Some(total) = quote_total(row.sell_quote, quantity) else {
        return Some("Buy unavailable: quote total overflow".into());
    };
    if total > view.player.tank_energy {
        return Some(format!(
            "Buy unavailable: requires {} E but tank holds {} E",
            total.0, view.player.tank_energy.0
        ));
    }
    if view.player.cargo_used.saturating_add(u64::from(quantity))
        > u64::from(view.player.cargo_capacity)
    {
        return Some(format!(
            "Buy unavailable: cargo would exceed {}/{}",
            view.player.cargo_used, view.player.cargo_capacity
        ));
    }
    None
}

fn sell_unavailable_reason(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    quantity: u32,
) -> Option<String> {
    if !view.local_trade.available {
        return Some(
            view.local_trade
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "Trading is unavailable".into()),
        );
    }
    let held = held_quantity(view, row);
    if held < u64::from(quantity) {
        return Some(format!(
            "Sell unavailable: held {held}, Qty {quantity} requested"
        ));
    }
    if row.buy_quote.0 <= 0 {
        return Some("Sell unavailable: market is not buying this good".into());
    }
    quote_total(row.buy_quote, quantity)
        .is_none()
        .then_some("Sell unavailable: quote total overflow".into())
}

fn availability_label(reason: Option<&str>) -> String {
    reason.map_or_else(
        || "available".into(),
        |reason| format!("unavailable: {reason}"),
    )
}

fn system_name(view: &ApplicationView, id: &game_app::ContentId) -> String {
    if view.local_trade.system.id == *id {
        return view.local_trade.system.name.clone();
    }
    if view.inspection.system.id == *id {
        return view.inspection.system.name.clone();
    }
    if let Some(system) = view.systems.iter().find(|system| system.id == *id) {
        return system.name.clone();
    }
    if let Some(connection) = view
        .systems
        .iter()
        .flat_map(|system| &system.connections)
        .find(|connection| connection.system_id == *id)
    {
        return connection.system_name.clone();
    }
    if let Some(route) = view
        .selected_route
        .as_ref()
        .filter(|route| route.destination_id == *id)
    {
        return route.destination_name.clone();
    }
    "Unknown system".into()
}

fn route_summary_from_system(view: &ApplicationView, destination: &game_app::ContentId) -> String {
    if let Some(system) = view.systems.iter().find(|system| &system.id == destination) {
        return format!(
            "Route distance {} · {} ticks",
            system
                .route_distance_from_player
                .map_or_else(|| "unreachable".into(), |distance| format!("{distance:.1}")),
            system
                .route_ticks_from_player
                .map_or_else(|| "unreachable".into(), |ticks| ticks.to_string())
        );
    }
    if let Some(connection) = view
        .systems
        .iter()
        .flat_map(|system| &system.connections)
        .find(|connection| &connection.system_id == destination)
    {
        return format!(
            "1 jump · {:.1} distance · {} ticks",
            connection.distance, connection.travel_ticks
        );
    }
    "Route details unavailable".into()
}

fn help_text(activity: Activity) -> String {
    let contextual = match activity {
        Activity::Systems => {
            "Systems: ↑/↓ select · (O) sort column · (D) reverse · F2 propose route"
        }
        Activity::Trade => {
            "Trade: ↑/↓ good · (N) quantity · (B)uy · sell (X) · (T)ravel/Enter · Esc clear route"
        }
        Activity::Governance => {
            "Governance: ↑/↓ row · ←/→ edit · (I)nspect Systems selection · Esc governed target"
        }
        Activity::Intelligence => "Intelligence: ↑/↓ events · reaching newest resumes tail-follow",
    };
    format!(
        "{contextual}\n\nActivities: F1 Systems · F2 Trade · F3 Governance · F4 Intelligence\nGlobal: Space pause/run · (S) single step · (R) rate · (?) help · (Q) quit\nOverlays own input; Enter confirms and Esc cancels."
    )
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
        rendered_at(
            100,
            35,
            view,
            &UiState {
                input_layer: InputLayer::Detail,
                ..UiState::default()
            },
        )
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
        assert!(governed.contains("Investments — Allocation Total"));
        assert!(governed.contains("Route subsidy 10% · active"));
        for value in ["Collector", "30%", "0/4", "300 E", "ready"] {
            assert!(governed.contains(value), "missing investment value {value}");
        }

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
            required_energy: Energy(43),
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
        assert!(governance.contains("Investments — Allocation Total"));

        let intelligence = rendered_activity(&edge, Activity::Intelligence);
        assert!(intelligence.contains("Recent Events"));
        assert!(intelligence.contains("Rejected: insufficient cargo capacity"));

        for (width, height) in [(79, 30), (80, 29), (40, 10), (100, 12)] {
            let rendered = rendered_at(width, height, &base, &UiState::default());
            assert!(rendered.contains("Unsupported terminal size"));
        }

        let help = UiState {
            input_layer: InputLayer::Help,
            ..UiState::default()
        };
        assert!(rendered_at(100, 35, &edge, &help).contains("Help"));
        let quantity = UiState {
            input_layer: InputLayer::Quantity,
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
        handle_key(KeyCode::Right, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.governor.policy.operating_reserve_ticks, 4);
        ui.governance_index = 1;
        handle_key(KeyCode::Right, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.governor.policy.producer_margin_percent, 16);
        ui.governance_index = 2 + view.inspection.market.len();
        handle_key(KeyCode::Right, &mut ui, &view, &app)
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
        ui.governance_index = 2;
        handle_key(KeyCode::Right, &mut ui, &invalid, &app)
            .await
            .unwrap();
        assert!(ui.message.contains("invalid market policy"));

        app.request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let readonly = app.views.borrow().clone();
        assert!(!readonly.inspection.governor.governed);
        handle_key(KeyCode::Right, &mut ui, &readonly, &app)
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

        let compact_root = rendered_at(80, 30, &view, &UiState::default());
        assert!(!compact_root.contains("System Detail"));
        let compact_detail = rendered_at(
            80,
            30,
            &view,
            &UiState {
                input_layer: InputLayer::Detail,
                ..UiState::default()
            },
        );
        assert!(compact_detail.contains("System Detail"));
        assert!(!compact_detail.contains("Systems — Name"));

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
        let mut view = test_view();
        let mut selected = view.systems[0].clone();
        selected.id = id("core:s1");
        selected.name = "Brasshaven".into();
        selected.player_location = false;
        selected.player_governed = false;
        selected.health = EnergyHealth::Low;
        view.systems.push(selected);
        let ui = UiState {
            selected_system: Some(id("core:s1")),
            system_sort: SystemSortKey::Risk,
            sort_direction: SortDirection::Descending,
            ..UiState::default()
        };
        let rendered = rendered_at(160, 45, &view, &ui);
        assert!(rendered.contains("Systems — Risk ↓"));
        assert!(rendered.contains(">   Brasshaven"));
        assert!(rendered.contains("LOC"));
        assert!(rendered.contains("GOV"));
        assert!(rendered.contains("Energy"));
        assert!(rendered.contains("Population"));
        assert!(!rendered.contains("core:"));
    }

    #[test]
    fn semantic_cues_have_textual_fallbacks_and_shortcut_accent_styles() {
        let mut view = test_view();
        view.systems[0].health = EnergyHealth::Deficit;
        view.systems[0].brownout_stage = BrownoutStage::Emergency;
        view.systems[0].name = "A very long frontier system name that must truncate safely".into();
        let ui = UiState {
            selected_system: Some(id("core:s0")),
            ..UiState::default()
        };
        let backend = TestBackend::new(160, 45);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &view, &ui)).unwrap();
        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains(">"), "selection needs a textual marker");
        assert!(rendered.contains("WARN"), "warning needs a textual label");
        assert!(rendered.contains("Emergency"), "severity must be named");
        assert!(rendered.contains("S(o)rt"));
        assert!(rendered.contains("(D)irection"));
        assert!(!rendered.contains("core:"));
        let footer_has_accent = (43..45).any(|y| {
            (0..160).any(|x| {
                buffer
                    .cell((x, y))
                    .is_some_and(|cell| cell.fg == Color::Yellow)
            })
        });
        assert!(
            footer_has_accent,
            "shortcut characters need a consistent accent style"
        );
    }

    #[test]
    fn empty_and_extreme_views_render_safely_in_compact_and_regular_layouts() {
        let mut view = test_view();
        view.systems.clear();
        view.local_trade.market.clear();
        view.inspection.market.clear();
        view.events.clear();
        view.player.tank_energy = Energy(i64::MAX);
        view.player.tank_capacity = Energy(i64::MAX);
        view.player.cargo_used = u64::MAX;
        view.player.cargo_capacity = u32::MAX;
        for (width, height) in [(80, 30), (160, 45)] {
            for activity in [Activity::Systems, Activity::Trade, Activity::Intelligence] {
                let rendered = rendered_at(
                    width,
                    height,
                    &view,
                    &UiState {
                        activity,
                        ..UiState::default()
                    },
                );
                assert!(!rendered.is_empty());
                assert!(!rendered.contains("core:"));
            }
        }
    }

    #[test]
    fn trade_governance_and_intelligence_render_contextual_targets_in_both_layouts() {
        let view = test_view();
        for (width, height) in [(80, 30), (160, 45)] {
            let trade = rendered_at(
                width,
                height,
                &view,
                &UiState {
                    activity: Activity::Trade,
                    route_proposal: Some(id("core:s1")),
                    ..UiState::default()
                },
            );
            assert!(trade.contains("Local Market — Aster Reach"));
            assert!(trade.contains("Remote Inspection — Brasshaven (read-only)"));
            assert!(trade.contains("Qty 1"));
            assert!(trade.contains("Route Proposal"));
            assert!(trade.contains("(T)ravel"));
            assert!(!trade.contains("core:"));

            let governance = rendered_at(
                width,
                height,
                &view,
                &UiState {
                    activity: Activity::Governance,
                    ..UiState::default()
                },
            );
            assert!(governance.contains("Governance — Aster Reach"));
            assert!(governance.contains(">"));
            assert!(governance.contains("Allocation"));
            assert!(governance.contains("Total"));

            let intelligence = rendered_at(
                width,
                height,
                &view,
                &UiState {
                    activity: Activity::Intelligence,
                    ..UiState::default()
                },
            );
            assert!(intelligence.contains("Recent Events"));
            assert!(intelligence.contains("Player Summary"));
            assert!(intelligence.contains("Fleet / World"));
            assert!(intelligence.contains("/"));
        }
    }

    #[tokio::test]
    async fn travel_is_previewed_in_trade_then_explicitly_committed_and_rejection_preserves_it() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState::default();
        let mut view = app.views.borrow().clone();
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s1"));

        handle_key(KeyCode::F(2), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(ui.activity, Activity::Trade);
        assert_eq!(ui.route_proposal, Some(id("core:s1")));
        assert!(!view.player.traveling, "preview must not mutate simulation");

        handle_key(KeyCode::F(3), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s0"));
        handle_key(KeyCode::F(2), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(ui.route_proposal, Some(id("core:s1")));
        assert_eq!(
            view.selected_route
                .as_ref()
                .map(|route| &route.destination_id),
            Some(&id("core:s1"))
        );

        handle_key(KeyCode::Char('t'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert!(view.player.traveling);

        let app = game_app::spawn(test_session());
        let view = app.views.borrow().clone();
        let mut rejected = UiState {
            activity: Activity::Trade,
            route_proposal: Some(id("core:missing")),
            ..UiState::default()
        };
        handle_key(KeyCode::Char('t'), &mut rejected, &view, &app)
            .await
            .unwrap();
        assert_eq!(rejected.route_proposal, Some(id("core:missing")));
        assert!(!rejected.message.is_empty());
        app.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn governance_defaults_to_governed_and_explicit_inspection_is_read_only() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState::default();
        let mut view = app.views.borrow().clone();
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(ui.selected_system, Some(id("core:s1")));

        handle_key(KeyCode::F(3), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.system.id, id("core:s0"));
        assert!(view.inspection.governor.governed);

        handle_key(KeyCode::Char('i'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.system.id, id("core:s1"));
        assert!(!view.inspection.governor.governed);
        assert_eq!(ui.governance_inspection, Some(id("core:s1")));

        handle_key(KeyCode::Esc, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.inspection.system.id, id("core:s0"));
        assert!(view.inspection.governor.governed);
        app.shutdown().await.unwrap();
    }

    #[test]
    fn intelligence_scroll_preserves_sequence_anchor_and_reports_newer_events() {
        let mut ui = UiState {
            activity: Activity::Intelligence,
            ..UiState::default()
        };
        let first = (10..=12)
            .map(|sequence| PresentationEvent {
                sequence,
                text: format!("Event {sequence}"),
            })
            .collect::<Vec<_>>();
        ui.reconcile_events(&first);
        assert_eq!(ui.event_anchor, Some(12));
        ui.scroll_events(&first, -1);
        assert_eq!(ui.event_anchor, Some(11));

        let appended = (10..=14)
            .map(|sequence| PresentationEvent {
                sequence,
                text: format!("Event {sequence}"),
            })
            .collect::<Vec<_>>();
        ui.reconcile_events(&appended);
        assert_eq!(ui.event_anchor, Some(11));
        assert!(ui.newer_events_available);

        let rolled = (13..=15)
            .map(|sequence| PresentationEvent {
                sequence,
                text: format!("Event {sequence}"),
            })
            .collect::<Vec<_>>();
        ui.reconcile_events(&rolled);
        assert_eq!(ui.event_anchor, Some(13));
        assert!(ui.newer_events_available);
        ui.scroll_events(&rolled, 99);
        assert_eq!(ui.event_anchor, Some(15));
        assert!(!ui.newer_events_available);
    }

    #[test]
    fn systems_table_keeps_the_stable_selected_row_visible_in_compact_layout() {
        let mut view = test_view();
        let template = view.systems[0].clone();
        for index in 1..20 {
            let mut system = template.clone();
            system.id = id(&format!("core:s{index}"));
            system.name = format!("System {index:02}");
            system.player_location = false;
            system.player_governed = false;
            view.systems.push(system);
        }
        let rendered = rendered_at(
            80,
            30,
            &view,
            &UiState {
                selected_system: Some(id("core:s19")),
                ..UiState::default()
            },
        );
        assert!(rendered.contains("System 19"));
        assert!(rendered.contains(">"));
    }

    #[tokio::test]
    async fn obsolete_hidden_target_shortcuts_are_inert_in_runtime_dispatch() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState {
            activity: Activity::Governance,
            ..UiState::default()
        };
        let before = app.views.borrow().clone();
        for key in [']', '[', ',', '.', 'I', '-', '+', '='] {
            handle_key(KeyCode::Char(key), &mut ui, &before, &app)
                .await
                .unwrap();
        }
        let after = app.views.borrow().clone();
        assert_eq!(
            after.inspection.governor.policy,
            before.inspection.governor.policy
        );
        assert_eq!(
            after.inspection.governor.investment_policy,
            before.inspection.governor.investment_policy
        );
        app.shutdown().await.unwrap();
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
        assert_eq!(ui.input_layer, InputLayer::Help);
        handle_key(KeyCode::Esc, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.input_layer, InputLayer::Root);
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s1"));
        assert_eq!(view.tick, 1, "UI navigation must not advance simulation");
        handle_key(KeyCode::F(2), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('t'), &mut ui, &view, &app)
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
