use std::{
    ffi::CString,
    fmt::Display,
    thread,
    time::{self, Instant},
};

use gui::{debug_draw_center_crosshair, draw_info_ui, is_clicking_gui};
use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, DefaultKey, SlotMap};

use crate::{gui::debug_draw_info, input::append_input_to_working_text};
use input::{get_char_and_key_pressed, process_key_down_events, process_key_pressed_events};
use render::{draw_brush_marker, draw_stroke};
use state::State;

mod gui;
mod input;
mod persistence;
mod render;
mod state;

fn main() {
    let keymap = default_keymap();
    let mut debugging = false;

    let mut screen_width = 1280;
    let mut screen_height = 720;

    let (mut rl, thread) = raylib::init()
        .size(screen_width, screen_height)
        .resizable()
        .title("Window")
        .build();

    let target_fps = 60;
    let seconds_per_frame = 1.0 / target_fps as f32;
    let duration_per_frame = time::Duration::from_secs_f32(seconds_per_frame);

    let camera = Camera2D {
        offset: rvec2(screen_width / 2, screen_height / 2),
        target: rvec2(0, 0),
        rotation: 0.0,
        zoom: 1.0,
    };

    let initial_brush_size = 10.0;

    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };

    let mut current_brush_color = Color::BLACK;

    let mut state = State {
        strokes: SlotMap::new(),
        undo_actions: Vec::new(),
        redo_actions: Vec::new(),
        stroke_graveyard: SlotMap::new(),
        text: SlotMap::with_key(),
        text_graveyard: SlotMap::with_key(),
        output_path: None,
        camera,
        background_color: Default::default(),
        mode: Mode::UsingTool(Tool::Brush),
        mouse_pos: rvec2(0, 0),
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(current_brush_color, brush.brush_size);
    let mut working_text: Option<Text> = None;
    let mut last_mouse_pos = rl.get_mouse_position();

    let mut brush_color_picker_info: Option<GuiColorPickerInfo> = None;
    let keymap_panel_bounds = rrect(200, 200, 300, 300);

    while !rl.window_should_close() {
        let delta_time = rl.get_frame_time();
        let current_fps = rl.get_fps();
        // TODO: Hotkey configuration
        // TODO(reece): Have zoom follow the cursor i.e zoom into where the cursor is rather than
        // "top left corner"
        // TODO(reece): Improve how the lines look. Make a line renderer or something?
        // TODO(reece): BUG: Brush marker looks like it's a bit off centre from the mouse cursor
        // TODO(reece): Use shaders for line drawing?
        //
        // TODO(reece): Installable so it's searchable as a program
        // TODO(reece): Optimize this so we're not smashing the cpu/gpu whilst doing nothing (only
        // update on user input?)

        let start_time = Instant::now();
        screen_width = rl.get_screen_width();
        screen_height = rl.get_screen_height();
        state.camera.offset = rvec2(screen_width / 2, screen_height / 2);

        state.mouse_pos = rl.get_mouse_position();
        let drawing_pos = rl.get_screen_to_world2D(state.mouse_pos, state.camera);

        match state.mode {
            Mode::UsingTool(tool) => match tool {
                Tool::Brush => {
                    // TODO: FIXME: Quite easy to accidentally draw when coming out of background
                    // color picker
                    if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                        if let Some(picker_info) = &brush_color_picker_info {
                            if !is_clicking_gui(state.mouse_pos, picker_info.bounds_with_slider()) {
                                brush_color_picker_info = None;
                            }
                        } else {
                            if brush.brush_type == BrushType::Deleting {
                                let strokes_to_delete =
                                    state.strokes_within_point(drawing_pos, brush.brush_size);
                                state.delete_strokes(strokes_to_delete);
                            } else {
                                // Drawing
                                if !is_drawing {
                                    working_stroke =
                                        Stroke::new(current_brush_color, brush.brush_size);
                                    is_drawing = true;
                                }

                                let point = Point {
                                    x: drawing_pos.x,
                                    y: drawing_pos.y,
                                };
                                working_stroke.points.push(point);
                            }
                        }
                    }
                    if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) {
                        // Finished drawing
                        // TODO: FIXME: Do not allow text tool if currently drawing, otherwise we won't be able to end
                        // the brush stroke unless we change back to brush mode
                        if is_drawing {
                            state.add_stroke_with_undo(working_stroke);
                            working_stroke = Stroke::new(current_brush_color, brush.brush_size);
                        }
                        is_drawing = false;
                    }

                    if rl.is_mouse_button_pressed(MouseButton::MOUSE_RIGHT_BUTTON) {
                        let picker_width = 100;
                        let picker_height = 100;
                        brush_color_picker_info = Some(GuiColorPickerInfo {
                            initiation_pos: state.mouse_pos,
                            bounds: rrect(
                                state.mouse_pos.x - (picker_width as f32 / 2.0),
                                state.mouse_pos.y - (picker_height as f32 / 2.0),
                                picker_width,
                                picker_height,
                            ),
                            picker_slider_x_padding: 30.0,
                        });
                    }
                }
                Tool::Text => {
                    if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                        dbg!("Hit left click on text tool");
                        // Start text
                        if working_text.is_none() {
                            working_text = Some(Text {
                                content: "".to_string(),
                                position: Some(drawing_pos),
                            });
                        }
                        state.mode = Mode::TypingText;
                    }
                }
            },
            Mode::PickingBackgroundColor(color_picker) => {
                if rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                    if !is_clicking_gui(state.mouse_pos, color_picker.bounds_with_slider()) {
                        state.mode = Mode::UsingTool(Tool::Brush);
                    }
                }
            }
            Mode::TypingText => {
                loop {
                    let char_and_key_pressed = get_char_and_key_pressed(&mut rl);
                    let ch = char_and_key_pressed.0;
                    let key = char_and_key_pressed.1;
                    if ch.is_none() && key.is_none() {
                        break;
                    }

                    let key = key.unwrap();

                    if let Some(ch) = ch {
                        append_input_to_working_text(ch, &mut working_text);
                    }

                    if key == KeyboardKey::KEY_ENTER {
                        dbg!("Exiting text tool");
                        if let Some(text) = working_text {
                            if !text.content.is_empty() {
                                state.add_text_with_undo(text);
                            }
                        }

                        working_text = None;
                        state.mode = Mode::UsingTool(Tool::Brush);
                        break;
                    }
                    // TODO: Handle holding in backspace, probably a 'delay' between each removal
                    // if backspace is held
                    if key == KeyboardKey::KEY_BACKSPACE {
                        dbg!("Backspace is down");
                        if let Some(text) = working_text.as_mut() {
                            let _removed_char = text.content.pop();
                        }
                    }
                }
            }
            Mode::ShowingKeymapPanel => {
                if rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                    if !is_clicking_gui(state.mouse_pos, keymap_panel_bounds) {
                        state.mode = Mode::default();
                    }
                }
            }
        }

        if state.mode != Mode::TypingText {
            process_key_pressed_events(&keymap, &mut debugging, &mut rl, &mut brush, &mut state);
            process_key_down_events(
                &keymap,
                screen_width,
                screen_height,
                &mut rl,
                &mut brush,
                &mut state,
                delta_time,
            );
        }

        // TODO: Configurable mouse buttons
        if rl.is_mouse_button_down(MouseButton::MOUSE_MIDDLE_BUTTON) {
            apply_mouse_drag_to_camera(state.mouse_pos, last_mouse_pos, &mut state.camera);
        }

        let mouse_wheel_diff = rl.get_mouse_wheel_move();
        if rl.is_key_up(KeyboardKey::KEY_LEFT_CONTROL) {
            apply_mouse_wheel_zoom(mouse_wheel_diff, &mut state.camera);
        }

        if rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL) {
            apply_mouse_wheel_brush_size(mouse_wheel_diff, &mut brush);
        }

        clamp_brush_size(&mut brush);

        clamp_camera_zoom(&mut state.camera);

        last_mouse_pos = state.mouse_pos;

        let camera_view_boundary = rrect(
            state.camera.offset.x / state.camera.zoom + state.camera.target.x
                - screen_width as f32 / state.camera.zoom,
            state.camera.offset.y / state.camera.zoom + state.camera.target.y
                - (screen_height as f32 / state.camera.zoom),
            screen_width as f32 / state.camera.zoom,
            screen_height as f32 / state.camera.zoom,
        );

        let mut drawing = rl.begin_drawing(&thread);
        {
            let mut drawing_camera = drawing.begin_mode2D(state.camera);

            drawing_camera.clear_background(state.background_color.0);
            for (_, stroke) in &state.strokes {
                if is_stroke_in_camera_view(&camera_view_boundary, stroke) {
                    draw_stroke(&mut drawing_camera, &stroke, stroke.brush_size);
                }
            }
            for (_, text) in &state.text {
                if let Some(pos) = text.position {
                    if camera_view_boundary.check_collision_point_rec(pos) {
                        drawing_camera.draw_text(
                            &text.content,
                            pos.x as i32,
                            pos.y as i32,
                            16,
                            Color::BLACK,
                        );
                    }
                }
            }

            // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
            draw_stroke(
                &mut drawing_camera,
                &working_stroke,
                working_stroke.brush_size,
            );

            if let Some(working_text) = &working_text {
                if let Some(pos) = working_text.position {
                    drawing_camera.draw_text(
                        &working_text.content,
                        pos.x as i32,
                        pos.y as i32,
                        16,
                        Color::BLACK,
                    );
                }
            }

            draw_brush_marker(&mut drawing_camera, drawing_pos, &brush);

            if debugging {
                debug_draw_center_crosshair(
                    &mut drawing_camera,
                    &state,
                    screen_width,
                    screen_height,
                );
            }
        }

        if let Mode::PickingBackgroundColor(color_picker) = state.mode {
            state.background_color.0 =
                drawing.gui_color_picker(color_picker.bounds, state.background_color.0);
        }

        if let Some(picker_info) = &mut brush_color_picker_info {
            // TODO: Scale the GUI?
            if !is_drawing {
                // Hide when not drawing
                current_brush_color =
                    drawing.gui_color_picker(picker_info.bounds, current_brush_color);
            }
            if debugging {
                drawing.draw_rectangle_lines_ex(picker_info.bounds_with_slider(), 1, Color::GOLD);
            }
        }

        if state.mode == Mode::ShowingKeymapPanel {
            // TODO: Maybe just do mode check here
            drawing.gui_group_box(
                keymap_panel_bounds,
                Some(&CString::new("Yo gabba gabba").unwrap()),
            );
        }

        draw_info_ui(&mut drawing, &state, &brush);

        if debugging {
            debug_draw_info(&mut drawing, &state, drawing_pos, current_fps);
        }

        let elapsed = start_time.elapsed();
        if elapsed < duration_per_frame {
            let time_to_sleep = duration_per_frame - elapsed;
            thread::sleep(time_to_sleep);
        }
    }
}

fn apply_mouse_drag_to_camera(mouse_pos: Vector2, last_mouse_pos: Vector2, camera: &mut Camera2D) {
    // TODO(reece): Dragging and drawing can be done together at the moment, but it's very jaggy
    let mouse_diff = mouse_pos - last_mouse_pos;
    camera.target.x -= mouse_diff.x / camera.zoom;
    camera.target.y -= mouse_diff.y / camera.zoom;
}

fn apply_mouse_wheel_zoom(mouse_wheel_diff: f32, camera: &mut Camera2D) {
    let mouse_wheel_zoom_dampening = 0.065;
    // TODO: FIXME: This stuff "works" but it's an awful experience. Seems way worse when the window is a
    // smaller portion of the overall screen size due to scaling
    camera.zoom += mouse_wheel_diff * mouse_wheel_zoom_dampening;
}

fn apply_mouse_wheel_brush_size(mouse_wheel_diff: f32, brush: &mut Brush) {
    let mouse_wheel_amplifying = 3.50;
    brush.brush_size += mouse_wheel_diff * mouse_wheel_amplifying;
}

fn clamp_camera_zoom(camera: &mut Camera2D) {
    if camera.zoom < 0.1 {
        camera.zoom = 0.1;
    }

    if camera.zoom > 10.0 {
        camera.zoom = 10.0;
    }
}

fn clamp_brush_size(brush: &mut Brush) {
    if brush.brush_size < 1.0 {
        brush.brush_size = 1.0;
    }
}

fn is_stroke_in_camera_view(camera_boundary: &Rectangle, stroke: &Stroke) -> bool {
    for point in &stroke.points {
        if camera_boundary.check_collision_point_rec(point) {
            return true;
        }
    }
    return false;
}

#[derive(Debug, Deserialize, Serialize)]
struct Point {
    x: f32,
    y: f32,
}

impl Into<ffi::Vector2> for &Point {
    fn into(self) -> ffi::Vector2 {
        ffi::Vector2 {
            x: self.x,
            y: self.y,
        }
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_str = format!("{},{}", self.x, self.y);
        f.write_str(&display_str)
    }
}

#[derive(Deserialize, Serialize)]
struct Stroke {
    points: Vec<Point>,
    color: Color,
    brush_size: f32,
    // TODO(reece): Could store the brush used in the stroke so we know the parameters of each
    // stroke
}

impl Stroke {
    fn new(color: Color, brush_size: f32) -> Self {
        let default_num_of_points = 30;
        Stroke {
            points: Vec::with_capacity(default_num_of_points),
            color,
            brush_size,
        }
    }
}

// TODO: strokes and stroke_graveyard should have different key types probably
type Strokes = SlotMap<DefaultKey, Stroke>;

new_key_type! { struct TextKey; }
#[derive(Debug, Deserialize, Serialize)]
enum Action {
    AddStroke(DefaultKey),
    RemoveStroke(DefaultKey),
    AddText(TextKey),
    RemoveText(TextKey),
}

#[derive(Debug, Deserialize, Serialize)]
struct Text {
    content: String,
    position: Option<Vector2>,
}

type CameraZoomPercentageDiff = i32;
type DiffPerSecond = i32;

#[derive(Debug, PartialEq, Eq, Hash)]
enum HoldCommand {
    CameraZoom(CameraZoomPercentageDiff),
    PanCameraHorizontal(DiffPerSecond),
    PanCameraVertical(DiffPerSecond),
    ChangeBrushSize(DiffPerSecond),
    SpawnBrushStrokes,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum PressCommand {
    Undo,
    Redo,
    UseTextTool,
    ToggleDebugging,
    Save,
    SaveAs,
    Load,
    ChangeBrushType(BrushType),
    PickBackgroundColor,
    ToggleKeymapWindow,
}

type PressKeyMappings = Vec<(KeyboardKey, PressCommand)>;
type HoldKeyMappings = Vec<(KeyboardKey, HoldCommand)>;

struct Keymap {
    // Does not support key combinations at the moment. Could be Vec<(Vec<KeyboardKey>, Command)>
    // if we wanted
    on_press: PressKeyMappings,
    on_hold: HoldKeyMappings,
}

fn default_keymap() -> Keymap {
    let on_press = PressKeyMappings::from([
        (KeyboardKey::KEY_M, PressCommand::ToggleDebugging),
        (KeyboardKey::KEY_O, PressCommand::Save),
        (KeyboardKey::KEY_I, PressCommand::SaveAs),
        (KeyboardKey::KEY_P, PressCommand::Load),
        (KeyboardKey::KEY_Z, PressCommand::Undo),
        (KeyboardKey::KEY_R, PressCommand::Redo),
        (
            KeyboardKey::KEY_E,
            PressCommand::ChangeBrushType(BrushType::Deleting),
        ),
        (
            KeyboardKey::KEY_Q,
            PressCommand::ChangeBrushType(BrushType::Drawing),
        ),
        (KeyboardKey::KEY_T, PressCommand::UseTextTool),
        (KeyboardKey::KEY_B, PressCommand::PickBackgroundColor),
        (KeyboardKey::KEY_SLASH, PressCommand::ToggleKeymapWindow),
    ]);
    let on_hold = HoldKeyMappings::from([
        (KeyboardKey::KEY_A, HoldCommand::PanCameraHorizontal(-250)),
        (KeyboardKey::KEY_D, HoldCommand::PanCameraHorizontal(250)),
        (KeyboardKey::KEY_S, HoldCommand::PanCameraVertical(250)),
        (KeyboardKey::KEY_W, HoldCommand::PanCameraVertical(-250)),
        (KeyboardKey::KEY_L, HoldCommand::CameraZoom(-5)),
        (KeyboardKey::KEY_K, HoldCommand::CameraZoom(5)),
        (
            KeyboardKey::KEY_LEFT_BRACKET,
            HoldCommand::ChangeBrushSize(-50),
        ),
        (
            KeyboardKey::KEY_RIGHT_BRACKET,
            HoldCommand::ChangeBrushSize(50),
        ),
        (KeyboardKey::KEY_H, HoldCommand::SpawnBrushStrokes),
    ]);

    return Keymap { on_press, on_hold };
}

struct Brush {
    brush_type: BrushType,
    brush_size: f32,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum BrushType {
    Drawing,
    Deleting,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum Tool {
    Brush,
    Text,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Mode {
    UsingTool(Tool),
    PickingBackgroundColor(GuiColorPickerInfo),
    TypingText,
    ShowingKeymapPanel,
}

impl Default for Mode {
    fn default() -> Self {
        Self::UsingTool(Tool::Brush)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct GuiColorPickerInfo {
    initiation_pos: Vector2,
    bounds: Rectangle,
    /// The given bounds for the rgui color picker doesn't include the color slider bar at the
    /// side. Haven't looked too deeply into it, but the slider seems to be the same width
    /// regardless of the size of the color picker.
    picker_slider_x_padding: f32,
}

impl GuiColorPickerInfo {
    /// Returns the bounds of the color picker, including the color slider bar at the side.
    fn bounds_with_slider(&self) -> Rectangle {
        let mut bounds_with_picker = self.bounds;
        bounds_with_picker.width += self.picker_slider_x_padding;
        return bounds_with_picker;
    }
}
