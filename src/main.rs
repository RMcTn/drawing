use std::{env, path::PathBuf};

use log::info;

mod app;
mod gui;
mod input;
mod persistence;
mod render;
mod replay;
mod state;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        info!("No arguments found. Running without replay");
        app::run(None, None);
        return;
    }

    let replay_path = &args[1];
    if replay_path.is_empty() {
        info!("Empty replay path given. Running without replay");
    } else {
        info!("Running with replay path {}", replay_path);
        app::run(Some(PathBuf::from(replay_path)), None);
    }
}
