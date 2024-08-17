use std::path::PathBuf;

mod app;
mod gui;
mod input;
mod persistence;
mod render;
mod replay;
mod state;

fn main() {
    app::run(None);
}
