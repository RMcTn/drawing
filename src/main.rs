use std::path::PathBuf;

use app::TestSettings;
use clap::{Parser, Subcommand};

mod app;
mod gui;
mod input;
mod persistence;
mod render;
mod replay;
mod state;
mod test_snapshot;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Test {
        #[arg(long, requires = "save_path")]
        save_after_replay: bool,
        #[arg(long)]
        quit_after_replay: bool,
        #[arg(long)]
        save_path: Option<PathBuf>,
        #[arg(long)]
        snapshot_path: Option<PathBuf>,
        /// Pause replay and accept frame/event/stroke stepping commands in the terminal.
        #[arg(long)]
        debug_replay: bool,
        /// Put this PNG on the clipboard for the duration of the replay test.
        #[arg(long, conflicts_with = "clipboard_text_path")]
        clipboard_image_path: Option<PathBuf>,
        /// Put this text file on the clipboard for the duration of the replay test.
        #[arg(long, conflicts_with = "clipboard_image_path")]
        clipboard_text_path: Option<PathBuf>,
        #[arg(long)]
        replay_path: PathBuf,
    },
    Run {
        #[arg(long)]
        replay_path: Option<PathBuf>,
    },
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Some(command) => match command {
            Commands::Test {
                save_after_replay,
                save_path,
                replay_path,
                snapshot_path,
                debug_replay,
                clipboard_image_path,
                clipboard_text_path,
                quit_after_replay,
            } => {
                let _clipboard_guard = match (clipboard_image_path, clipboard_text_path) {
                    (Some(path), None) => Some(
                        input::set_clipboard_image_for_test(&path).unwrap_or_else(|error| {
                            panic!(
                                "Could not set test clipboard image {}: {error}",
                                path.display()
                            )
                        }),
                    ),
                    (None, Some(path)) => Some(
                        input::set_clipboard_text_for_test(&path).unwrap_or_else(|error| {
                            panic!(
                                "Could not set test clipboard text {}: {error}",
                                path.display()
                            )
                        }),
                    ),
                    (None, None) => None,
                    (Some(_), Some(_)) => {
                        unreachable!("clap prevents conflicting clipboard fixtures")
                    }
                };
                app::run(
                    Some(replay_path),
                    Some(TestSettings {
                        save_after_replay,
                        save_path,
                        snapshot_path,
                        debug_replay,
                        quit_after_replay,
                    }),
                )
            }
            Commands::Run { replay_path } => app::run(replay_path, None),
        },
        None => app::run(None, None),
    }
}
