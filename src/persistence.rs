use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::state::State;

pub fn save_with_file_picker(state: &mut State) {
    if let Some(path) = get_save_path() {
        if let Err(err) = save(state, &path) {
            eprintln!("Could not save {}. Error: {}", &path.to_string_lossy(), err.to_string())
        } else {
            state.output_path = Some(path);
        }
    } else {
        println!("File picker was exited without picking a file. No saving has taken place");
    }
}

pub fn save(state: &State, path: &Path) -> Result<(), std::io::Error> {
    // TODO: FIXME: There's no versioning for save files at the moment
    // so anything new isn't backwards compatible
    let output = serde_json::to_string(&state)?;
    let mut file = File::create(&path)?;
    file.write_all(output.as_bytes())?;
    Ok(())
}

fn get_save_path() -> Option<PathBuf> {
    return rfd::FileDialog::new().save_file();
}

pub fn load(path: &Path) -> Result<State, std::io::Error> {
    let contents = std::fs::read_to_string(path)?;
    let state: State = serde_json::from_str(&contents)?;
    return Ok(state);
}

pub fn get_load_path() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_file()
}

pub fn load_with_file_picker(state: &mut State) {
    if let Some(path) = get_load_path() {
        if let Ok(loaded_state) = load(&path) {
            *state = loaded_state;
            state.output_path = None;
        } else {
            eprintln!("Could not load {}. File doesn't contain valid drawing data.", path.to_string_lossy())
        }
    } else {
        println!("File picker was exited without picking a file. No loading has taken place");
    }
}
