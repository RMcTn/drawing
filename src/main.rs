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
                quit_after_replay,
            } => app::run(
                Some(replay_path),
                Some(TestSettings {
                    save_after_replay,
                    save_path,
                    snapshot_path,
                    debug_replay,
                    quit_after_replay,
                }),
            ),
            Commands::Run { replay_path } => app::run(replay_path, None),
        },
        None => app::run(None, None),
    }
}
