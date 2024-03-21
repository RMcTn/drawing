use raylib::color::Color;
use raylib::consts::KeyboardKey;
use raylib::{get_random_value, RaylibHandle};

use crate::persistence::{save, save_with_file_picker};
use crate::state::State;
use crate::{persistence, Brush, Command, Keymap, Point, Stroke, Text, Tool};

pub fn process_key_down_events(
    keymap: &Keymap,
    screen_width: i32,
    screen_height: i32,
    rl: &mut RaylibHandle,
    brush: &mut Brush,
    state: &mut State,
    delta_time: f32,
) {
    for (key, command) in keymap.on_hold.iter() {
        if rl.is_key_down(*key) {
            match command {
                Command::CameraZoom(percentage_diff) => {
                    // NOTE: There will be rounding errors here, but we can format the zoom
                    // string
                    state.camera.zoom += *percentage_diff as f32 / 100.0;
                }
                Command::PanCameraHorizontal(diff_per_sec) => {
                    state.camera.target.x += *diff_per_sec as f32 * delta_time;
                }
                Command::PanCameraVertical(diff_per_sec) => {
                    state.camera.target.y += *diff_per_sec as f32 * delta_time;
                }
                // TODO: Changing brush size mid stroke doesn't affect the stroke. Is this the
                // behaviour we want?
                Command::ChangeBrushSize(size_diff_per_sec) => {
                    brush.brush_size += *size_diff_per_sec as f32 * delta_time
                }
                Command::SpawnBrushStrokes => {
                    // Create bunch of strokes with random coords in screen space for benchmark testing
                    // @SPEEDUP Allow passed in number of points to allocate to new Stroke

                    for _ in 0..50 {
                        let initial_x: i32 = get_random_value(0, screen_width);
                        let initial_y: i32 = get_random_value(0, screen_height);
                        let generated_points: Vec<Point> = (1..10)
                            .map(|n| Point {
                                x: (initial_x + n) as f32,
                                y: (initial_y + n) as f32,
                            })
                            .collect();

                        let mut generated_stroke = Stroke::new(Color::SKYBLUE, 10.0);
                        generated_stroke.points = generated_points;

                        state.add_stroke_with_undo(generated_stroke);
                    }
                }

                c => todo!(
                    "Unimplemented command, or this isn't meant to be a push command: {:?}",
                    c
                ),
            }
        }
    }
}

pub fn process_key_pressed_events(
    keymap: &Keymap,
    debugging: &mut bool,
    rl: &mut RaylibHandle,
    brush: &mut Brush,
    mut state: &mut State,
    current_tool: &mut Tool,
) {
    for (key, command) in keymap.on_press.iter() {
        if rl.is_key_pressed(*key) {
            match command {
                Command::ToggleDebugging => *debugging = !*debugging,
                Command::Save => {
                    if let Some(current_path) = state.output_path.clone() {
                        if let Err(err) = save(&mut state, &current_path) {
                            eprintln!(
                                "Could not save {}. Error: {}",
                                current_path.to_string_lossy(),
                                err.to_string()
                            )
                        }
                    } else {
                        save_with_file_picker(&mut state);
                    }
                }
                Command::SaveAs => {
                    save_with_file_picker(&mut state);
                }
                Command::Load => {
                    persistence::load_with_file_picker(&mut state);
                }
                Command::Undo => {
                    // TODO: Undo/Redo will need reworked for text mode
                    state.undo();
                }
                Command::Redo => {
                    state.redo();
                }
                // TODO(reece): Want to check if a brush stroke is already happening? Could just cut
                // the working stroke off when changing brush type
                Command::ChangeBrushType(new_type) => brush.brush_type = *new_type,
                Command::UseTextTool => {
                    *current_tool = Tool::Text;
                    // TODO: Exit text mode without 'saving'
                }

                c => todo!(
                    "Unimplemented command, or this isn't meant to be a press command: {:?}",
                    c
                ),
            }
        }
    }
}

/// A key press and a char press are treated differently in Raylib it looks like.
/// Key presses are always uppercase (i.e 'a' will be KEY_A, so will 'A').
/// Char presses are the individual characters that have been pressed, so can differentiate between
/// uppercase and lowercase (same with symbols)
pub fn get_char_and_key_pressed(raylib: &mut RaylibHandle) -> (Option<i32>, Option<KeyboardKey>) {
    let char_pressed = unsafe { raylib::ffi::GetCharPressed() };

    let key_pressed = raylib.get_key_pressed();

    if char_pressed == 0 {
        return (None, key_pressed);
    }

    return (Some(char_pressed), key_pressed);
}

pub fn append_input_to_working_text(ch: i32, working_text: &mut Option<Text>) {
    if working_text.is_none() {
        let _ = working_text.insert(Text {
            content: "".to_string(),
            position: None,
        });
    }

    let ch = char::from_u32(ch as u32);
    match ch {
        Some(c) => working_text.as_mut().unwrap().content.push(c), // Was a safe
        // unwrap at the time
        None => (), // TODO: FIXME: Some sort of logging/let the user know for
                    // unrepresentable character?
    }
}
