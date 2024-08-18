use std::{env, path::PathBuf};

use app::TestSettings;
use clap::{Parser, Subcommand};

mod app;
mod gui;
mod input;
mod persistence;
mod render;
mod replay;
mod state;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Test {
        #[arg(long, required = true)]
        save_after_replay: bool,
        #[arg(long, required = true)]
        quit_after_replay: bool,
        #[arg(long)]
        save_path: PathBuf,
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
                quit_after_replay,
            } => app::run(
                Some(replay_path),
                Some(TestSettings {
                    save_after_replay,
                    save_path,
                    quit_after_replay,
                }),
            ),
            Commands::Run { replay_path } => app::run(replay_path, None),
        },
        None => app::run(None, None),
    }
}
