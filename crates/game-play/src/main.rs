use game_app::ProfileDescriptor;
use std::path::PathBuf;

fn main() {
    let profile_path = PathBuf::from("content/profiles/starter.ron");
    let profile = ProfileDescriptor::new(profile_path, "starter-profile");
    if let Err(error) = game_tui::run(profile, 0) {
        eprintln!("4x-term could not start: {error}");
        std::process::exit(1);
    }
}
