use game_app::AppRequest;
use game_core::{ContentId, CoreError, Energy, GameCommand, GameSession};

#[tokio::test]
async fn public_crate_boundaries_compose() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let definition = game_content::load_directory(root).expect("content boundary");
    let session = GameSession::new(definition).expect("core boundary");
    let app = game_app::spawn(session);
    app.request(AppRequest::Step)
        .await
        .expect("application boundary");
    assert_eq!(app.views.borrow().tick, 1);
    let _ui = game_tui::UiState::default();
    app.shutdown().await.expect("shutdown boundary");
}

#[test]
fn player_completes_a_multi_hop_headless_trade() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let definition = game_content::load_directory(root).expect("content boundary");
    let mut session = GameSession::new(definition).expect("core boundary");
    let good = ContentId::new("frontier:ferrite_ore").unwrap();
    let destination = ContentId::new("frontier:system_20").unwrap();
    let origin = ContentId::new("frontier:system_01").unwrap();
    let route = session
        .shortest_path(&origin, &destination)
        .expect("connected route");
    assert!(route.0.len() > 2);

    session
        .submit(GameCommand::Buy {
            good: good.clone(),
            quantity: 2,
        })
        .unwrap();
    session
        .submit(GameCommand::BeginTravel {
            destination: destination.clone(),
        })
        .unwrap();
    assert_eq!(
        session.submit(GameCommand::Buy {
            good: good.clone(),
            quantity: 1
        }),
        Err(CoreError::InTransit)
    );
    for _ in 0..500 {
        if session
            .snapshot()
            .traders
            .iter()
            .find(|trader| trader.player)
            .unwrap()
            .travel
            .is_none()
        {
            break;
        }
        session.step().unwrap();
    }
    let arrived = session.snapshot();
    let player = arrived.traders.iter().find(|trader| trader.player).unwrap();
    assert_eq!(player.system, destination);
    assert!(player.travel.is_none());
    session
        .submit(GameCommand::Sell {
            good: good.clone(),
            quantity: 2,
        })
        .unwrap();
    let finished = session.snapshot();
    let player = finished
        .traders
        .iter()
        .find(|trader| trader.player)
        .unwrap();
    assert_eq!(player.cargo.get(&good).copied().unwrap_or(0), 0);
    assert_eq!(player.ledger.completed_transactions, 2);
    assert!(player.ledger.sales_revenue > Energy(0));
}
