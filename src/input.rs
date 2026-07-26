use std::borrow::Cow;
use std::cmp;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use arboard::{Clipboard, ImageData};
use image::{DynamicImage, ImageFormat, RgbaImage};

use log::{debug, error, info};
use raylib::automation::{AutomationEvent, AutomationEventList};
use raylib::color::Color;
use raylib::ffi::MouseButton;
use raylib::math::rrect;
use raylib::RaylibHandle;

use crate::app::{
    Brush, CanvasImage, GuiColorPickerInfo, HoldCommand, Keymap, Mode, Point, PressCommand,
    Renderable, Stroke, Text, Thing, Tool, RECORDING_OUTPUT_PATH,
};
use crate::persistence::{self, save, save_with_file_picker};
use crate::replay::{load_replay, play_replay};
use crate::state::{State, TextColor, TextSize};

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
            use HoldCommand::*;
            match command {
                CameraZoom(percentage_diff) => {
                    // NOTE: There will be rounding errors here, but we can format the zoom
                    // string
                    state.camera.zoom += *percentage_diff as f32 / 100.0;
                }
                PanCameraHorizontal(diff_per_sec) => {
                    state.camera.target.x += *diff_per_sec as f32 * delta_time;
                }
                PanCameraVertical(diff_per_sec) => {
                    state.camera.target.y += *diff_per_sec as f32 * delta_time;
                }
                // TODO: Changing brush size mid stroke doesn't affect the stroke. Is this the
                // behaviour we want?
                ChangeBrushSize(size_diff_per_sec) => {
                    if state.mode == Mode::UsingTool(Tool::Brush) {
                        brush.brush_size += *size_diff_per_sec as f32 * delta_time
                    }
                }
                SpawnBrushStrokes => {
                    // Create bunch of strokes with random coords in screen space for benchmark testing
                    // @SPEEDUP Allow passed in number of points to allocate to new Stroke

                    for _ in 0..50 {
                        let initial_x: i32 = rl.get_random_value(0..screen_width);
                        let initial_y: i32 = rl.get_random_value(0..screen_height);
                        let generated_points: Vec<Point> = (1..10)
                            .map(|n| Point {
                                x: (initial_x + n) as f32,
                                y: (initial_y + n) as f32,
                            })
                            .collect();

                        let mut generated_stroke = Stroke::new(Color::SKYBLUE, 10.0);
                        generated_stroke.points = generated_points;

                        let thing = Thing {
                            kind: crate::app::Renderable::Stroke(generated_stroke),
                        };
                        state.add_thing_with_undo(thing);
                    }
                }
                ChangeTextSize(size_diff_per_sec) => {
                    if state.mode == Mode::UsingTool(Tool::Text) {
                        let diff_to_apply =
                            cmp::max((*size_diff_per_sec as f32 * delta_time) as u32, 1);
                        if *size_diff_per_sec > 0 {
                            state.text_size.0 = state.text_size.0.saturating_add(diff_to_apply);
                        } else {
                            state.text_size.0 = state.text_size.0.saturating_sub(diff_to_apply);
                        }
                    }
                }
            }
        }
    }
}

pub fn process_key_pressed_events(
    keymap: &Keymap,
    debugging: &mut bool,
    rl: &mut RaylibHandle,
    brush: &mut Brush,
    state: &mut State,
    processed_commands: &mut HashMap<PressCommand, bool>,
    automation_events_list: &mut AutomationEventList,
    automation_events: &mut Vec<AutomationEvent>,
) {
    for (keys, command) in keymap.on_press.iter() {
        let mut all_keys_pressed = true;

        for key in keys {
            if !rl.is_key_down(*key) {
                all_keys_pressed = false;

                processed_commands
                    .entry(*command)
                    .and_modify(|processed| *processed = false);

                break;
            }
        }

        let command_already_processed = processed_commands.get(command).unwrap_or(&false);

        if all_keys_pressed && !command_already_processed {
            processed_commands
                .entry(*command)
                .and_modify(|processed| *processed = true);

            use PressCommand::*;
            match command {
                ToggleDebugging => *debugging = !*debugging,
                Save => {
                    if let Some(current_path) = state.output_path.clone() {
                        if let Err(err) = save(state, &current_path) {
                            eprintln!(
                                "Could not save {}. Error: {}",
                                current_path.to_string_lossy(),
                                err
                            )
                        }
                    } else {
                        save_with_file_picker(state);
                    }
                }
                SaveAs => {
                    save_with_file_picker(state);
                }
                Load => {
                    persistence::load_with_file_picker(state);
                }
                Undo => {
                    state.undo();
                }
                Redo => {
                    state.redo();
                }
                // TODO(reece): Want to check if a brush stroke is already happening? Could just cut
                // the working stroke off when changing brush type
                ChangeBrushType(new_type) => {
                    state.mode = Mode::UsingTool(Tool::Brush);
                    brush.brush_type = *new_type;
                }
                UseTextTool => {
                    state.mode = Mode::UsingTool(Tool::Text);
                    // TODO: Exit text mode without 'saving'
                }
                PickBackgroundColor => {
                    let picker_width = 100;
                    let picker_height = 100;

                    state.mode = Mode::PickingBackgroundColor(GuiColorPickerInfo {
                        initiation_pos: state.mouse_pos,
                        bounds: rrect(
                            state.mouse_pos.x - (picker_width as f32 / 2.0),
                            state.mouse_pos.y - (picker_height as f32 / 2.0),
                            picker_width,
                            picker_height,
                        ),
                        picker_slider_x_padding: 50.0,
                    });
                }
                UseColorPicker => {
                    state.mode = Mode::UsingTool(Tool::ColorPicker);
                }
                UseSelectionPicker => {
                    state.mode = Mode::UsingTool(Tool::Selection);
                }
                RecordLocation => state.locations.push(state.camera.target),
                ToggleKeymapWindow => match state.mode {
                    Mode::ShowingKeymapPanel => state.mode = Mode::default(),
                    _ => state.mode = Mode::ShowingKeymapPanel,
                },
                ToggleRecording => {
                    if state.is_playing_inputs {
                        // Don't want to start recording because we replayed the toggle recording
                        // input :)
                    } else {
                        if state.is_recording_inputs {
                            rl.stop_automation_event_recording();
                            state.is_recording_inputs = false;
                            if automation_events_list.export(RECORDING_OUTPUT_PATH) {
                                // TODO: Really need a way to easily put info messages in the UI
                                println!("Recording saved to {}", RECORDING_OUTPUT_PATH);
                            } else {
                                eprintln!("Couldn't save recording file to {}: Don't have any more info than that I'm afraid :/", RECORDING_OUTPUT_PATH);
                            }
                        } else {
                            state.is_recording_inputs = true;
                            rl.set_automation_event_base_frame(0);
                            rl.start_automation_event_recording();
                        }
                    }
                }
                LoadAndPlayRecordedInputs => {
                    if state.is_recording_inputs {
                        info!("Not loading inputs as we're currently recording");
                    } else {
                        let default_recording_path = Path::new(RECORDING_OUTPUT_PATH);
                        match load_replay(
                            default_recording_path,
                            rl,
                            automation_events_list,
                            automation_events,
                        ) {
                            Some(_) => play_replay(state),
                            None => error!("Error loading replay"),
                        }
                    }
                }
            }
        }
    }
}

/// A key press and a char press are treated differently in Raylib it looks like.
/// Key presses are always uppercase (i.e 'a' will be KEY_A, so will 'A').
/// Char presses are the individual characters that have been pressed, so can differentiate between
/// uppercase and lowercase (same with symbols)
pub fn get_char_pressed() -> Option<u32> {
    let char_pressed = unsafe { raylib::ffi::GetCharPressed() };

    if char_pressed == 0 {
        return None;
    }

    return Some(char_pressed as u32);
}

enum SavedClipboardContents {
    Image {
        width: usize,
        height: usize,
        bytes: Vec<u8>,
    },
    Text(String),
    Empty,
}

/// Keeps deterministic clipboard setup scoped to a replay test and restores the user's clipboard
/// once the application exits.
pub struct TestClipboardGuard {
    clipboard: Clipboard,
    previous: SavedClipboardContents,
}

impl Drop for TestClipboardGuard {
    fn drop(&mut self) {
        let result = match &self.previous {
            SavedClipboardContents::Image {
                width,
                height,
                bytes,
            } => self.clipboard.set_image(ImageData {
                width: *width,
                height: *height,
                bytes: Cow::Borrowed(bytes),
            }),
            SavedClipboardContents::Text(text) => self.clipboard.set_text(text),
            SavedClipboardContents::Empty => self.clipboard.clear(),
        };
        if let Err(error) = result {
            error!("Could not restore clipboard after replay test: {error}");
        }
    }
}

fn test_clipboard_guard() -> Result<TestClipboardGuard, String> {
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    let previous = match clipboard.get_image() {
        Ok(image) => SavedClipboardContents::Image {
            width: image.width,
            height: image.height,
            bytes: image.bytes.into_owned(),
        },
        Err(_) => match clipboard.get_text() {
            Ok(text) => SavedClipboardContents::Text(text),
            Err(_) => SavedClipboardContents::Empty,
        },
    };
    Ok(TestClipboardGuard {
        clipboard,
        previous,
    })
}

pub fn set_clipboard_image_for_test(path: &Path) -> Result<TestClipboardGuard, String> {
    let mut guard = test_clipboard_guard()?;
    let image = image::ImageReader::open(path)
        .map_err(|error| error.to_string())?
        .decode()
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    guard
        .clipboard
        .set_image(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(image.into_raw()),
        })
        .map_err(|error| error.to_string())?;

    Ok(guard)
}

pub fn set_clipboard_text_for_test(path: &Path) -> Result<TestClipboardGuard, String> {
    let mut guard = test_clipboard_guard()?;
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    guard
        .clipboard
        .set_text(text)
        .map_err(|error| error.to_string())?;
    Ok(guard)
}

/// Paste clipboard text into the active text object, or create a text/image renderable at `position`.
/// Images take precedence outside text-entry mode because some clipboard providers expose both.
pub fn paste_clipboard(
    state: &mut State,
    position: raylib::math::Vector2,
    working_text: Option<&mut Text>,
) {
    let mut clipboard = match Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(err) => {
            error!("Could not open clipboard: {err}");
            return;
        }
    };

    if let Some(text) = working_text {
        match clipboard.get_text() {
            Ok(content) => text.content.push_str(&content),
            Err(err) => debug!("Clipboard does not contain text: {err}"),
        }
        return;
    }

    if let Ok(image) = clipboard.get_image() {
        let width = image.width as u32;
        let height = image.height as u32;
        let Some(rgba) = RgbaImage::from_raw(width, height, image.bytes.into_owned()) else {
            error!("Clipboard returned invalid RGBA image data");
            return;
        };
        let mut png_data = Vec::new();
        if let Err(err) = DynamicImage::ImageRgba8(rgba)
            .write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
        {
            error!("Could not encode clipboard image: {err}");
            return;
        }

        state.add_thing_with_undo(Thing {
            kind: Renderable::Image(CanvasImage {
                rect: rrect(position.x, position.y, width as i32, height as i32),
                png_data,
            }),
        });
        return;
    }

    match clipboard.get_text() {
        Ok(content) if !content.is_empty() => state.add_thing_with_undo(Thing {
            kind: Renderable::Text(Text {
                content,
                position: Some(position),
                size: state.text_size,
                color: state.text_color,
            }),
        }),
        Ok(_) => {}
        Err(err) => debug!("Clipboard does not contain text or an image: {err}"),
    }
}

pub fn append_input_to_working_text(
    ch: u32,
    working_text: &mut Option<Text>,
    text_size: TextSize,
    text_color: TextColor,
) {
    if working_text.is_none() {
        let _ = working_text.insert(Text {
            content: "".to_string(),
            position: None,
            size: text_size,
            color: text_color,
        });
    }

    let ch = char::from_u32(ch);
    match ch {
        Some(c) => working_text.as_mut().unwrap().content.push(c), // Was a safe
        // unwrap at the time
        None => (), // TODO: FIXME: Some sort of logging/let the user know for
                    // unrepresentable character?
    }
}

/// Checking for mouse button presses doesn't play well with Raylib automation events.
/// Consider if is_mouse_button_down works for your use case, and this will play nicer with
/// Raylib's automation events
pub fn is_mouse_button_pressed(
    rl: &mut RaylibHandle,
    button: MouseButton,
    mouse_buttons_pressed_this_frame: &mut HashMap<MouseButton, bool>,
) -> bool {
    if rl.is_mouse_button_pressed(button) {
        debug!("{:?} was pressed", button);
        mouse_buttons_pressed_this_frame
            .entry(button)
            .and_modify(|b| *b = true);
        return true;
    } else {
        return false;
    }
}

pub fn was_mouse_button_released(
    rl: &mut RaylibHandle,
    button: MouseButton,
    mouse_buttons_pressed_last_frame: &HashMap<MouseButton, bool>,
) -> bool {
    return !rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT)
        && *mouse_buttons_pressed_last_frame.get(&button).unwrap(); // Should be a safe unwrap, the
                                                                    // hashmap should be pre
                                                                    // populated with needed mouse
                                                                    // keys
}

pub fn is_mouse_button_down(
    rl: &mut RaylibHandle,
    button: MouseButton,
    buttons_pressed_this_frame: &mut HashMap<MouseButton, bool>,
) -> bool {
    if rl.is_mouse_button_down(button) {
        buttons_pressed_this_frame
            .entry(button)
            .and_modify(|b| *b = true);
        return true;
    } else {
        return false;
    }
}
