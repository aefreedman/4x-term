mod artifacts;
mod cli;
mod recorder;

use artifacts::{DefaultIdentity, reserve_default, reserve_explicit};
use cli::{Command, TraceRequest};
use game_app::ProfileDescriptor;
use recorder::RonlRecorder;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("4x-term could not start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = cli::parse(std::env::args_os().skip(1))?;
    match command {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Run {
            trace: TraceRequest::Disabled,
        } => run_ordinary(),
        Command::Run {
            trace: TraceRequest::Default,
        } => run_traced(None),
        Command::Run {
            trace: TraceRequest::Explicit(path),
        } => run_traced(Some(path)),
    }
}

fn profile() -> ProfileDescriptor {
    ProfileDescriptor::new(
        PathBuf::from("content/profiles/starter.ron"),
        "starter-profile",
    )
}

fn run_ordinary() -> Result<(), String> {
    game_tui::run(profile(), 0).map_err(|error| error.to_string())
}

fn run_traced(explicit_path: Option<PathBuf>) -> Result<(), String> {
    let reservation = match explicit_path {
        Some(path) => reserve_explicit(path)?,
        None => reserve_default(Path::new("playtest-logs"), DefaultIdentity::current()?)?,
    };
    let artifacts::ArtifactReservation {
        raw_path,
        summary_path,
        raw_file,
        summary_file,
    } = reservation;

    eprintln!("Playtest trace: {}", raw_path.display());
    eprintln!("Playtest summary: {}", summary_path.display());

    let mut recorder = RonlRecorder::new(raw_file);
    let state = game_tui::start(profile(), 0).enable_playtest_trace(env!("CARGO_PKG_VERSION"));
    let run_result = game_tui::run_state_observed(state, &mut recorder);
    let completed = recorder.finish(summary_file, run_result.is_ok());

    match (run_result, completed) {
        (Ok(()), Ok(completed)) => {
            let final_tick = completed
                .final_tick
                .map_or_else(|| "unavailable".to_owned(), |tick| tick.to_string());
            eprintln!(
                "Playtest trace complete: {} ({} events, final tick {final_tick})",
                raw_path.display(),
                completed.event_count
            );
            eprintln!("Playtest summary complete: {}", summary_path.display());
            Ok(())
        }
        (Err(run_error), Ok(_)) => Err(format!(
            "{run_error}; incomplete playtest summary written to {}",
            summary_path.display()
        )),
        (Ok(()), Err(summary_error)) => Err(summary_error),
        (Err(run_error), Err(summary_error)) => Err(format!(
            "{run_error}; additionally, finalizing the playtest artifacts failed: {summary_error}"
        )),
    }
}
