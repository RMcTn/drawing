use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::state::State;

pub fn save_with_file_picker(state: &mut State) {
    if let Some(path) = get_save_path() {
        if let Err(err) = save(state, &path) {
            eprintln!("Could not save {}. Error: {}", &path.to_string_lossy(), err)
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
    let mut file = File::create(path)?;
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
        match load(&path) {
            Ok(loaded_state) => {
                *state = loaded_state;
                state.output_path = None;
            }
            Err(e) => {
                eprintln!(
                    "Could not load {}. File doesn't contain valid drawing data. {}",
                    path.to_string_lossy(),
                    e
                )
            }
        }
    } else {
        println!("File picker was exited without picking a file. No loading has taken place");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use raylib::color::Color;

    use crate::{
        app::{Renderable, Stroke, Thing},
        test_snapshot::StateSnapshot,
    };

    use super::{load, save, State};

    #[test]
    fn save_and_load_round_trip_preserves_drawing_state() {
        let mut state = State::default();
        state.add_thing_with_undo(Thing {
            kind: Renderable::Stroke(Stroke {
                points: vec![],
                color: Color::RED,
                brush_size: 12.0,
            }),
        });

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "draw-app-persistence-{}-{unique}.sav",
            std::process::id()
        ));

        save(&state, &path).unwrap();
        let loaded = load(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(StateSnapshot::from(&state), StateSnapshot::from(&loaded));
    }
}
