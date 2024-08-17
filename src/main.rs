use std::path::PathBuf;

mod app;
mod gui;
mod input;
mod persistence;
mod render;
mod replay;
mod state;

fn main() {
    app::run(Some(PathBuf::from("./draw_with_colour_change.rae")));
}
