use anyhow::{Context, Result};
use game_core::GameSession;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("4x-term.log")
        .context("failed to open 4x-term.log")?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_ansi(false)
        .with_writer(Mutex::new(log))
        .try_init()
        .ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let content = content_path(&args)?;
    let loaded = game_content::load_directory_with_encyclopedia(&content)
        .with_context(|| format!("failed to load content from {}", content.display()))?;
    let encyclopedia = encyclopedia_view(loaded.encyclopedia);
    let session = GameSession::new(loaded.definition).context("failed to construct simulation")?;
    game_tui::run(game_app::spawn_with_encyclopedia(session, encyclopedia)).await?;
    Ok(())
}

fn encyclopedia_view(
    sections: Vec<game_content::EncyclopediaSection>,
) -> game_app::EncyclopediaView {
    game_app::EncyclopediaView {
        sections: sections
            .into_iter()
            .map(|section| game_app::EncyclopediaSectionView {
                title: section.title,
                articles: section
                    .articles
                    .into_iter()
                    .map(|article| game_app::EncyclopediaArticleView {
                        title: article.title,
                        paragraphs: article.paragraphs,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn content_path(args: &[String]) -> Result<PathBuf> {
    match args {
        [] => Ok(PathBuf::from("content")),
        [option, path] if option == "--content" && !path.starts_with("--") => {
            Ok(PathBuf::from(path))
        }
        _ => anyhow::bail!("only --content <path> is supported during migration"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_path_accepts_only_the_retained_option() {
        assert_eq!(content_path(&[]).unwrap(), PathBuf::from("content"));
        assert_eq!(
            content_path(&["--content".into(), "fixture".into()]).unwrap(),
            PathBuf::from("fixture")
        );
        assert!(content_path(&["--unknown".into()]).is_err());
        assert!(content_path(&["--content".into()]).is_err());
    }
}
