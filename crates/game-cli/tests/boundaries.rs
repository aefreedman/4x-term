use game_app::AppRequest;
use game_core::GameSession;

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
