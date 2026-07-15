//! Ratatui input/render adapter. This crate never accesses the ECS world.

pub mod input;
pub mod state;

pub use input::{InputAction, route_key};
pub use state::{
    Activity, InputLayer, LayoutClass, SortDirection, SystemDetailKind, SystemOrderItem,
    SystemSortKey, TradeOrderSide, TradeRegion, UiState, classify_layout, order_systems,
};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use game_app::{
    AppHandle, AppRequest, ApplicationView, InvestmentKind, InvestmentStatus, RunControl, RunState,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use std::io::stdout;

const ENCYCLOPEDIA_PAGE_LINES: u16 = 8;

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
                    ui.message = "Preset quantity unchanged".into();
                }
                InputLayer::Order => {
                    ui.quantity_input = None;
                    ui.trade_order_side = None;
                    ui.trade_order_good = None;
                    ui.message = "Trade order cancelled".into();
                }
                InputLayer::Help | InputLayer::Detail | InputLayer::Root => {}
            }
            ui.input_layer = InputLayer::Root;
        }
        InputAction::QuantityDigit(digit) => {
            if let Some(input) = &mut ui.quantity_input
                && input.len() < 10
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
            ui.message = format!("Preset quantity set to {quantity}");
        }
        InputAction::UseOrderMaximum => {
            if let (Some(side), Some(row)) = (ui.trade_order_side, trade_order_row(view, ui)) {
                let limit = trade_order_limit(view, row, side);
                if limit.maximum > 0 {
                    ui.quantity_input = Some(limit.maximum.to_string());
                } else {
                    ui.message = limit.reason;
                }
            }
        }
        InputAction::ConfirmOrder => {
            let input = ui.quantity_input.as_deref().unwrap_or_default();
            let Ok(requested) = input.parse::<u32>() else {
                ui.message = if input.is_empty() {
                    "Enter a quantity greater than zero".into()
                } else {
                    "Quantity is outside the supported range".into()
                };
                return Ok(false);
            };
            let Some(side) = ui.trade_order_side else {
                ui.input_layer = InputLayer::Root;
                return Ok(false);
            };
            let Some(row) = trade_order_row(view, ui) else {
                ui.message = "The selected order good is no longer available".into();
                return Ok(false);
            };
            let limit = trade_order_limit(view, row, side);
            if requested == 0 {
                ui.message = "Enter a quantity greater than zero".into();
            } else if requested > limit.maximum {
                ui.message = format!(
                    "Requested {requested}; maximum is {} ({})",
                    limit.maximum, limit.reason
                );
            } else {
                let request = match side {
                    TradeOrderSide::Buy => AppRequest::Buy {
                        good: row.good_id.clone(),
                        quantity: requested,
                    },
                    TradeOrderSide::Sell => AppRequest::Sell {
                        good: row.good_id.clone(),
                        quantity: requested,
                    },
                };
                match app.request(request).await {
                    Ok(()) => {
                        ui.message = format!(
                            "{} {} ×{requested}",
                            match side {
                                TradeOrderSide::Buy => "Bought",
                                TradeOrderSide::Sell => "Sold",
                            },
                            row.name
                        );
                        ui.quantity_input = None;
                        ui.trade_order_side = None;
                        ui.trade_order_good = None;
                        ui.input_layer = InputLayer::Root;
                    }
                    Err(error) => ui.message = error.to_string(),
                }
            }
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
            if let Some(proposal) = ui.route_proposal.clone() {
                ui.selected_trade_destination = Some(proposal);
            }
            let destinations = trade_destination_ids(view, ui);
            ui.reconcile_trade_destination(&destinations);
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
        InputAction::Switch(Activity::Encyclopedia) => {
            ui.activity = Activity::Encyclopedia;
            clamp_encyclopedia_selection(ui, view);
        }
        InputAction::ToggleHelp => ui.input_layer = InputLayer::Help,
        InputAction::ToggleRun => {
            let state = if view.run_state == RunState::Paused {
                RunControl::Running
            } else {
                RunControl::Paused
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
            } else if ui.activity == Activity::Trade && ui.trade_region == TradeRegion::Destinations
            {
                activate_trade_destination(ui, view, app).await;
            }
        }
        InputAction::PageUp => {
            ui.encyclopedia_article_scroll = ui
                .encyclopedia_article_scroll
                .saturating_sub(ENCYCLOPEDIA_PAGE_LINES);
        }
        InputAction::PageDown => {
            ui.encyclopedia_article_scroll = ui
                .encyclopedia_article_scroll
                .saturating_add(ENCYCLOPEDIA_PAGE_LINES);
        }
        InputAction::Sort => {
            ui.system_sort = ui.system_sort.next();
            sync_system_row(ui, view);
        }
        InputAction::ToggleSortDirection => {
            ui.sort_direction = ui.sort_direction.toggled();
            sync_system_row(ui, view);
        }
        InputAction::OpenDetail => {
            ui.system_detail = SystemDetailKind::Overview;
            ui.input_layer = InputLayer::Detail;
        }
        InputAction::OpenMarketDetail => {
            ui.system_detail = SystemDetailKind::Market;
            ui.input_layer = InputLayer::Detail;
        }
        InputAction::NextSection | InputAction::PreviousSection => {
            let delta = if action == InputAction::NextSection {
                1
            } else {
                -1
            };
            match ui.activity {
                Activity::Governance => jump_governance_section(ui, view, delta),
                Activity::Trade => {
                    ui.trade_region = match ui.trade_region {
                        TradeRegion::Goods => TradeRegion::Destinations,
                        TradeRegion::Destinations => TradeRegion::Goods,
                    };
                    let destinations = trade_destination_ids(view, ui);
                    ui.reconcile_trade_destination(&destinations);
                    if ui.trade_region == TradeRegion::Destinations {
                        activate_trade_destination(ui, view, app).await;
                    }
                }
                Activity::Encyclopedia => jump_encyclopedia_section(ui, view, delta),
                Activity::Systems | Activity::Intelligence => {}
            }
        }
        InputAction::OpenQuantity => {
            if ordinary_market_rows(view).is_empty() {
                ui.message = "No ordinary local market goods are available".into();
            } else {
                ui.quantity_input = Some(String::new());
                ui.input_layer = InputLayer::Quantity;
            }
        }
        InputAction::OpenBuyOrder | InputAction::OpenSellOrder => {
            if let Some(row) = selected_ordinary_market_row(view, ui) {
                ui.trade_order_side = Some(if action == InputAction::OpenBuyOrder {
                    TradeOrderSide::Buy
                } else {
                    TradeOrderSide::Sell
                });
                ui.trade_order_good = Some(row.good_id.clone());
                ui.quantity_input = Some(String::new());
                ui.input_layer = InputLayer::Order;
                ui.message.clear();
            } else {
                ui.message = "No ordinary local market goods are available".into();
            }
        }
        InputAction::Buy => {
            if let Some(row) = selected_ordinary_market_row(view, ui) {
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
                ui.message = "No ordinary local market goods are available".into();
            }
        }
        InputAction::Sell => {
            if let Some(row) = selected_ordinary_market_row(view, ui) {
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
                ui.message = "No ordinary local market goods are available".into();
            }
        }
        InputAction::AcceptEnergyContract => {
            let Some(destination) = ui.selected_trade_destination.as_ref() else {
                ui.message =
                    "Select a Trade destination before accepting an Energy contract".into();
                return Ok(false);
            };
            let Some(opportunity) =
                view.energy_logistics
                    .opportunities
                    .iter()
                    .find(|opportunity| {
                        &opportunity.destination.id == destination && opportunity.blocker.is_none()
                    })
            else {
                ui.message =
                    "No viable Energy opportunity is available for the selected destination".into();
                return Ok(false);
            };
            match app
                .request(AppRequest::AcceptEnergyContract {
                    source: opportunity.source.id.clone(),
                    destination: opportunity.destination.id.clone(),
                    gross_payload: opportunity.maximum_gross_payload,
                })
                .await
            {
                Ok(()) => {
                    ui.message = format!(
                        "Submitted Energy contract request to {} at gross payload {} E; step to resolve",
                        opportunity.destination.name,
                        format_energy(opportunity.maximum_gross_payload)
                    );
                }
                Err(error) => ui.message = error.to_string(),
            }
        }
        InputAction::CancelEnergyContract => {
            let Some(contract) = view
                .energy_logistics
                .contracts
                .iter()
                .find(|contract| contract.player_owned)
            else {
                ui.message = "No player-owned Energy contract is active".into();
                return Ok(false);
            };
            match app
                .request(AppRequest::CancelEnergyContract {
                    contract_id: contract.id,
                })
                .await
            {
                Ok(()) => {
                    ui.message = format!("Cancelled Energy contract #{}", contract.id.get());
                }
                Err(error) => ui.message = error.to_string(),
            }
        }
        InputAction::TransferOwnedBulkToTank => {
            let amount = game_app::Energy(i64::from(ui.trade_quantity));
            match app
                .request(AppRequest::TransferOwnedBulkToTank { amount })
                .await
            {
                Ok(()) => {
                    ui.message = format!(
                        "Transferred {} E from owned bulk to tank",
                        format_energy(amount)
                    );
                }
                Err(error) => ui.message = error.to_string(),
            }
        }
        InputAction::DepositOwnedBulkEnergy => {
            let amount = game_app::Energy(i64::from(ui.trade_quantity));
            match app
                .request(AppRequest::DepositOwnedBulkEnergy { amount })
                .await
            {
                Ok(()) => {
                    ui.message = format!(
                        "Deposited {} E from owned bulk into the current market",
                        format_energy(amount)
                    );
                }
                Err(error) => ui.message = error.to_string(),
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
        InputAction::TravelUntilArrival => {
            let destination = if view.player.traveling {
                view.selected_route
                    .as_ref()
                    .map(|route| route.destination_id.clone())
            } else {
                ui.route_proposal.clone()
            };
            if let Some(destination) = destination {
                match app
                    .request(AppRequest::TravelUntilArrival {
                        destination: destination.clone(),
                    })
                    .await
                {
                    Ok(()) => {
                        ui.message = format!(
                            "Running until arrival at {}",
                            system_name(view, &destination)
                        );
                    }
                    Err(error) => ui.message = error.to_string(),
                }
            } else {
                ui.message = "No active journey or route proposal".into();
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

    let target_index = row - 2;
    if let Some(market) = view.inspection.market.get(target_index) {
        let next = if delta > 0 {
            market.authored_target.saturating_add(1)
        } else {
            market.authored_target.saturating_sub(1).max(1)
        };
        if next == market.authored_target {
            ui.message = "Market target is already at its minimum of 1".into();
            return;
        }
        match app
            .request(AppRequest::SetMarketTarget {
                system,
                good: market.good_id.clone(),
                target: next,
            })
            .await
        {
            Ok(()) => ui.message = format!("{} target updated to {next}", market.name),
            Err(error) => ui.message = error.to_string(),
        }
        return;
    }

    let import_index = target_index - view.inspection.market.len();
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
            ui.system_index = wrapped_shifted(current, ordered.len(), delta);
            ui.selected_system = ordered.get(ui.system_index).map(|system| system.id.clone());
        }
        Activity::Governance => {
            let policy_rows = 2;
            let market_rows = view.inspection.market.len();
            let investment_start = policy_rows + market_rows.saturating_mul(2);
            let total = investment_start + view.inspection.governor.investments.len();
            ui.governance_index = shifted(ui.governance_index, total, delta);
            if ui.governance_index >= investment_start {
                ui.investment_index = ui.governance_index - investment_start;
            }
        }
        Activity::Trade => match ui.trade_region {
            TradeRegion::Goods => move_ordinary_market_selection(ui, view, delta),
            TradeRegion::Destinations => {
                let destinations = trade_destination_ids(view, ui);
                ui.reconcile_trade_destination(&destinations);
                ui.trade_destination_index =
                    shifted(ui.trade_destination_index, destinations.len(), delta);
                ui.selected_trade_destination =
                    destinations.get(ui.trade_destination_index).cloned();
            }
        },
        Activity::Intelligence => ui.scroll_events(&view.events, delta),
        Activity::Encyclopedia => {
            let article_count = view
                .encyclopedia
                .sections
                .get(ui.encyclopedia_section_index)
                .map_or(0, |section| section.articles.len());
            let previous = ui.encyclopedia_article_index;
            ui.encyclopedia_article_index =
                shifted(ui.encyclopedia_article_index, article_count, delta);
            if ui.encyclopedia_article_index != previous {
                ui.encyclopedia_article_scroll = 0;
            }
        }
    }
}

async fn activate_trade_destination(ui: &mut UiState, view: &ApplicationView, app: &AppHandle) {
    let Some(destination) = ui.selected_trade_destination.clone() else {
        ui.message = "No comparison destinations are available for the selected good".into();
        return;
    };
    let Some(comparison) = view
        .trade_markets
        .iter()
        .find(|market| market.system.id == destination)
    else {
        ui.message = "Selected comparison destination is no longer available".into();
        return;
    };
    if view.player.traveling
        || comparison.availability == game_app::TradeDestinationAvailability::Traveling
    {
        ui.message = "Destination preview is locked while the player is in transit".into();
        return;
    }
    if comparison.availability != game_app::TradeDestinationAvailability::Available
        || comparison.route.is_none()
    {
        ui.message = comparison
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| "No route is available to the selected destination".into());
        return;
    }
    match app
        .request(AppRequest::SelectSystem(destination.clone()))
        .await
    {
        Ok(()) => {
            ui.route_proposal = Some(destination.clone());
            ui.message = format!("Comparing destination {}", system_name(view, &destination));
        }
        Err(error) => ui.message = error.to_string(),
    }
}

fn trade_destination_ids(view: &ApplicationView, ui: &UiState) -> Vec<game_app::ContentId> {
    if selected_ordinary_market_row(view, ui).is_none() {
        return Vec::new();
    }
    view.trade_markets
        .iter()
        .filter(|market| !market.local)
        .map(|market| market.system.id.clone())
        .collect()
}

fn shifted(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        0
    } else {
        current.saturating_add_signed(delta).min(len - 1)
    }
}

fn wrapped_shifted(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    ((current as isize + delta).rem_euclid(len as isize)) as usize
}

fn jump_encyclopedia_section(ui: &mut UiState, view: &ApplicationView, delta: isize) {
    ui.encyclopedia_section_index = wrapped_shifted(
        ui.encyclopedia_section_index,
        view.encyclopedia.sections.len(),
        delta,
    );
    ui.encyclopedia_article_index = 0;
    ui.encyclopedia_article_scroll = 0;
}

fn clamp_encyclopedia_selection(ui: &mut UiState, view: &ApplicationView) {
    let previous = (ui.encyclopedia_section_index, ui.encyclopedia_article_index);
    ui.encyclopedia_section_index = ui
        .encyclopedia_section_index
        .min(view.encyclopedia.sections.len().saturating_sub(1));
    let article_count = view
        .encyclopedia
        .sections
        .get(ui.encyclopedia_section_index)
        .map_or(0, |section| section.articles.len());
    ui.encyclopedia_article_index = ui
        .encyclopedia_article_index
        .min(article_count.saturating_sub(1));
    if previous != (ui.encyclopedia_section_index, ui.encyclopedia_article_index) {
        ui.encyclopedia_article_scroll = 0;
    }
}

fn jump_governance_section(ui: &mut UiState, view: &ApplicationView, delta: isize) {
    let mut starts = vec![0];
    if !view.inspection.market.is_empty() {
        starts.push(2);
        starts.push(2 + view.inspection.market.len());
    }
    if !view.inspection.governor.investments.is_empty() {
        starts.push(2 + view.inspection.market.len().saturating_mul(2));
    }
    let current = starts
        .iter()
        .rposition(|start| *start <= ui.governance_index)
        .unwrap_or(0);
    let section = wrapped_shifted(current, starts.len(), delta);
    ui.governance_index = starts[section];
    if ui.governance_index >= 2 + view.inspection.market.len().saturating_mul(2) {
        ui.investment_index = 0;
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
    if view
        .local_trade
        .market
        .get(ui.market_index)
        .is_none_or(|row| row.good_id.as_str() == ENERGY_GOOD_ID)
    {
        ui.market_index = ordinary_market_indices(view).first().copied().unwrap_or(0);
    }
    let destinations = trade_destination_ids(view, ui);
    ui.reconcile_trade_destination(&destinations);
    clamp_encyclopedia_selection(ui, view);
    let governance_rows = 2
        + view.inspection.market.len().saturating_mul(2)
        + view.inspection.governor.investments.len();
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
        Activity::Encyclopedia => {
            render_encyclopedia_activity(frame, shell[2], view, ui, layout_class);
        }
    }
    render_footer(frame, shell[3], view, ui);

    if ui.input_layer == InputLayer::Quantity {
        let input = ui.quantity_input.as_deref().unwrap_or_default();
        let popup = centered_rect(54, 8, area);
        let (good, buy_total, sell_total) = selected_ordinary_market_row(view, ui).map_or_else(
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
            Paragraph::new(vec![
                Line::from(format!("Good: {good}")),
                Line::from(format!("Quantity: {input}_")),
                Line::from(format!("Buy total: {buy_total} · Sell total: {sell_total}")),
                Line::from(vec![
                    shortcut_span("Enter"),
                    Span::raw(" confirm · "),
                    shortcut_span("Esc"),
                    Span::raw(" cancel"),
                ]),
            ])
            .block(Block::bordered().title("Reusable Trade Quantity")),
            popup,
        );
    } else if ui.input_layer == InputLayer::Order {
        let popup = centered_rect(72, 12, area);
        let order_input = ui.quantity_input.as_deref().unwrap_or_default();
        let parsed_request = order_input.parse::<u32>().ok();
        let requested = parsed_request.unwrap_or(0);
        let side = ui.trade_order_side.unwrap_or(TradeOrderSide::Buy);
        let lines = trade_order_row(view, ui).map_or_else(
            || {
                vec![Line::from(
                    "The selected order good is no longer available.",
                )]
            },
            |row| {
                let limit = trade_order_limit(view, row, side);
                let projected = requested.min(limit.maximum);
                let (unit_price, total, projected_total, tank_after, cargo_after) = match side {
                    TradeOrderSide::Buy => (
                        row.sell_quote,
                        total_label(row.sell_quote, requested),
                        total_label(row.sell_quote, projected),
                        view.player.tank_energy.0.saturating_sub(
                            quote_total(row.sell_quote, projected).map_or(0, |value| value.0),
                        ),
                        view.player.cargo_used.saturating_add(u64::from(projected)),
                    ),
                    TradeOrderSide::Sell => (
                        row.buy_quote,
                        total_label(row.buy_quote, requested),
                        total_label(row.buy_quote, projected),
                        view.player.tank_energy.0.saturating_add(
                            quote_total(row.buy_quote, projected).map_or(0, |value| value.0),
                        ),
                        view.player.cargo_used.saturating_sub(u64::from(projected)),
                    ),
                };
                let status = if parsed_request.is_none() && !order_input.is_empty() {
                    "Quantity is outside the supported range.".into()
                } else if requested == 0 {
                    "Enter a quantity, or press M to use the maximum.".into()
                } else if requested <= limit.maximum {
                    "Ready to confirm this one-transaction order.".into()
                } else {
                    format!(
                        "Requested {requested} exceeds the maximum by {}.",
                        requested.saturating_sub(limit.maximum)
                    )
                };
                vec![
                    Line::from(format!(
                        "{} · {} price {} E/unit",
                        row.name,
                        match side {
                            TradeOrderSide::Buy => "Buy",
                            TradeOrderSide::Sell => "Sell",
                        },
                        unit_price.0
                    )),
                    Line::from(format!(
                        "Requested: {}_ · Reusable preset: {}",
                        ui.quantity_input.as_deref().unwrap_or_default(),
                        ui.trade_quantity
                    )),
                    Line::from(format!("Maximum now: {} ({})", limit.maximum, limit.reason)),
                    Line::from(format!("Order total: {total}")),
                    Line::from(format!(
                        "{}: {projected_total} · Tank {}→{} E · Cargo {}→{}/{}",
                        if requested <= limit.maximum {
                            "After order"
                        } else {
                            "At maximum"
                        },
                        view.player.tank_energy.0,
                        tank_after,
                        view.player.cargo_used,
                        cargo_after,
                        view.player.cargo_capacity
                    )),
                    Line::from(status),
                    Line::from(vec![
                        Span::raw("("),
                        shortcut_span("M"),
                        Span::raw(") use maximum · "),
                        shortcut_span("Enter"),
                        Span::raw(" confirm · "),
                        shortcut_span("Esc"),
                        Span::raw(" cancel"),
                    ]),
                ]
            },
        );
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title(match side {
                    TradeOrderSide::Buy => "One-Transaction Buy Order",
                    TradeOrderSide::Sell => "One-Transaction Sell Order",
                })),
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
    let compact = area.width < 120;
    let entries = [
        (Activity::Systems, "F1", "Systems"),
        (Activity::Trade, "F2", "Trade"),
        (
            Activity::Governance,
            "F3",
            if compact { "Gov" } else { "Governance" },
        ),
        (
            Activity::Intelligence,
            "F4",
            if compact { "Intel" } else { "Intelligence" },
        ),
        (
            Activity::Encyclopedia,
            "F5",
            if compact { "Encycl." } else { "Encyclopedia" },
        ),
    ];
    let mut spans = Vec::new();
    for (activity, key, label) in entries {
        let is_active = activity == active;
        spans.push(Span::styled(
            if is_active { " * " } else { "   " },
            if is_active {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            },
        ));
        spans.push(shortcut_span(key));
        spans.push(Span::styled(
            format!(" {label}  "),
            if is_active {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            },
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_global_status(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView) {
    let status = format!(
        "{} · Tick {} · Rate {} · Location {} · Tank {}/{} E",
        match view.run_state {
            RunState::Paused => "PAUSED",
            RunState::Running => "RUNNING",
            RunState::RunningUntilArrival => "RUNNING TO ARRIVAL",
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
                spans.push(Span::raw(match ui.system_detail {
                    SystemDetailKind::Overview => "System overview · ",
                    SystemDetailKind::Market => "Market inspection · ",
                }));
                spans.push(shortcut_span("Esc"));
                spans.push(Span::raw(" return"));
            } else {
                spans.push(shortcut_span("↑/↓"));
                spans.push(Span::raw(" Select · "));
                spans.push(shortcut_span("Enter"));
                spans.push(Span::raw(" overview · ("));
                spans.push(shortcut_span("M"));
                spans.push(Span::raw(")arket · S("));
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
                spans.push(shortcut_span("F2"));
                if view.player.traveling {
                    spans.push(Span::raw(" route disabled: in transit"));
                } else if route_available {
                    spans.push(Span::raw(" propose selected route"));
                } else {
                    spans.push(Span::raw(" route disabled: unreachable/already here"));
                }
            }
        }
        Activity::Trade => {
            spans.push(shortcut_span("Tab/Shift-Tab"));
            spans.push(Span::raw(" Region · "));
            spans.push(shortcut_span("↑/↓"));
            spans.push(Span::raw(match ui.trade_region {
                TradeRegion::Goods => " Good · (",
                TradeRegion::Destinations => " Destination · (",
            }));
            spans.push(shortcut_span("N"));
            spans.push(Span::raw(format!(") Qty {} · ", ui.trade_quantity)));
            spans.push(Span::raw("("));
            spans.push(shortcut_span("e"));
            spans.push(Span::raw(") Energy accept · ("));
            spans.push(shortcut_span("x"));
            spans.push(Span::raw(") cancel · ("));
            spans.push(shortcut_span("f"));
            spans.push(Span::raw(") bulk→tank · ("));
            spans.push(shortcut_span("p"));
            spans.push(Span::raw(") bulk→market · "));
            if let Some(row) = selected_ordinary_market_row(view, ui) {
                let buy_reason = buy_unavailable_reason(view, row, ui.trade_quantity);
                let sell_reason = sell_unavailable_reason(view, row, ui.trade_quantity);
                spans.push(Span::raw("("));
                spans.push(shortcut_span("b"));
                spans.push(Span::raw(buy_reason.map_or_else(
                    || ") quick buy · ".into(),
                    |reason| format!(") buy disabled: {} · ", action_reason(&reason)),
                )));
                spans.push(shortcut_span("Shift-B"));
                spans.push(Span::raw(" buy order · ("));
                spans.push(shortcut_span("s"));
                spans.push(Span::raw(sell_reason.map_or_else(
                    || ") quick sell · ".into(),
                    |reason| format!(") sell disabled: {} · ", action_reason(&reason)),
                )));
                spans.push(shortcut_span("Shift-S"));
                spans.push(Span::raw(" sell order · "));
            } else {
                spans.push(Span::raw("("));
                spans.push(shortcut_span("B"));
                spans.push(Span::raw(")uy / ("));
                spans.push(shortcut_span("S"));
                spans.push(Span::raw(")ell disabled: no good · "));
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
                    spans.push(Span::raw(")ravel · "));
                    spans.push(shortcut_span("Esc"));
                    spans.push(Span::raw(" clear route"));
                }
            } else {
                spans.push(Span::raw(")ravel disabled: route details unavailable"));
            }
            spans.push(Span::raw(" · ("));
            spans.push(shortcut_span("g"));
            if view.player.traveling || matching_route.is_some() {
                spans.push(Span::raw(") run to arrival"));
            } else {
                spans.push(Span::raw(") run to arrival disabled"));
            }
        }
        Activity::Governance => {
            spans.push(shortcut_span("↑/↓"));
            spans.push(Span::raw(" Row · "));
            spans.push(shortcut_span("Tab/Shift-Tab"));
            spans.push(Span::raw(" Section · "));
            if view.inspection.governor.governed && ui.governance_inspection.is_none() {
                spans.push(shortcut_span("←/→"));
                spans.push(Span::raw(" Edit · "));
            } else {
                spans.push(Span::raw("Edit disabled: read-only · "));
            }
            spans.push(Span::raw("("));
            spans.push(shortcut_span("I"));
            spans.push(Span::raw(")nspect Systems selection · "));
            spans.push(shortcut_span("Esc"));
            spans.push(Span::raw(" governed target"));
        }
        Activity::Intelligence => {
            spans.push(shortcut_span("↑/↓"));
            spans.push(Span::raw(" Scroll events · newest resumes tail-follow"));
        }
        Activity::Encyclopedia => {
            spans.push(shortcut_span("Tab/Shift-Tab"));
            spans.push(Span::raw(" Section · "));
            spans.push(shortcut_span("↑/↓"));
            spans.push(Span::raw(" Article · "));
            spans.push(shortcut_span("PgUp/PgDn"));
            spans.push(Span::raw(" Scroll article"));
        }
    }
    spans.push(Span::raw(" · "));
    spans.push(shortcut_span("Space"));
    spans.push(Span::raw(match view.run_state {
        RunState::Paused => " run · ",
        RunState::Running => " pause · ",
        RunState::RunningUntilArrival => " pause/cancel auto-arrival · ",
    }));
    spans.push(shortcut_span("."));
    spans.push(Span::raw(" step (paused) · "));
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

fn quick_action_status(reason: Option<&str>) -> String {
    let Some(reason) = reason else {
        return "ready".into();
    };
    let detail = [
        "market stock",
        "tank Energy",
        "cargo capacity",
        "units held",
        "market quote",
        "market funding",
        "tank capacity",
        "market Energy storage",
        "transaction quantity limit",
    ]
    .into_iter()
    .find(|label| reason.contains(label))
    .unwrap_or("trading unavailable");
    format!("blocked: {detail}")
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

fn system_risk(system: &game_app::SystemListItem) -> u8 {
    match (system.health, system.brownout_stage.label()) {
        (game_app::EnergyHealth::Deficit, _) | (_, "Starvation") => 3,
        (game_app::EnergyHealth::Low, _) | (_, "Emergency") => 2,
        (_, "Throttled") => 1,
        _ => 0,
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
            let risk = system_risk(system);
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
    if ui.input_layer == InputLayer::Detail {
        match ui.system_detail {
            SystemDetailKind::Overview => {
                render_system_inspector(frame, area, view, ui, layout_class)
            }
            SystemDetailKind::Market => render_system_market(frame, area, view, ui),
        }
        return;
    }
    let panes = match layout_class {
        LayoutClass::Regular => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(area),
        LayoutClass::Compact => {
            render_systems_table(frame, area, view, ui);
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
            if ordered_system.risk >= 2 {
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
            } else if ordered_system.risk >= 2 {
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
        Cell::from(if area.width >= 90 {
            "LOC/GOV/WARN"
        } else {
            "Flags"
        }),
        right_cell("Energy"),
        right_cell("Runway"),
        right_cell("Population"),
        right_cell("Route"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));
    let widths = if area.width >= 90 {
        [
            Constraint::Length(3),
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(6),
        ]
    } else {
        [
            Constraint::Length(3),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(18),
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

fn governance_control_cell(editable: bool, suffix: &'static str) -> Cell<'static> {
    if editable {
        Cell::from(Line::from(vec![shortcut_span("←/→"), Span::raw(suffix)]))
    } else {
        Cell::from("read-only")
    }
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
        (system_risk(system) >= 2).then_some("WARNING"),
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
    if system.production.is_empty() {
        lines.push(Line::from(
            "Configured production capability: no goods produced",
        ));
    } else {
        lines.push(Line::from("Configured production capability:"));
        lines.extend(system.production.iter().map(|output| {
            let mut methods = Vec::new();
            if output.source_quantity_per_tick > 0 {
                methods.push(format!(
                    "source base {}/tick",
                    output.source_quantity_per_tick
                ));
            }
            methods.extend(
                output.recipes.iter().map(|recipe| {
                    format!("{} {}/run", recipe.recipe_name, recipe.quantity_per_run)
                }),
            );
            Line::from(format!("• {} — {}", output.good_name, methods.join("; ")))
        }));
    }
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

fn render_system_market(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let ordered = order_systems(&system_order_items(view), ui.system_sort, ui.sort_direction);
    let selected = selected_system_id(view, ui, &ordered);
    let Some(system) = selected
        .as_ref()
        .and_then(|selected| view.systems.iter().find(|system| &system.id == selected))
    else {
        frame.render_widget(
            Paragraph::new("No system selected")
                .block(Block::bordered().title("Market Inspection")),
            area,
        );
        return;
    };
    let remote = system.id != view.player.location;
    let title = if remote {
        format!("Remote Market — {} (read-only)", system.name)
    } else {
        format!("Local Market Inspection — {} (read-only)", system.name)
    };
    if view.inspection.system.id != system.id {
        frame.render_widget(
            Paragraph::new("Market details are not available for the selected system yet")
                .block(Block::bordered().title(title)),
            area,
        );
        return;
    }
    let mut rows = view
        .inspection
        .market
        .iter()
        .filter(|row| row.good_id.as_str() != ENERGY_GOOD_ID)
        .map(|row| {
            Row::new(vec![
                Cell::from(row.name.clone()),
                right_cell(row.inventory.to_string()),
                right_cell(row.target.to_string()),
                right_cell(format!("{} E", row.buy_quote.0)),
                right_cell(format!("{} E", row.sell_quote.0)),
            ])
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("No market goods available")]));
    }
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Min(18),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(14),
                Constraint::Length(14),
            ],
        )
        .header(bold_row([
            "Good",
            "Stock",
            "Target",
            "Market buys",
            "Market sells",
        ]))
        .column_spacing(1)
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
            Constraint::Length(5),
            Constraint::Min(7),
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
            governance_control_cell(editable, " edit"),
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

    let target_capacity = usize::from(panes[2].height.saturating_sub(3)).max(1);
    let target_selected = ui
        .governance_index
        .checked_sub(2)
        .filter(|index| *index < view.inspection.market.len())
        .unwrap_or(0);
    let (target_start, target_end) = viewport(
        view.inspection.market.len(),
        target_selected,
        target_capacity,
    );
    let mut target_rows = view.inspection.market[target_start..target_end]
        .iter()
        .enumerate()
        .map(|(offset, market)| {
            let index = target_start + offset;
            let selected = ui.governance_index == index + 2;
            Row::new(vec![
                Cell::from(if selected { ">" } else { " " }),
                Cell::from(market.name.clone()),
                right_cell(market.inventory.to_string()),
                right_cell(market.authored_target.to_string()),
                right_cell(market.target.to_string()),
                governance_control_cell(editable, " ±1"),
            ])
            .style(selected_style(selected))
        })
        .collect::<Vec<_>>();
    if target_rows.is_empty() {
        target_rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No market targets"),
        ]));
    }
    frame.render_widget(
        Table::new(
            target_rows,
            [
                Constraint::Length(1),
                Constraint::Percentage(30),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(9),
                Constraint::Min(9),
            ],
        )
        .header(bold_row([
            "",
            "Good",
            "Stock",
            "Base",
            "Effective",
            "Control",
        ]))
        .column_spacing(1)
        .block(Block::bordered().title(format!(
            "Market Targets · {}",
            viewport_label(target_start, target_end, view.inspection.market.len())
        ))),
        panes[2],
    );

    let import_offset = 2 + view.inspection.market.len();
    let import_capacity = usize::from(panes[3].height.saturating_sub(3)).max(1);
    let import_selected = ui
        .governance_index
        .checked_sub(import_offset)
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
            let selected = ui.governance_index == index + import_offset;
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
                governance_control_cell(editable, " ±10%"),
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
        panes[3],
    );

    let investment_start = 2 + view.inspection.market.len().saturating_mul(2);
    let total = governor
        .investments
        .iter()
        .fold(0_u32, |total, investment| {
            total.saturating_add(investment.allocation_percent)
        });
    let investment_capacity = usize::from(panes[4].height.saturating_sub(3)).max(1);
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
            panes[4],
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
            panes[4],
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
    if has_energy_logistics(view) {
        if layout_class == LayoutClass::Regular {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(19), Constraint::Length(16)])
                .split(area);
            render_regular_trade_core(frame, panes[0], view, ui);
            render_energy_logistics(frame, panes[1], view, ui, layout_class);
        } else {
            // At the minimum supported size, immutable logistics facts take
            // priority over action previews that remain available through the
            // footer/help and typed controls.
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(16), Constraint::Min(10)])
                .split(area);
            render_energy_logistics(frame, panes[0], view, ui, layout_class);
            let trade = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Min(6)])
                .split(panes[1]);
            render_local_market(frame, trade[0], view, ui, layout_class);
            render_trade_destinations(frame, trade[1], view, ui, layout_class);
        }
    } else if layout_class == LayoutClass::Regular {
        render_regular_trade_core(frame, area, view, ui);
    } else {
        let panes = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Min(5),
            ])
            .split(area);
        render_local_market(frame, panes[0], view, ui, layout_class);
        render_trade_action(frame, panes[1], view, ui, layout_class);
        render_trade_destinations(frame, panes[2], view, ui, layout_class);
        render_trade_route(frame, panes[3], view, ui, layout_class);
    }
}

fn render_regular_trade_core(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(7)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(5),
        ])
        .split(columns[1]);
    render_local_market(frame, left[0], view, ui, LayoutClass::Regular);
    render_trade_action(frame, left[1], view, ui, LayoutClass::Regular);
    render_trade_destinations(frame, right[0], view, ui, LayoutClass::Regular);
    render_trade_route(frame, right[1], view, ui, LayoutClass::Regular);
    render_trade_player(frame, right[2], view, ui);
}

fn has_energy_logistics(view: &ApplicationView) -> bool {
    let logistics = &view.energy_logistics;
    !logistics.markets.is_empty()
        || !logistics.opportunities.is_empty()
        || !logistics.contracts.is_empty()
        || logistics.storage.tank_capacity.0 != 0
        || logistics.storage.bulk_capacity.0 != 0
}

fn focused_energy_market<'a>(
    view: &'a ApplicationView,
    ui: &UiState,
) -> Option<&'a game_app::EnergyMarketLogisticsView> {
    ui.selected_trade_destination
        .as_ref()
        .and_then(|selected| {
            view.energy_logistics
                .markets
                .iter()
                .find(|market| &market.system.id == selected)
        })
        .or_else(|| {
            view.energy_logistics
                .markets
                .iter()
                .find(|market| market.system.id == view.player.location)
        })
        .or_else(|| view.energy_logistics.markets.first())
}

fn focused_energy_opportunity<'a>(
    view: &'a ApplicationView,
    ui: &UiState,
) -> Option<&'a game_app::EnergyContractOpportunityView> {
    let selected = ui
        .selected_trade_destination
        .as_ref()
        .or(ui.route_proposal.as_ref());
    match selected {
        Some(selected) => view
            .energy_logistics
            .opportunities
            .iter()
            .find(|opportunity| {
                &opportunity.destination.id == selected && opportunity.blocker.is_none()
            }),
        None => view
            .energy_logistics
            .opportunities
            .iter()
            .find(|opportunity| opportunity.blocker.is_none()),
    }
}

fn player_energy_contract(view: &ApplicationView) -> Option<&game_app::ActiveEnergyContractView> {
    view.energy_logistics
        .contracts
        .iter()
        .find(|contract| contract.player_owned)
}

fn render_energy_logistics(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    _layout_class: LayoutClass,
) {
    let mut lines = Vec::new();
    if let Some(market) = focused_energy_market(view, ui) {
        lines.push(Line::from(format!(
            "Market {} · Stock {}/{} · Target {} · Request {}",
            market.system.name,
            format_energy(market.stock),
            format_energy(market.capacity),
            format_energy(market.target),
            format_energy(market.requested),
        )));
        lines.push(Line::from(format!(
            "Offer {} · Inbound {} · Runway {} · Stage {}",
            format_energy(market.offered),
            format_energy(market.inbound),
            format_u64(market.runway),
            market.stage.label(),
        )));
        lines.push(Line::from(format!(
            "Cause {} · Blocker {}",
            market
                .cause
                .as_ref()
                .map_or_else(|| "none".into(), debug_label),
            market.blocker.as_deref().unwrap_or("none"),
        )));
    }

    let storage = &view.energy_logistics.storage;
    lines.push(Line::from(format!(
        "Tank {}/{} · Owned bulk {} · Locked {}{}",
        format_energy(storage.tank),
        format_energy(storage.tank_capacity),
        format_energy(storage.owned_bulk),
        format_energy(storage.locked_bulk),
        storage
            .locked_contract
            .map_or_else(String::new, |contract| {
                format!(" (Contract #{})", contract.get())
            }),
    )));
    lines.push(Line::from(format!(
        "Bulk {}/{} · General cargo {}/{}: {}",
        format_energy(storage.bulk_used),
        format_energy(storage.bulk_capacity),
        view.player.cargo_used,
        view.player.cargo_capacity,
        general_cargo_label(view),
    )));
    lines.push(Line::from(format!(
        "Transfer max: tank {} · market {}{}",
        format_energy(storage.owned_to_tank_maximum),
        format_energy(storage.owned_to_market_maximum),
        storage
            .transfer_blocker
            .as_deref()
            .map_or_else(String::new, |blocker| format!(" · blocked: {blocker}")),
    )));

    if let Some(opportunity) = focused_energy_opportunity(view, ui) {
        lines.push(Line::from(format!(
            "Best {} → {} · Payload {}",
            opportunity.source.name,
            opportunity.destination.name,
            format_energy(opportunity.maximum_gross_payload),
        )));
        lines.push(Line::from(format!(
            "Deadhead {} E/{}t · Loaded {} E/{}t · Recovery {} E/{}t",
            format_energy(opportunity.deadhead.burn),
            opportunity.deadhead.ticks,
            format_energy(opportunity.loaded.burn),
            opportunity.loaded.ticks,
            format_energy(opportunity.recovery.burn),
            opportunity.recovery.ticks,
        )));
        lines.push(Line::from(format!(
            "Fee {} · Allocation {} · Profit {} · Net {} · Freight {}",
            format_energy(opportunity.carrier_fee),
            format_energy(opportunity.carrier_allocation),
            format_energy(opportunity.expected_net_profit),
            format_energy(opportunity.net_delivery),
            format_bps(opportunity.freight_rate_bps),
        )));
        lines.push(Line::from(format!(
            "Runway {} → {} · Blocker {}",
            format_u64(opportunity.destination_runway_before),
            format_u64(opportunity.destination_runway_after),
            opportunity.blocker.as_deref().unwrap_or("none"),
        )));
    }

    if let Some(contract) = player_energy_contract(view) {
        lines.push(Line::from(format!(
            "Contract #{} {} · {} → {} · Deadline {}",
            contract.id.get(),
            contract_state_label(&contract.state),
            contract.source.name,
            contract.destination.name,
            contract.deadline.map_or_else(|| "—".into(), format_u64),
        )));
        lines.push(Line::from(format!(
            "Payload {} · Locked {} · D/L/R {}/{}/{} E · ticks {}/{}/{}",
            format_energy(contract.gross_payload),
            format_energy(contract.locked_amount),
            format_energy(contract.deadhead.burn),
            format_energy(contract.loaded.burn),
            format_energy(contract.recovery.burn),
            contract.deadhead.ticks,
            contract.loaded.ticks,
            contract.recovery.ticks,
        )));
        lines.push(Line::from(format!(
            "Fee {} · Allocation {} · Profit {} · Net {} · Freight {}",
            format_energy(contract.carrier_fee),
            format_energy(contract.carrier_allocation),
            format_energy(contract.expected_net_profit),
            format_energy(contract.net_delivery),
            format_bps(contract.freight_rate_bps),
        )));
        lines.push(Line::from(format!(
            "Settled {} · Reimb {} · Fee converted {} · Recovery reserve {}",
            format_energy(contract.cumulative_settled),
            format_energy(contract.converted_reimbursement),
            format_energy(contract.converted_fee),
            format_energy(contract.recovery_reserve),
        )));
        lines.push(Line::from(format!(
            "Carrier {} · Blocker {}",
            contract.carrier_name,
            contract.latest_blocker.as_deref().unwrap_or("none"),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Energy Logistics")),
        area,
    );
}

fn render_local_market(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let ordinary_rows = ordinary_market_rows(view);
    let ordinary_indices = ordinary_market_indices(view);
    let selected_index = ordinary_indices
        .iter()
        .position(|index| *index == ui.market_index)
        .unwrap_or(0);
    let (start, end) = viewport(ordinary_rows.len(), selected_index, capacity);
    let mut rows = ordinary_rows[start..end]
        .iter()
        .copied()
        .enumerate()
        .map(|(offset, row)| {
            let index = start + offset;
            let selected = index == selected_index && ui.trade_region == TradeRegion::Goods;
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
        "Local Market — {}{}{} · {}",
        view.local_trade.system.name,
        if ui.trade_region == TradeRegion::Goods {
            " [ACTIVE]"
        } else {
            ""
        },
        if view.local_trade.available {
            ""
        } else {
            " — UNAVAILABLE"
        },
        viewport_label(start, end, ordinary_rows.len())
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
    let lines = selected_ordinary_market_row(view, ui).map_or_else(
        || {
            vec![
                Line::from("No goods are listed at the local market."),
                Line::from(vec![
                    Span::raw("("),
                    shortcut_span("B"),
                    Span::raw(")uy unavailable · ("),
                    shortcut_span("S"),
                    Span::raw(")ell unavailable"),
                ]),
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
                        "{} · Qty {} · Held {} · Stock {}",
                        row.name, ui.trade_quantity, held, row.inventory
                    )),
                    Line::from(format!(
                        "Buy total {} · Tank {}→{} E · Cargo {}→{}/{}",
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
                        "Sell total {} · Tank {}→{} E · Cargo {}→{}/{}",
                        sell_total
                            .map_or_else(|| "overflow".into(), |total| format!("{} E", total.0)),
                        view.player.tank_energy.0,
                        sell_total.map_or(view.player.tank_energy.0, |total| {
                            view.player.tank_energy.0.saturating_add(total.0)
                        }),
                        view.player.cargo_used,
                        view.player
                            .cargo_used
                            .saturating_sub(u64::from(ui.trade_quantity)),
                        view.player.cargo_capacity,
                    )),
                    Line::from(vec![
                        Span::raw("("),
                        shortcut_span("b"),
                        Span::raw(format!(
                            ")uy {} · ",
                            quick_action_status(buy_reason.as_deref())
                        )),
                        shortcut_span("B"),
                        Span::raw(" order · ("),
                        shortcut_span("s"),
                        Span::raw(format!(
                            ")ell {} · ",
                            quick_action_status(sell_reason.as_deref())
                        )),
                        shortcut_span("S"),
                        Span::raw(" order"),
                    ]),
                ]
            } else {
                vec![
                    Line::from(format!(
                        "{} · Qty {} · Held {} · Market stock {}",
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
                    Line::from(vec![
                        Span::raw("("),
                        shortcut_span("b"),
                        Span::raw(format!(
                            ")uy {} · ",
                            quick_action_status(buy_reason.as_deref())
                        )),
                        shortcut_span("Shift-B"),
                        Span::raw(" one-transaction order"),
                    ]),
                    Line::from(vec![
                        Span::raw("("),
                        shortcut_span("s"),
                        Span::raw(format!(
                            ")ell {} · ",
                            quick_action_status(sell_reason.as_deref())
                        )),
                        shortcut_span("Shift-S"),
                        Span::raw(" one-transaction order"),
                    ]),
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

fn render_trade_route(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
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
    let access = trade_network_access_label(view.player.trade_network_access);
    let title = remote.map_or_else(
        || format!("Route / Trade Network — {access}"),
        |target| {
            format!(
                "Route — {} (read-only) / Network {access}",
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
                if layout_class == LayoutClass::Compact {
                    vec![
                        Line::from(format!(
                            "To {} · {elapsed}/{} ticks · {remaining} left · {} jumps",
                            route.destination_name,
                            route.total_ticks,
                            route.legs.len()
                        )),
                        Line::from(leg.map_or_else(
                            || format_route_chain(route),
                            |leg| format!("Current leg: {} → {}", leg.from_name, leg.to_name),
                        )),
                        Line::from(vec![
                            Span::raw("("),
                            shortcut_span("g"),
                            Span::raw(") run to arrival · Local trading disabled"),
                        ]),
                    ]
                } else {
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
                        Line::from(vec![
                            Span::raw("("),
                            shortcut_span("g"),
                            Span::raw(") run to arrival · Local trading disabled"),
                        ]),
                    ]
                }
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
            || {
                Line::from(vec![
                    Span::raw("("),
                    shortcut_span("T"),
                    Span::raw(")ravel / ("),
                    shortcut_span("g"),
                    Span::raw(") disabled: exact route details unavailable · "),
                    shortcut_span("Esc"),
                    Span::raw(" clears proposal"),
                ])
            },
            |route| {
                if route.required_energy > view.player.tank_energy {
                    Line::from(vec![
                        Span::raw(format!(
                            "Travel / g disabled: needs {} E; tank holds {} E · ",
                            route.required_energy.0, view.player.tank_energy.0
                        )),
                        shortcut_span("Esc"),
                        Span::raw(" clears proposal"),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("("),
                        shortcut_span("T"),
                        Span::raw(")ravel / "),
                        shortcut_span("Enter"),
                        Span::raw(" · ("),
                        shortcut_span("g"),
                        Span::raw(") run to arrival · "),
                        shortcut_span("Esc"),
                        Span::raw(" clears proposal"),
                    ])
                }
            },
        );
        if layout_class == LayoutClass::Compact {
            vec![
                Line::from(format!(
                    "{} → {} · {summary}",
                    view.player.location_name,
                    system_name(view, destination)
                )),
                Line::from(energy),
                command,
            ]
        } else {
            vec![
                Line::from(format!(
                    "Route Proposal: {} → {}",
                    view.player.location_name,
                    system_name(view, destination)
                )),
                Line::from(summary),
                Line::from(energy),
                command,
            ]
        }
    } else {
        vec![
            Line::from("No Route Proposal"),
            Line::from(vec![
                Span::raw("Select a destination in Systems, then press "),
                shortcut_span("F2"),
                Span::raw("."),
            ]),
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
    let cargo = general_cargo_label(view);
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
            "General cargo {}/{} · value {} E: {}",
            p.cargo_used, p.cargo_capacity, p.cargo_energy_value.0, cargo
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

fn render_trade_destinations(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let Some(good) = selected_ordinary_market_row(view, ui) else {
        frame.render_widget(
            Paragraph::new("No selected local good; destination comparison is empty")
                .block(Block::bordered().title("Destination Comparison — read-only")),
            area,
        );
        return;
    };
    let destinations = view
        .trade_markets
        .iter()
        .filter(|market| !market.local)
        .collect::<Vec<_>>();
    let selected_id = ui.selected_trade_destination.as_ref();
    let selected_index = selected_id
        .and_then(|selected| {
            destinations
                .iter()
                .position(|market| &market.system.id == selected)
        })
        .unwrap_or(0);
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let (start, end) = viewport(destinations.len(), selected_index, capacity);
    let mut rows = destinations[start..end]
        .iter()
        .map(|comparison| {
            let market = comparison
                .market
                .iter()
                .find(|row| row.good_id == good.good_id);
            let selected = ui.trade_region == TradeRegion::Destinations
                && selected_id == Some(&comparison.system.id);
            let (stock, target, buy, sell) = market.map_or_else(
                || ("—".into(), "—".into(), "—".into(), "—".into()),
                |row| {
                    (
                        row.inventory.to_string(),
                        row.target.to_string(),
                        format!("{} E", row.buy_quote.0),
                        format!("{} E", row.sell_quote.0),
                    )
                },
            );
            let ticks = comparison
                .route
                .as_ref()
                .map_or_else(|| "—".into(), |route| route.total_ticks.to_string());
            let availability = match comparison.availability {
                game_app::TradeDestinationAvailability::CurrentLocation => "LOCAL".into(),
                game_app::TradeDestinationAvailability::Unreachable => "UNREACHABLE".into(),
                game_app::TradeDestinationAvailability::Traveling => "IN TRANSIT".into(),
                game_app::TradeDestinationAvailability::Available => {
                    comparison.route.as_ref().map_or_else(
                        || "ROUTE N/A".into(),
                        |route| {
                            if route.required_energy > view.player.tank_energy {
                                format!("NEEDS {} E", route.required_energy.0)
                            } else {
                                format!("{} E", route.required_energy.0)
                            }
                        },
                    )
                }
            };
            let cells = if layout_class == LayoutClass::Regular {
                vec![
                    Cell::from(if selected { ">" } else { " " }),
                    Cell::from(comparison.system.name.clone()),
                    right_cell(stock),
                    right_cell(target),
                    right_cell(buy),
                    right_cell(sell),
                    right_cell(ticks),
                    Cell::from(availability),
                ]
            } else {
                vec![
                    Cell::from(if selected { ">" } else { " " }),
                    Cell::from(comparison.system.name.clone()),
                    right_cell(format!("{stock}/{target}")),
                    right_cell(format!("{buy}/{sell}")),
                    right_cell(ticks),
                    Cell::from(availability),
                ]
            };
            Row::new(cells).style(selected_style(selected))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("—"),
            Cell::from("No remote systems available"),
        ]));
    }
    let title = format!(
        "Destinations{} — {} — read-only · {}",
        if ui.trade_region == TradeRegion::Destinations {
            " [ACTIVE]"
        } else {
            ""
        },
        good.name,
        viewport_label(start, end, destinations.len())
    );
    let table = if layout_class == LayoutClass::Regular {
        Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Min(11),
            ],
        )
        .header(bold_row([
            "",
            "System",
            "Stock",
            "Target",
            "Mkt buys",
            "Mkt sells",
            "Ticks",
            "Available",
        ]))
    } else {
        Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(11),
                Constraint::Length(11),
                Constraint::Length(17),
                Constraint::Length(6),
                Constraint::Min(11),
            ],
        )
        .header(bold_row([
            "",
            "System",
            "Stock/Tgt",
            "Mkt Buy/Sell",
            "Ticks",
            "Available",
        ]))
    };
    frame.render_widget(
        table
            .column_spacing(1)
            .block(Block::bordered().title(title)),
        area,
    );
}

fn trade_network_access_label(access: game_app::TradeNetworkAccess) -> &'static str {
    match access {
        game_app::TradeNetworkAccess::Offline => "Offline",
        game_app::TradeNetworkAccess::ReservationContracts => "Reservation Contracts",
    }
}

fn render_encyclopedia_activity(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ApplicationView,
    ui: &UiState,
    layout_class: LayoutClass,
) {
    let Some(section) = view
        .encyclopedia
        .sections
        .get(ui.encyclopedia_section_index)
    else {
        frame.render_widget(
            Paragraph::new("No encyclopedia sections are available")
                .block(Block::bordered().title("Encyclopedia")),
            area,
        );
        return;
    };
    let section_bar = Line::from(
        view.encyclopedia
            .sections
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let active = index == ui.encyclopedia_section_index;
                [
                    Span::styled(
                        if active {
                            format!("[{}]", item.title)
                        } else {
                            item.title.clone()
                        },
                        if active {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                    Span::raw("  "),
                ]
            })
            .collect::<Vec<_>>(),
    );
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    frame.render_widget(Paragraph::new(section_bar), panes[0]);
    if layout_class == LayoutClass::Regular {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
            .split(panes[1]);
        render_encyclopedia_articles(frame, columns[0], section, ui);
        render_encyclopedia_article(frame, columns[1], section, ui);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(1)])
            .split(panes[1]);
        render_encyclopedia_articles(frame, rows[0], section, ui);
        render_encyclopedia_article(frame, rows[1], section, ui);
    }
}

fn render_encyclopedia_articles(
    frame: &mut Frame<'_>,
    area: Rect,
    section: &game_app::EncyclopediaSectionView,
    ui: &UiState,
) {
    let selected = ui
        .encyclopedia_article_index
        .min(section.articles.len().saturating_sub(1));
    let capacity = usize::from(area.height.saturating_sub(2)).max(1);
    let (start, end) = viewport(section.articles.len(), selected, capacity);
    let lines = if section.articles.is_empty() {
        vec![Line::from("No articles")]
    } else {
        section.articles[start..end]
            .iter()
            .enumerate()
            .map(|(offset, article)| {
                let is_selected = start + offset == selected;
                Line::from(vec![
                    Span::styled(
                        if is_selected { "> " } else { "  " },
                        if is_selected {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(article.title.clone(), selected_style(is_selected)),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!(
            "Articles · {}",
            viewport_label(start, end, section.articles.len())
        ))),
        area,
    );
}

fn render_encyclopedia_article(
    frame: &mut Frame<'_>,
    area: Rect,
    section: &game_app::EncyclopediaSectionView,
    ui: &UiState,
) {
    let Some(article) = section.articles.get(ui.encyclopedia_article_index) else {
        frame.render_widget(
            Paragraph::new("No selected article").block(Block::bordered().title("Article")),
            area,
        );
        return;
    };
    let lines = article
        .paragraphs
        .iter()
        .enumerate()
        .flat_map(|(index, paragraph)| {
            let mut lines = Vec::new();
            if index > 0 {
                lines.push(Line::default());
            }
            lines.push(Line::from(paragraph.clone()));
            lines
        })
        .collect::<Vec<_>>();
    let block = Block::bordered().title(article.title.clone());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let content_area = panes[1];
    let total_lines = wrapped_article_line_count(&article.paragraphs, content_area.width);
    let visible_lines = usize::from(content_area.height);
    let max_scroll = total_lines.saturating_sub(visible_lines);
    let scroll = usize::from(ui.encyclopedia_article_scroll).min(max_scroll);
    let first = if total_lines == 0 { 0 } else { scroll + 1 };
    let last = scroll.saturating_add(visible_lines).min(total_lines);
    let more = match (scroll > 0, scroll < max_scroll) {
        (true, true) => "more ↑↓",
        (true, false) => "more ↑",
        (false, true) => "more ↓",
        (false, false) => "all visible",
    };
    frame.render_widget(
        Paragraph::new(format!(
            "Lines {first}-{last}/{total_lines} · {more} · PgUp/PgDn scroll"
        ))
        .style(Style::default().fg(Color::Gray)),
        panes[0],
    );
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        content_area,
    );
}

fn wrapped_article_line_count(paragraphs: &[String], width: u16) -> usize {
    let width = usize::from(width);
    if width == 0 {
        return 0;
    }
    paragraphs
        .iter()
        .enumerate()
        .map(|(index, paragraph)| {
            let separator = usize::from(index > 0);
            let mut wrapped = 0_usize;
            let mut current_width = 0_usize;
            for word in paragraph.split_whitespace() {
                let word_width = Line::from(word).width();
                if current_width == 0 {
                    wrapped = wrapped.saturating_add(word_width.div_ceil(width).max(1));
                    current_width = word_width % width;
                    if current_width == 0 && word_width > 0 {
                        current_width = width;
                    }
                } else if current_width.saturating_add(1).saturating_add(word_width) <= width {
                    current_width = current_width.saturating_add(1).saturating_add(word_width);
                } else {
                    wrapped = wrapped.saturating_add(word_width.div_ceil(width).max(1));
                    current_width = word_width % width;
                    if current_width == 0 && word_width > 0 {
                        current_width = width;
                    }
                }
            }
            separator.saturating_add(wrapped.max(1))
        })
        .sum()
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

const ENERGY_GOOD_ID: &str = "core:energy";

fn ordinary_market_rows(view: &ApplicationView) -> Vec<&game_app::MarketRow> {
    view.local_trade
        .market
        .iter()
        .filter(|row| row.good_id.as_str() != ENERGY_GOOD_ID)
        .collect()
}

fn ordinary_market_indices(view: &ApplicationView) -> Vec<usize> {
    view.local_trade
        .market
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.good_id.as_str() != ENERGY_GOOD_ID).then_some(index))
        .collect()
}

fn selected_ordinary_market_row<'a>(
    view: &'a ApplicationView,
    ui: &UiState,
) -> Option<&'a game_app::MarketRow> {
    view.local_trade
        .market
        .get(ui.market_index)
        .filter(|row| row.good_id.as_str() != ENERGY_GOOD_ID)
        .or_else(|| ordinary_market_rows(view).first().copied())
}

fn move_ordinary_market_selection(ui: &mut UiState, view: &ApplicationView, delta: isize) {
    let indices = ordinary_market_indices(view);
    let current = indices
        .iter()
        .position(|index| *index == ui.market_index)
        .unwrap_or(0);
    let next = shifted(current, indices.len(), delta);
    ui.market_index = indices.get(next).copied().unwrap_or(0);
}

fn format_energy(value: game_app::Energy) -> String {
    format_i128(i128::from(value.0))
}

fn format_u64(value: u64) -> String {
    format_i128(i128::from(value))
}

fn format_i128(value: i128) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut grouped =
        String::with_capacity(digits.len() + digits.len() / 3 + usize::from(negative));
    if negative {
        grouped.push('-');
    }
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(character);
    }
    grouped
}

fn format_bps(bps: u32) -> String {
    let whole = bps / 100;
    let fraction = bps % 100;
    if fraction == 0 {
        format!("{whole}%")
    } else if fraction.is_multiple_of(10) {
        format!("{whole}.{}%", fraction / 10)
    } else {
        format!("{whole}.{fraction:02}%")
    }
}

fn debug_label(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn contract_state_label(state: &impl std::fmt::Debug) -> &'static str {
    let state = format!("{state:?}");
    if state.starts_with("DeadheadingToSource") {
        "Deadheading"
    } else if state.starts_with("InTransit") {
        "In Transit"
    } else if state.starts_with("Arrived") {
        "Arrived"
    } else if state.starts_with("Recovering") {
        "Recovering"
    } else {
        "Unknown"
    }
}

fn general_cargo_label(view: &ApplicationView) -> String {
    if view.player.cargo.is_empty() {
        "empty".into()
    } else {
        view.player
            .cargo
            .iter()
            .map(|item| format!("{} x{}", item.good_name, item.quantity))
            .collect::<Vec<_>>()
            .join(", ")
    }
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

fn trade_order_row<'a>(view: &'a ApplicationView, ui: &UiState) -> Option<&'a game_app::MarketRow> {
    let good = ui.trade_order_good.as_ref()?;
    view.local_trade
        .market
        .iter()
        .find(|row| &row.good_id == good)
}

fn held_quantity(view: &ApplicationView, row: &game_app::MarketRow) -> u64 {
    view.player
        .cargo
        .iter()
        .find(|cargo| cargo.good_id == row.good_id)
        .map_or(0, |cargo| cargo.quantity)
}

struct TradeOrderLimit {
    maximum: u32,
    reason: String,
}

fn trade_order_limit(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    side: TradeOrderSide,
) -> TradeOrderLimit {
    if !view.local_trade.available {
        return TradeOrderLimit {
            maximum: 0,
            reason: view
                .local_trade
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "local trading is unavailable".into()),
        };
    }
    let Some(limits) = &row.local_trade_limits else {
        return TradeOrderLimit {
            maximum: 0,
            reason: "trade limit unavailable".into(),
        };
    };
    let limit = match side {
        TradeOrderSide::Buy => &limits.buy,
        TradeOrderSide::Sell => &limits.sell,
    };
    TradeOrderLimit {
        maximum: limit.maximum,
        reason: limit.reason.clone(),
    }
}

fn buy_unavailable_reason(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    quantity: u32,
) -> Option<String> {
    local_trade_unavailable_reason(view, row, TradeOrderSide::Buy, quantity)
        .map(|reason| format!("Buy unavailable: {reason}"))
}

fn sell_unavailable_reason(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    quantity: u32,
) -> Option<String> {
    local_trade_unavailable_reason(view, row, TradeOrderSide::Sell, quantity)
        .map(|reason| format!("Sell unavailable: {reason}"))
}

fn local_trade_unavailable_reason(
    view: &ApplicationView,
    row: &game_app::MarketRow,
    side: TradeOrderSide,
    quantity: u32,
) -> Option<String> {
    if !view.local_trade.available {
        return Some(
            view.local_trade
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "local trading is unavailable".into()),
        );
    }
    let limit = trade_order_limit(view, row, side);
    (quantity > limit.maximum)
        .then(|| format!("maximum is {} because of {}", limit.maximum, limit.reason))
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

fn help_text(activity: Activity) -> Vec<Line<'static>> {
    let contextual = match activity {
        Activity::Systems => Line::from(vec![
            Span::raw("Systems: "),
            shortcut_span("↑/↓"),
            Span::raw(" select · "),
            shortcut_span("Enter"),
            Span::raw(" overview · ("),
            shortcut_span("M"),
            Span::raw(")arket · ("),
            shortcut_span("O"),
            Span::raw(") sort · ("),
            shortcut_span("D"),
            Span::raw(") reverse · "),
            shortcut_span("F2"),
            Span::raw(" propose route"),
        ]),
        Activity::Trade => Line::from(vec![
            Span::raw("Trade: "),
            shortcut_span("Tab/Shift-Tab"),
            Span::raw(" goods/destinations · "),
            shortcut_span("↑/↓"),
            Span::raw(" row · ("),
            shortcut_span("N"),
            Span::raw(") reusable quantity · "),
            shortcut_span("b/s"),
            Span::raw(" quick trade · "),
            shortcut_span("Shift-B/Shift-S"),
            Span::raw(" one-transaction order · ("),
            shortcut_span("E"),
            Span::raw(") accept best Energy · ("),
            shortcut_span("X"),
            Span::raw(") cancel contract · ("),
            shortcut_span("F"),
            Span::raw(") owned bulk to tank · ("),
            shortcut_span("P"),
            Span::raw(") owned bulk to market · ("),
            shortcut_span("T"),
            Span::raw(")ravel/"),
            shortcut_span("Enter"),
            Span::raw(" · ("),
            shortcut_span("g"),
            Span::raw(") run to arrival · "),
            shortcut_span("Esc"),
            Span::raw(
                " clear route · Energy remains the only unit of account for ordinary goods, but Energy itself moves physically only through delivery contracts and exact storage transfers.",
            ),
        ]),
        Activity::Governance => Line::from(vec![
            Span::raw("Governance: "),
            shortcut_span("↑/↓"),
            Span::raw(" row · "),
            shortcut_span("Tab/Shift-Tab"),
            Span::raw(" section · targets show persistent Base → population-scaled Effective · "),
            shortcut_span("←/→"),
            Span::raw(" edit · ("),
            shortcut_span("I"),
            Span::raw(")nspect Systems selection · "),
            shortcut_span("Esc"),
            Span::raw(" governed target"),
        ]),
        Activity::Intelligence => Line::from(vec![
            Span::raw("Intelligence: "),
            shortcut_span("↑/↓"),
            Span::raw(" events · reaching newest resumes tail-follow"),
        ]),
        Activity::Encyclopedia => Line::from(vec![
            Span::raw("Encyclopedia: "),
            shortcut_span("Tab/Shift-Tab"),
            Span::raw(" section · "),
            shortcut_span("↑/↓"),
            Span::raw(" article · "),
            shortcut_span("PgUp/PgDn"),
            Span::raw(" scroll article"),
        ]),
    };
    vec![
        contextual,
        Line::default(),
        Line::from(vec![
            Span::raw("Activities: "),
            shortcut_span("F1"),
            Span::raw(" Systems · "),
            shortcut_span("F2"),
            Span::raw(" Trade · "),
            shortcut_span("F3"),
            Span::raw(" Governance · "),
            shortcut_span("F4"),
            Span::raw(" Intelligence · "),
            shortcut_span("F5"),
            Span::raw(" Encyclopedia"),
        ]),
        Line::from(vec![
            Span::raw("Global: "),
            shortcut_span("Space"),
            Span::raw(" pause/run · "),
            shortcut_span("."),
            Span::raw(" step while paused · "),
            shortcut_span("R"),
            Span::raw(" rate · "),
            shortcut_span("?"),
            Span::raw(" help · "),
            shortcut_span("Q"),
            Span::raw(" quit"),
        ]),
        Line::from(vec![
            Span::raw("Overlays own input; "),
            shortcut_span("Enter"),
            Span::raw(" confirms and "),
            shortcut_span("Esc"),
            Span::raw(" cancels."),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_app::{
        AggregateDynamicsView, CargoItemView, ConnectionView, EncyclopediaArticleView,
        EncyclopediaSectionView, EncyclopediaView, EnergyHealth, GovernorInvestmentPolicy,
        GovernorMarketPolicy, GovernorView, InvestmentView, LocalTradeLimitsView, LocalTradeView,
        MarketEnergyView, MarketRow, PlayerStatusView, PopulationView, PresentationEvent,
        ProductionOutputView, ProductionRecipeView, RouteLegView, RouteView,
        SeasonalGenerationView, SystemIdentityView, SystemInspectionView, SystemListItem, TickRate,
        TradeDestinationAvailability, TradeMarketComparisonView, TradeQuantityLimitView,
    };
    use game_core::{
        BrownoutStage, ContentId, ENERGY_ID, EconomyConfig, Energy, FleetDynamics, FleetMode,
        GameDefinition, GameSession, GoodCategory, GoodDefinition, Governance, InvestmentPolicy,
        InvestmentStatus, MarketAuthority, MarketPolicy, PopulationState, PopulationTrend,
        Position3, RefuelPolicy, SeasonalGenerationState, SeasonalTrend, SystemDefinition,
        TradeNetworkAccess, TraderDefinition,
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
                energy_logistics: Default::default(),
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
                archetype: None,
                energy_tank: Energy(100),
                energy_tank_capacity: Energy(1_000),
                bulk_energy_capacity: Energy::ZERO,
                cargo_capacity: 10,
                speed: 1.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: true,
            }],
            player_trade_network_access: TradeNetworkAccess::Offline,
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
            authored_target: 10,
            buy_quote: Energy(9),
            sell_quote: Energy(11),
            unit_cost: Energy(8),
            funded_demand: 3,
            local_trade_limits: Some(LocalTradeLimitsView {
                buy: TradeQuantityLimitView {
                    maximum: 8,
                    reason: "cargo capacity".into(),
                },
                sell: TradeQuantityLimitView {
                    maximum: 2,
                    reason: "units held".into(),
                },
            }),
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
                production: vec![ProductionOutputView {
                    good_id: id("core:ore"),
                    good_name: "Ore".into(),
                    source_quantity_per_tick: 2,
                    recipes: vec![ProductionRecipeView {
                        recipe_id: id("core:smelt"),
                        recipe_name: "Smelting".into(),
                        quantity_per_run: 1,
                    }],
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
                system: system.clone(),
                available: true,
                unavailable_reason: None,
                market: market.clone(),
            },
            trade_markets: vec![
                TradeMarketComparisonView {
                    system: system.clone(),
                    local: true,
                    read_only: true,
                    availability: TradeDestinationAvailability::CurrentLocation,
                    unavailable_reason: Some("Player is already at this system".into()),
                    route: None,
                    market: market.clone(),
                },
                TradeMarketComparisonView {
                    system: SystemIdentityView {
                        id: id("core:s1"),
                        name: "Brasshaven".into(),
                    },
                    local: false,
                    read_only: true,
                    availability: TradeDestinationAvailability::Available,
                    unavailable_reason: None,
                    route: Some(RouteView {
                        destination_id: id("core:s1"),
                        destination_name: "Brasshaven".into(),
                        legs: vec![RouteLegView {
                            from_id: id("core:s0"),
                            from_name: "Aster Reach".into(),
                            to_id: id("core:s1"),
                            to_name: "Brasshaven".into(),
                            distance: 3.5,
                            travel_ticks: 4,
                        }],
                        current_leg: None,
                        total_distance: 3.5,
                        total_ticks: 4,
                        remaining_ticks: None,
                        required_energy: Energy(4),
                    }),
                    market: vec![MarketRow {
                        good_id: id("core:ore"),
                        name: "Ore".into(),
                        inventory: 3,
                        target: 20,
                        authored_target: 20,
                        buy_quote: Energy(13),
                        sell_quote: Energy(15),
                        unit_cost: Energy(8),
                        funded_demand: 5,
                        local_trade_limits: None,
                    }],
                },
            ],
            encyclopedia: EncyclopediaView {
                sections: vec![
                    EncyclopediaSectionView {
                        title: "Worlds & Population".into(),
                        articles: vec![
                            EncyclopediaArticleView {
                                title: "Systems and Energy".into(),
                                paragraphs: vec!["A system is a location with a market and routes to other systems.".into()],
                            },
                            EncyclopediaArticleView {
                                title: "Brownouts".into(),
                                paragraphs: vec![
                                    "Normal, Throttled, Emergency, and Starvation form the brownout ladder."
                                        .into(),
                                ],
                            },
                        ],
                    },
                    EncyclopediaSectionView {
                        title: "Recipes".into(),
                        articles: vec![EncyclopediaArticleView {
                            title: "Alloy Smelting".into(),
                            paragraphs: vec![
                                "Inputs: 3 Ferrite Ore. Outputs: 2 Structural Alloy."
                                    .into(),
                            ],
                        }],
                    },
                ],
            },
            dynamics: AggregateDynamicsView {
                stage_occupancy_ticks: [10, 2, 1, 0],
                stage_transitions: 3,
                population_changes: 1,
                population_milestones: 1,
            },
            player: PlayerStatusView {
                trade_network_access: TradeNetworkAccess::Offline,
                location: id("core:s0"),
                location_name: "Aster Reach".into(),
                tank_energy: Energy(100),
                tank_capacity: Energy(250),
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
            energy_logistics: game_app::EnergyLogisticsView::default(),
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
    fn energy_logistics_render_all_required_contract_facts_in_both_layouts() {
        let mut view = test_view();
        let mut ids = game_core::EnergyContracts::default();
        let contract_id = ids.allocate_id().unwrap();
        let source = SystemIdentityView {
            id: id("core:s0"),
            name: "Aster Reach".into(),
        };
        let destination = SystemIdentityView {
            id: id("core:s1"),
            name: "Brasshaven".into(),
        };
        let route = |burn, ticks| game_app::EnergyRouteFactsView {
            systems: vec![source.clone(), destination.clone()],
            burn: Energy(burn),
            ticks,
        };
        view.energy_logistics = game_app::EnergyLogisticsView {
            markets: vec![game_app::EnergyMarketLogisticsView {
                system: destination.clone(),
                stock: Energy(30),
                capacity: Energy(5_000),
                target: Energy(5_000),
                offered: Energy::ZERO,
                requested: Energy(3_940),
                inbound: Energy(3_940),
                runway: 3,
                stage: BrownoutStage::Emergency,
                cause: Some(game_core::EnergyStarvationCause::AcceptedDeliveryPending),
                blocker: Some("accepted delivery pending".into()),
            }],
            opportunities: vec![game_app::EnergyContractOpportunityView {
                source: source.clone(),
                destination: destination.clone(),
                maximum_gross_payload: Energy(4_000),
                deadhead: route(10, 1),
                loaded: route(20, 2),
                recovery: route(20, 2),
                carrier_fee: Energy(40),
                carrier_allocation: Energy(60),
                net_delivery: Energy(3_940),
                freight_rate_bps: 150,
                expected_net_profit: Energy(30),
                score: 10_000_000,
                destination_runway_before: 3,
                destination_runway_after: 42,
                blocker: None,
            }],
            contracts: vec![game_app::ActiveEnergyContractView {
                id: contract_id,
                state: game_core::EnergyContractState::Arrived {
                    arrived_tick: 7,
                    settlement_deadline: 27,
                },
                source: source.clone(),
                destination: destination.clone(),
                carrier_id: id("core:player"),
                carrier_name: "Free Trader".into(),
                player_owned: true,
                gross_payload: Energy(4_000),
                deadhead: route(10, 1),
                loaded: route(20, 2),
                recovery: route(20, 2),
                carrier_fee: Energy(40),
                carrier_allocation: Energy(60),
                net_delivery: Energy(3_940),
                freight_rate_bps: 150,
                expected_net_profit: Energy(30),
                current_leg: None,
                remaining_ticks: None,
                locked_amount: Energy(1_940),
                cumulative_settled: Energy(2_000),
                converted_reimbursement: Energy(20),
                converted_fee: Energy(20),
                deadline: Some(27),
                recovery_reserve: Energy(20),
                latest_blocker: Some("storage headroom".into()),
            }],
            storage: game_app::PlayerEnergyStorageView {
                tank: Energy(100),
                tank_capacity: Energy(250),
                owned_bulk: Energy(60),
                locked_bulk: Energy(1_940),
                locked_contract: Some(contract_id),
                bulk_used: Energy(2_000),
                bulk_capacity: Energy(4_000),
                owned_to_tank_maximum: Energy::ZERO,
                owned_to_market_maximum: Energy::ZERO,
                transfer_blocker: Some("active Energy contract".into()),
            },
            diagnostics: game_core::EnergyLogisticsDiagnostics::default(),
        };
        let ui = UiState {
            activity: Activity::Trade,
            ..UiState::default()
        };

        let regular = rendered_at(160, 45, &view, &ui);
        for fact in [
            "Energy Logistics",
            "Payload 4,000",
            "Deadhead 10",
            "Loaded 20",
            "Fee 40",
            "Profit 30",
            "Net 3,940",
            "Freight 1.5%",
            "Recovery 20",
            "Runway 3 → 42",
            "Locked 1,940",
            "Deadline 27",
        ] {
            assert!(regular.contains(fact), "missing {fact:?} in:\n{regular}");
        }
        let compact = rendered_at(80, 30, &view, &ui);
        for fact in [
            "Energy Logistics",
            "Request 3,940",
            "Tank 100/250",
            "Locked 1,940",
        ] {
            assert!(compact.contains(fact), "missing {fact:?} in:\n{compact}");
        }
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
        for label in [
            "Local Market",
            "Funded",
            "Destinations",
            "Route / Trade Network",
        ] {
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
        ui.governance_index = 2;
        let previous_target = view.inspection.market[0].authored_target;
        handle_key(KeyCode::Right, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(
            view.inspection.market[0].authored_target,
            previous_target + 1
        );

        ui.governance_index = 2 + view.inspection.market.len().saturating_mul(2);
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
            .insert(id(ENERGY_ID), 100);
        ui.governance_index = 2 + invalid.inspection.market.len();
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
        assert!(compact_detail.contains("Configured production capability"));
        assert!(compact_detail.contains("Ore — source base 2/tick; Smelting 1/run"));
        assert!(!compact_detail.contains("Systems — Name"));

        for (width, height) in [(160, 45), (200, 60)] {
            let rendered = rendered_at(width, height, &view, &UiState::default());
            assert!(rendered.contains("F1 Systems"));
            assert!(rendered.contains("Systems — Name ↑"));
            assert!(rendered.contains("Selected System Overview"));
            assert!(rendered.contains("Configured production capability"));
            assert!(rendered.contains("Ore — source base 2/tick; Smelting 1/run"));
            assert!(!rendered.contains("Local Market"));
        }

        let mut no_production = view;
        no_production.systems[0].production.clear();
        let rendered = rendered_at(
            80,
            30,
            &no_production,
            &UiState {
                input_layer: InputLayer::Detail,
                ..UiState::default()
            },
        );
        assert!(rendered.contains("Configured production capability: no goods produced"));
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
    fn systems_table_preserves_the_complete_energy_gauge_at_supported_widths() {
        let view = test_view();
        for (width, height) in [(80, 30), (160, 45)] {
            let rendered = rendered_at(width, height, &view, &UiState::default());
            let cells = rendered.chars().collect::<Vec<_>>();
            let row = cells
                .chunks(usize::from(width))
                .map(|line| line.iter().collect::<String>())
                .find(|line| line.contains(">   Aster"))
                .expect("selected system row");
            assert!(
                row.contains("[#####-] 800/1000"),
                "energy gauge was clipped at width {width}: {row:?}"
            );
            let name_start = row.find("Aster").unwrap();
            let flags_start = row.find("LOC GOV").unwrap();
            assert!(
                flags_start.saturating_sub(name_start) <= 21,
                "system name column used excess width at {width}: {row:?}"
            );
        }
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
    fn system_warn_marker_only_marks_actual_warning_states() {
        fn selected_row(view: &ApplicationView) -> String {
            rendered_at(160, 45, view, &UiState::default())
                .chars()
                .collect::<Vec<_>>()
                .chunks(160)
                .map(|line| line.iter().collect::<String>())
                .find(|line| line.contains(">   Aster"))
                .expect("selected system row")
        }

        let mut view = test_view();
        view.systems[0].brownout_stage = BrownoutStage::Normal;
        view.systems[0].health = EnergyHealth::Healthy;
        assert_eq!(system_order_items(&view)[0].risk, 0);
        assert!(!selected_row(&view).contains("WARN"));

        view.systems[0].brownout_stage = BrownoutStage::Throttled;
        assert_eq!(system_order_items(&view)[0].risk, 1);
        assert!(!selected_row(&view).contains("WARN"));

        view.systems[0].health = EnergyHealth::Low;
        assert!(system_order_items(&view)[0].risk >= 2);
        assert!(selected_row(&view).contains("WARN"));
    }

    #[test]
    fn selected_remote_market_has_an_explicit_read_only_detail_surface() {
        let mut view = test_view();
        view.inspection.system = SystemIdentityView {
            id: id("core:s1"),
            name: "Brasshaven".into(),
        };
        view.inspection.read_only_market = true;
        view.selected_system = id("core:s1");
        let mut remote = view.systems[0].clone();
        remote.id = id("core:s1");
        remote.name = "Brasshaven".into();
        remote.player_location = false;
        remote.player_governed = false;
        view.systems.push(remote);
        let rendered = rendered_at(
            80,
            30,
            &view,
            &UiState {
                selected_system: Some(id("core:s1")),
                input_layer: InputLayer::Detail,
                system_detail: SystemDetailKind::Market,
                ..UiState::default()
            },
        );
        assert!(rendered.contains("Remote Market — Brasshaven (read-only)"));
        assert!(rendered.contains("Ore"));
        assert!(rendered.contains("9 E"));
        assert!(rendered.contains("11 E"));
    }

    #[tokio::test]
    async fn systems_navigation_wraps_and_governance_tab_jumps_sections() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState::default();
        let mut view = app.views.borrow().clone();
        handle_key(KeyCode::Up, &mut ui, &view, &app).await.unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s1"));
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s0"));

        handle_key(KeyCode::F(3), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(ui.governance_index, 0);
        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.governance_index, 2);
        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.governance_index, 2 + view.inspection.market.len());
        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(
            ui.governance_index,
            2 + view.inspection.market.len().saturating_mul(2)
        );
        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.governance_index, 0);
        handle_key(KeyCode::BackTab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(
            ui.governance_index,
            2 + view.inspection.market.len().saturating_mul(2)
        );
        app.shutdown().await.unwrap();
    }

    #[test]
    fn shortcut_keys_use_one_accent_color_in_primary_surfaces() {
        fn text_is_color(
            buffer: &ratatui::buffer::Buffer,
            mut y_range: std::ops::Range<u16>,
            text: &str,
            color: Color,
        ) -> bool {
            let width = text.chars().count() as u16;
            y_range.any(|y| {
                (0..buffer.area.width.saturating_sub(width).saturating_add(1)).any(|start| {
                    let candidate = (start..start + width)
                        .map(|x| buffer.cell((x, y)).map_or("", |cell| cell.symbol()))
                        .collect::<String>();
                    candidate == text
                        && (start..start + width)
                            .all(|x| buffer.cell((x, y)).is_some_and(|cell| cell.fg == color))
                })
            })
        }

        let view = test_view();
        let backend = TestBackend::new(160, 45);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &view, &UiState::default()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(text_is_color(buffer, 0..1, "F1", Color::Yellow));
        for key in ["↑/↓", "Enter", "F2", "Space"] {
            assert!(text_is_color(buffer, 43..45, key, Color::Yellow), "{key}");
        }

        let backend = TestBackend::new(80, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    &view,
                    &UiState {
                        activity: Activity::Trade,
                        ..UiState::default()
                    },
                )
            })
            .unwrap();
        assert!(text_is_color(
            terminal.backend().buffer(),
            12..22,
            "B",
            Color::Yellow
        ));
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
            assert!(trade.contains("Destinations"));
            assert!(trade.contains("Brasshaven"));
            assert!(trade.contains("Route — Brasshaven (read-only)"));
            assert!(trade.contains("Qty 1"));
            assert!(trade.contains(if width == 80 {
                "Aster Reach → Brasshaven"
            } else {
                "Route Proposal"
            }));
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
            assert!(governance.contains("Market Targets"));
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
    async fn run_to_arrival_shortcut_starts_the_route_and_arms_auto_pause() {
        let app = game_app::spawn(test_session());
        app.request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let view = app.views.borrow().clone();
        let mut ui = UiState {
            activity: Activity::Trade,
            route_proposal: Some(id("core:s1")),
            ..UiState::default()
        };
        handle_key(KeyCode::Char('g'), &mut ui, &view, &app)
            .await
            .unwrap();
        let running = app.views.borrow().clone();
        assert_eq!(running.run_state, RunState::RunningUntilArrival);
        assert!(running.player.traveling);
        assert!(ui.message.contains("Running until arrival"));
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

    #[test]
    fn encyclopedia_and_trade_comparisons_render_responsively_with_explicit_edge_states() {
        let view = test_view();
        for (width, height) in [(80, 30), (160, 45)] {
            let encyclopedia = rendered_at(
                width,
                height,
                &view,
                &UiState {
                    activity: Activity::Encyclopedia,
                    ..UiState::default()
                },
            );
            assert!(encyclopedia.contains("Worlds & Population"));
            assert!(encyclopedia.contains("Articles"));
            assert!(encyclopedia.contains("Systems and Energy"));
            assert!(encyclopedia.contains("A system is a location with a market"));
            assert_eq!(encyclopedia.matches("> ").count(), 1);

            let trade = rendered_at(
                width,
                height,
                &view,
                &UiState {
                    activity: Activity::Trade,
                    trade_region: TradeRegion::Destinations,
                    selected_trade_destination: Some(id("core:s1")),
                    route_proposal: Some(id("core:s1")),
                    ..UiState::default()
                },
            );
            for fact in ["Stock", "Mkt", "Ticks", "Brasshaven"] {
                assert!(trade.contains(fact), "missing comparison fact {fact}");
            }
            assert!(trade.contains(if width == 80 { "Stock/Tgt" } else { "Target" }));
            assert!(trade.contains("Offline"));
        }

        let mut unreachable = view.clone();
        let remote = unreachable
            .trade_markets
            .iter_mut()
            .find(|market| !market.local)
            .unwrap();
        remote.availability = TradeDestinationAvailability::Unreachable;
        remote.unavailable_reason = Some("No route".into());
        remote.route = None;
        let rendered = rendered_at(
            80,
            30,
            &unreachable,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                selected_trade_destination: Some(id("core:s1")),
                route_proposal: Some(id("core:s1")),
                ..UiState::default()
            },
        );
        assert!(rendered.contains("UNREACHABLE"));
        assert!(rendered.contains("Required energy unavailable"));

        let mut traveling = view.clone();
        traveling.player.traveling = true;
        traveling.trade_markets[1].availability = TradeDestinationAvailability::Traveling;
        traveling.selected_route = traveling.trade_markets[1].route.clone();
        let rendered = rendered_at(
            80,
            30,
            &traveling,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                selected_trade_destination: Some(id("core:s1")),
                ..UiState::default()
            },
        );
        assert!(rendered.contains("IN TRANSIT"));
        assert!(rendered.contains("To Brasshaven"));

        let mut empty = view;
        empty.local_trade.market.clear();
        empty.trade_markets.clear();
        let rendered = rendered_at(
            80,
            30,
            &empty,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                ..UiState::default()
            },
        );
        assert!(rendered.contains("destination comparison is empty"));
    }

    #[test]
    fn one_transaction_order_focuses_limits_cost_and_capacity() {
        let mut view = test_view();
        view.local_trade.market[0].inventory = 100;
        view.local_trade.market[0].sell_quote = Energy(10);
        view.player.tank_energy = Energy(1_000);
        view.player.tank_capacity = Energy(2_000);
        view.player.cargo_used = 340;
        view.player.cargo_capacity = 400;
        view.systems[0].energy_capacity = Energy(10_000);
        view.local_trade.market[0]
            .local_trade_limits
            .as_mut()
            .unwrap()
            .buy = TradeQuantityLimitView {
            maximum: 60,
            reason: "cargo capacity".into(),
        };
        let ui = UiState {
            activity: Activity::Trade,
            input_layer: InputLayer::Order,
            trade_order_side: Some(TradeOrderSide::Buy),
            trade_order_good: Some(id("core:ore")),
            quantity_input: Some("72".into()),
            trade_quantity: 5,
            ..UiState::default()
        };

        for (width, height) in [(80, 30), (160, 45)] {
            let rendered = rendered_at(width, height, &view, &ui);
            for fact in [
                "One-Transaction Buy Order",
                "Requested: 72_ · Reusable preset: 5",
                "Maximum now: 60 (cargo capacity)",
                "Order total: 720 E",
                "At maximum: 600 E · Tank 1000→400 E · Cargo 340→400/400",
                "Requested 72 exceeds the maximum by 12",
                "(M) use maximum",
            ] {
                assert!(
                    rendered.contains(fact),
                    "missing order fact at {width}x{height}: {fact}"
                );
            }
        }

        let sell_limit =
            trade_order_limit(&view, &view.local_trade.market[0], TradeOrderSide::Sell);
        assert_eq!(sell_limit.maximum, 2);
        assert_eq!(sell_limit.reason, "units held");
    }

    #[tokio::test]
    async fn one_transaction_order_uses_maximum_without_changing_reusable_quantity() {
        let app = game_app::spawn(test_session());
        let mut view = app.views.borrow().clone();
        let ore_index = view
            .local_trade
            .market
            .iter()
            .position(|row| row.name == "Ore")
            .unwrap();
        let expected_maximum = view.local_trade.market[ore_index]
            .local_trade_limits
            .as_ref()
            .unwrap()
            .buy
            .maximum;
        let mut ui = UiState {
            activity: Activity::Trade,
            market_index: ore_index,
            trade_quantity: 3,
            ..UiState::default()
        };

        handle_key(KeyCode::Char('S'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('7'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('m'), &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.quantity_input.as_deref(), Some("7"));
        assert!(ui.message.contains("units held"));
        handle_key(KeyCode::Esc, &mut ui, &view, &app)
            .await
            .unwrap();

        handle_key(KeyCode::Char('B'), &mut ui, &view, &app)
            .await
            .unwrap();
        view.local_trade.market.reverse();
        handle_key(KeyCode::Char('9'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('9'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.input_layer, InputLayer::Order);
        assert_eq!(app.views.borrow().player.cargo_used, 0);
        assert!(ui.message.contains("maximum"));

        handle_key(KeyCode::Char('m'), &mut ui, &view, &app)
            .await
            .unwrap();
        let maximum = ui.quantity_input.clone().unwrap().parse::<u32>().unwrap();
        assert_eq!(maximum, expected_maximum);
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.input_layer, InputLayer::Root);
        assert_eq!(ui.trade_order_side, None);
        assert_eq!(
            ui.trade_quantity, 3,
            "one-off orders must not change preset"
        );
        assert_eq!(app.views.borrow().player.cargo_used, u64::from(maximum));

        let view_after_buy = app.views.borrow().clone();
        ui.market_index = view_after_buy
            .local_trade
            .market
            .iter()
            .position(|row| row.name == "Ore")
            .unwrap();
        handle_key(KeyCode::Char('S'), &mut ui, &view_after_buy, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('m'), &mut ui, &view_after_buy, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &view_after_buy, &app)
            .await
            .unwrap();
        assert_eq!(ui.input_layer, InputLayer::Root);
        assert_eq!(ui.trade_quantity, 3);
        assert_eq!(app.views.borrow().player.cargo_used, 0);

        let view_after_sell = app.views.borrow().clone();
        handle_key(KeyCode::Char('B'), &mut ui, &view_after_sell, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('7'), &mut ui, &view_after_sell, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Esc, &mut ui, &view_after_sell, &app)
            .await
            .unwrap();
        assert_eq!(ui.input_layer, InputLayer::Root);
        assert_eq!(ui.trade_order_good, None);
        assert_eq!(ui.trade_quantity, 3);
        assert_eq!(app.views.borrow().player.cargo_used, 0);
        app.shutdown().await.unwrap();
    }

    #[test]
    fn compact_trade_preserves_exact_action_and_route_consequences_at_80x30() {
        let mut view = test_view();
        view.selected_system = id("core:s1");
        view.selected_route = view.trade_markets[1].route.clone();
        let rendered = rendered_at(
            80,
            30,
            &view,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                selected_trade_destination: Some(id("core:s1")),
                route_proposal: Some(id("core:s1")),
                ..UiState::default()
            },
        );

        for fact in [
            "Buy total 11 E · Tank 100→89 E · Cargo 2→3/10",
            "Sell total 9 E · Tank 100→109 E · Cargo 2→1/10",
            "Aster Reach → Brasshaven",
            "1 jumps · 3.5 distance · 4 ticks",
            "Requires 4 E · after arrival 96 E",
            "Brasshaven",
        ] {
            assert!(
                rendered.contains(fact),
                "missing compact Trade fact: {fact}"
            );
        }
        assert!(rendered.contains("(T)ravel / Enter · (g) run to arrival"));
    }

    #[test]
    fn trade_layout_gives_surplus_height_to_scrollable_market_lists() {
        let mut view = test_view();
        let local_template = view.local_trade.market[0].clone();
        view.local_trade.market = (0..35)
            .map(|index| {
                let mut row = local_template.clone();
                row.good_id = id(&format!("core:good_{index}"));
                row.name = format!("Good {index:02}");
                row
            })
            .collect();
        let remote_template = view.trade_markets[1].clone();
        view.trade_markets = (1..=30)
            .map(|index| {
                let mut market = remote_template.clone();
                market.system.id = id(&format!("core:s{index}"));
                market.system.name = format!("System {index:02}");
                market
            })
            .collect();

        let compact = rendered_at(
            80,
            30,
            &view,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                selected_trade_destination: Some(id("core:s1")),
                route_proposal: Some(id("core:s1")),
                ..UiState::default()
            },
        );
        assert!(compact.contains("1-5/35"));
        assert!(compact.contains("1-4/30"));
        assert!(compact.contains("Buy total"));
        assert!(compact.contains("Aster Reach → Brasshaven"));

        let regular = rendered_at(
            160,
            45,
            &view,
            &UiState {
                activity: Activity::Trade,
                trade_region: TradeRegion::Destinations,
                selected_trade_destination: Some(id("core:s1")),
                route_proposal: Some(id("core:s1")),
                ..UiState::default()
            },
        );
        assert!(regular.contains("1-31/35"));
        assert!(regular.contains("1-26/30"));
        assert!(regular.contains("Player / Trade"));
        assert!(regular.contains("Route Proposal"));
    }

    #[tokio::test]
    async fn encyclopedia_pages_through_long_wrapped_articles_and_resets_scroll() {
        let app = game_app::spawn(test_session());
        let mut view = test_view();
        let mut paragraphs = (0..69)
            .map(|index| {
                format!(
                    "Catalog fact {index}: this deliberately long factual paragraph verifies wrapped encyclopedia content remains reachable."
                )
            })
            .collect::<Vec<_>>();
        paragraphs.push("FINAL ENCYCLOPEDIA PARAGRAPH".into());
        view.encyclopedia.sections[0].articles[0].paragraphs = paragraphs;

        for (width, height) in [(80, 30), (160, 45)] {
            let mut ui = UiState {
                activity: Activity::Encyclopedia,
                ..UiState::default()
            };
            let initial = rendered_at(width, height, &view, &ui);
            assert!(initial.contains("more ↓"));
            assert!(initial.contains("PgUp/PgDn scroll"));
            assert!(!initial.contains("FINAL ENCYCLOPEDIA PARAGRAPH"));

            let mut reached_end = false;
            for _ in 0..40 {
                handle_key(KeyCode::PageDown, &mut ui, &view, &app)
                    .await
                    .unwrap();
                let rendered = rendered_at(width, height, &view, &ui);
                if rendered.contains("FINAL ENCYCLOPEDIA PARAGRAPH") {
                    assert!(rendered.contains("more ↑"));
                    reached_end = true;
                    break;
                }
            }
            assert!(
                reached_end,
                "long article was not reachable at {width}x{height}"
            );

            handle_key(KeyCode::Down, &mut ui, &view, &app)
                .await
                .unwrap();
            assert_eq!(ui.encyclopedia_article_index, 1);
            assert_eq!(ui.encyclopedia_article_scroll, 0);
            handle_key(KeyCode::PageDown, &mut ui, &view, &app)
                .await
                .unwrap();
            assert!(ui.encyclopedia_article_scroll > 0);
            handle_key(KeyCode::Tab, &mut ui, &view, &app)
                .await
                .unwrap();
            assert_eq!(ui.encyclopedia_section_index, 1);
            assert_eq!(ui.encyclopedia_article_index, 0);
            assert_eq!(ui.encyclopedia_article_scroll, 0);
        }
        app.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn destination_selection_edge_states_do_not_mutate_simulation_or_commit_routes() {
        let app = game_app::spawn(test_session());
        let initial = app.views.borrow().clone();

        let mut no_destinations = initial.clone();
        no_destinations.trade_markets.retain(|market| market.local);
        let mut ui = UiState {
            activity: Activity::Trade,
            ..UiState::default()
        };
        handle_key(KeyCode::Tab, &mut ui, &no_destinations, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &no_destinations, &app)
            .await
            .unwrap();
        let after_empty = app.views.borrow().clone();
        assert_eq!(ui.selected_trade_destination, None);
        assert_eq!(ui.route_proposal, None);
        assert_eq!(after_empty.tick, initial.tick);
        assert_eq!(after_empty.selected_system, initial.selected_system);
        assert_eq!(after_empty.player.location, initial.player.location);
        assert_eq!(after_empty.player.traveling, initial.player.traveling);
        assert_eq!(after_empty.player.tank_energy, initial.player.tank_energy);
        assert_eq!(after_empty.player.cargo_used, initial.player.cargo_used);
        assert_eq!(after_empty.player.transactions, initial.player.transactions);

        let mut unreachable = initial.clone();
        unreachable.trade_markets[1].availability = TradeDestinationAvailability::Unreachable;
        unreachable.trade_markets[1].unavailable_reason = Some("No route".into());
        unreachable.trade_markets[1].route = None;
        let mut ui = UiState {
            activity: Activity::Trade,
            ..UiState::default()
        };
        handle_key(KeyCode::Tab, &mut ui, &unreachable, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &unreachable, &app)
            .await
            .unwrap();
        let after_unreachable = app.views.borrow().clone();
        assert_eq!(ui.selected_trade_destination, Some(id("core:s1")));
        assert_eq!(ui.route_proposal, None);
        assert_eq!(after_unreachable.tick, initial.tick);
        assert_eq!(after_unreachable.selected_system, initial.selected_system);
        assert_eq!(after_unreachable.player.location, initial.player.location);
        assert_eq!(after_unreachable.player.traveling, initial.player.traveling);
        assert_eq!(
            after_unreachable.player.tank_energy,
            initial.player.tank_energy
        );
        assert_eq!(
            after_unreachable.player.cargo_used,
            initial.player.cargo_used
        );
        assert_eq!(
            after_unreachable.player.transactions,
            initial.player.transactions
        );

        app.request(AppRequest::BeginTravel {
            destination: id("core:s1"),
        })
        .await
        .unwrap();
        let traveling = app.views.borrow().clone();
        let mut ui = UiState {
            activity: Activity::Trade,
            ..UiState::default()
        };
        handle_key(KeyCode::Tab, &mut ui, &traveling, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &traveling, &app)
            .await
            .unwrap();
        let after_traveling_selection = app.views.borrow().clone();
        assert_eq!(ui.route_proposal, None);
        assert_eq!(after_traveling_selection.tick, traveling.tick);
        assert_eq!(
            after_traveling_selection.selected_system,
            traveling.selected_system
        );
        assert_eq!(
            after_traveling_selection.player.location,
            traveling.player.location
        );
        assert_eq!(
            after_traveling_selection.player.traveling,
            traveling.player.traveling
        );
        assert_eq!(
            after_traveling_selection.player.tank_energy,
            traveling.player.tank_energy
        );
        assert_eq!(
            after_traveling_selection.player.cargo_used,
            traveling.player.cargo_used
        );
        assert_eq!(
            after_traveling_selection.player.transactions,
            traveling.player.transactions
        );
        assert_eq!(
            after_traveling_selection
                .selected_route
                .as_ref()
                .and_then(|route| route.remaining_ticks),
            traveling
                .selected_route
                .as_ref()
                .and_then(|route| route.remaining_ticks)
        );
        app.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn destination_selection_updates_preview_without_travel_or_local_good_mutation() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState {
            activity: Activity::Trade,
            market_index: 0,
            ..UiState::default()
        };
        let mut view = app.views.borrow().clone();
        let local_good = view.local_trade.market[0].good_id.clone();

        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(ui.trade_region, TradeRegion::Destinations);
        assert_eq!(ui.selected_trade_destination, Some(id("core:s1")));
        assert_eq!(ui.route_proposal, Some(id("core:s1")));
        assert_eq!(view.selected_system, id("core:s1"));
        assert_eq!(
            view.selected_route
                .as_ref()
                .map(|route| &route.destination_id),
            Some(&id("core:s1"))
        );
        assert!(!view.player.traveling);
        assert_eq!(ui.market_index, 0);
        assert_eq!(view.local_trade.market[0].good_id, local_good);

        handle_key(KeyCode::Char('t'), &mut ui, &view, &app)
            .await
            .unwrap();
        assert!(app.views.borrow().player.traveling);
        app.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn obsolete_hidden_target_shortcuts_are_inert_in_runtime_dispatch() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState {
            activity: Activity::Governance,
            ..UiState::default()
        };
        let before = app.views.borrow().clone();
        for key in [']', '[', ',', ';', 'I', '-', '+', '='] {
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

        handle_key(KeyCode::Char('.'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.tick, 1);

        handle_key(KeyCode::F(5), &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.activity, Activity::Encyclopedia);
        assert_eq!(
            app.views.borrow().tick,
            1,
            "activity switches must not step"
        );
        handle_key(KeyCode::F(1), &mut ui, &view, &app)
            .await
            .unwrap();

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
        ui.market_index = view
            .inspection
            .market
            .iter()
            .position(|row| row.good_id == id("core:ore"))
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
        handle_key(KeyCode::Char('s'), &mut ui, &view, &app)
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
