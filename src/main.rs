use std::{
    collections::HashMap,
    fmt::Display,
    thread,
    time::{self, Duration, Instant},
};

use gui::{
    debug_draw_center_crosshair, draw_color_dropper_icon, draw_color_dropper_preview, draw_info_ui,
    draw_keymap, is_clicking_gui,
};
use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, DefaultKey, SlotMap};

use crate::{gui::debug_draw_info, input::append_input_to_working_text};
use input::{get_char_pressed, process_key_down_events, process_key_pressed_events};
use render::{draw_brush_marker, draw_stroke};
use state::{ForegroundColor, State};

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

    let color_picker_scaling_factor = 4; // TODO: Make other GUI things scalable.
                                         // TODO: Configurable scaling

    let color_dropper_icon_bytes = include_bytes!("../assets/color-dropper.png").to_vec();
    let color_dropper_icon_image = Image::load_image_from_mem(
        ".png",
        &color_dropper_icon_bytes,
        color_dropper_icon_bytes.len() as i32,
    )
    .expect("Couldn't create color dropper icon from packaged color dropper image");
    let color_dropper_icon = rl
        .load_texture_from_image(&thread, &color_dropper_icon_image)
        .expect("Couldn't find color dropper icon file");
    let color_dropper_width = color_dropper_icon.width(); // REFACTOR: Will want something similar
                                                          // for other tool icons
    let color_dropper_height = color_dropper_icon.height();
    let color_dropper_scaled_width = color_dropper_width * color_picker_scaling_factor;
    let color_dropper_scaled_height = color_dropper_height * color_picker_scaling_factor;
    let color_dropper_source_rect = rrect(0, 0, color_dropper_width, color_dropper_height);

    let target_fps = 60;
    let seconds_per_frame = 1.0 / target_fps as f32;
    let duration_per_frame = time::Duration::from_secs_f32(seconds_per_frame);

    let camera = Camera2D {
        offset: rvec2(screen_width / 2, screen_height / 2),
        target: rvec2(0, 0),
        rotation: 0.0,
        zoom: 1.0,
    };

    let outline_color = Color::BLACK; // TODO: Using black as a stand in until we do something that
                                      // reacts to the background color

    let initial_brush_size = 10.0;

    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };

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
        foreground_color: Default::default(),
        mode: Mode::UsingTool(Tool::Brush),
        mouse_pos: rvec2(0, 0),
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(ForegroundColor::default().0, brush.brush_size);
    let mut working_text: Option<Text> = None;
    let mut last_mouse_pos = rl.get_mouse_position();

    let mut brush_color_picker_info: Option<GuiColorPickerInfo> = None;

    let font = rl.get_font_default();

    let font_size = 20.0; // TODO: Make user configurable

    let mut time_since_last_text_deletion = Duration::ZERO;
    let delay_between_text_deletions = Duration::from_millis(100); // TODO: Make user configurable

    let mut processed_press_commands: HashMap<PressCommand, bool> = keymap
        .on_press
        .iter()
        .map(|entry| (entry.1, false))
        .collect();
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

        time_since_last_text_deletion += Duration::from_secs_f32(delta_time);

        let start_time = Instant::now();
        screen_width = rl.get_screen_width();
        screen_height = rl.get_screen_height();
        state.camera.offset = rvec2(screen_width / 2, screen_height / 2);

        state.mouse_pos = rl.get_mouse_position();
        let drawing_pos = rl.get_screen_to_world2D(state.mouse_pos, state.camera);

        let keymap_panel_padding_percent = 0.10;
        let keymap_panel_padding_x = screen_width as f32 * keymap_panel_padding_percent;
        let keymap_panel_padding_y = screen_height as f32 * keymap_panel_padding_percent;
        let keymap_panel_bounds = rrect(
            keymap_panel_padding_x,
            keymap_panel_padding_y,
            screen_width as f32 - (keymap_panel_padding_x * 2.0),
            screen_height as f32 - (keymap_panel_padding_y * 2.0),
        );

        let screen = rl.get_screen_data(&thread);

        // NOTE: Make sure any icons we don't want interfering with this color have a transparent
        // pixel at the mouse pos (or draw it away from the mouse pos a bit)
        let pixel_color_at_mouse_pos =
            // Give a little wiggle room when moving off the edges of the window, stops a crash :)
            screen.get_image_data()[state.mouse_pos.y.clamp(0.0, (screen_height - 1) as f32)
                as usize
                * screen_width as usize
                + state.mouse_pos.x.clamp(0.0, (screen_width - 1) as f32) as usize];

        match state.mode {
            Mode::UsingTool(tool) => match tool {
                Tool::Brush => {
                    // TODO: FIXME: Quite easy to accidentally draw when coming out of background
                    // color picker - Maybe a little delay before drawing after clicking off the
                    // picker?
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
                                        Stroke::new(state.foreground_color.0, brush.brush_size);
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
                            working_stroke =
                                Stroke::new(state.foreground_color.0, brush.brush_size);
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
                Tool::ColorPicker => {
                    if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                        // NOTE: This literally is whatever color is at the screen. This includes
                        // GUI elements! If it gets annoying enough, it can be changed, but this
                        // was simpler
                        state.foreground_color.0 = pixel_color_at_mouse_pos;

                        // TODO: Text colour picking as well
                        state.mode = Mode::UsingTool(Tool::Brush);
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
            Mode::TypingText => loop {
                if rl.is_key_down(KeyboardKey::KEY_BACKSPACE) {
                    if time_since_last_text_deletion >= delay_between_text_deletions {
                        if let Some(text) = working_text.as_mut() {
                            let _removed_char = text.content.pop();
                        }
                        time_since_last_text_deletion = Duration::ZERO;
                    }
                }

                if rl.is_key_pressed(KeyboardKey::KEY_ENTER) {
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

                let char_pressed = get_char_pressed();

                match char_pressed {
                    Some(ch) => append_input_to_working_text(ch, &mut working_text),
                    None => break,
                }
            },
            Mode::ShowingKeymapPanel => {
                if rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                    if !is_clicking_gui(state.mouse_pos, keymap_panel_bounds) {
                        state.mode = Mode::default();
                    }
                }
            }
        }

        if state.mode != Mode::TypingText {
            // TODO: FIXME: If these keymaps share keys (like S to move the camera, and ctrl + S to
            // save), then both will actions be triggered. Haven't thought about how to handle
            // that yet
            process_key_pressed_events(
                &keymap,
                &mut debugging,
                &mut rl,
                &mut brush,
                &mut state,
                &mut processed_press_commands,
            );
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

            // Draw "world space" GUI elements for the current mode
            if should_show_brush_marker(state.mode) {
                draw_brush_marker(&mut drawing_camera, drawing_pos, &brush);
            }

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

            if debugging {
                debug_draw_center_crosshair(
                    &mut drawing_camera,
                    &state,
                    screen_width,
                    screen_height,
                );
            }
        }

        // Draw non "world space" GUI elements for the current mode
        match state.mode {
            Mode::UsingTool(Tool::ColorPicker) => {
                draw_color_dropper_preview(
                    &mut drawing,
                    state.mouse_pos,
                    screen_height,
                    outline_color,
                    pixel_color_at_mouse_pos,
                );

                draw_color_dropper_icon(
                    &mut drawing,
                    state.mouse_pos,
                    color_dropper_scaled_width,
                    color_dropper_scaled_height,
                    &color_dropper_icon,
                    color_dropper_source_rect,
                );
            }
            Mode::PickingBackgroundColor(_) => {}
            Mode::TypingText => {}
            Mode::ShowingKeymapPanel => {}
            Mode::UsingTool(_) => {}
        }

        if let Mode::PickingBackgroundColor(color_picker) = state.mode {
            state.background_color.0 =
                drawing.gui_color_picker(color_picker.bounds, state.background_color.0);
        }

        if let Some(picker_info) = &mut brush_color_picker_info {
            // TODO: Scale the GUI?
            if !is_drawing {
                // Hide when not drawing
                state.foreground_color.0 =
                    drawing.gui_color_picker(picker_info.bounds, state.foreground_color.0);
            }
            if debugging {
                drawing.draw_rectangle_lines_ex(picker_info.bounds_with_slider(), 1, Color::GOLD);
            }
        }

        if state.mode == Mode::ShowingKeymapPanel {
            let letter_spacing = 4.0;
            draw_keymap(
                &mut drawing,
                &keymap,
                keymap_panel_bounds,
                &font,
                font_size,
                letter_spacing,
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

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
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
    UseColorPicker,
}

type KeyboardKeyCombo = Vec<KeyboardKey>;
type PressKeyMappings = Vec<(KeyboardKeyCombo, PressCommand)>;
type HoldKeyMappings = Vec<(KeyboardKey, HoldCommand)>;

struct Keymap {
    on_press: PressKeyMappings,
    on_hold: HoldKeyMappings,
}

fn default_keymap() -> Keymap {
    let on_press = PressKeyMappings::from([
        (vec![KeyboardKey::KEY_M], PressCommand::ToggleDebugging),
        (
            vec![KeyboardKey::KEY_S, KeyboardKey::KEY_LEFT_CONTROL],
            PressCommand::Save,
        ),
        (
            vec![
                KeyboardKey::KEY_S,
                KeyboardKey::KEY_LEFT_CONTROL,
                KeyboardKey::KEY_LEFT_ALT,
            ],
            PressCommand::SaveAs,
        ),
        (
            vec![KeyboardKey::KEY_O, KeyboardKey::KEY_LEFT_CONTROL],
            PressCommand::Load,
        ),
        (vec![KeyboardKey::KEY_Z], PressCommand::Undo),
        (vec![KeyboardKey::KEY_R], PressCommand::Redo),
        (
            vec![KeyboardKey::KEY_E],
            PressCommand::ChangeBrushType(BrushType::Deleting),
        ),
        (
            vec![KeyboardKey::KEY_Q],
            PressCommand::ChangeBrushType(BrushType::Drawing),
        ),
        (vec![KeyboardKey::KEY_T], PressCommand::UseTextTool),
        (vec![KeyboardKey::KEY_B], PressCommand::PickBackgroundColor),
        (
            vec![KeyboardKey::KEY_SLASH],
            PressCommand::ToggleKeymapWindow,
        ),
        (vec![KeyboardKey::KEY_C], PressCommand::UseColorPicker),
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
    ColorPicker,
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
fn should_show_brush_marker(mode: Mode) -> bool {
    match mode {
        Mode::UsingTool(Tool::Brush) => true,
        Mode::ShowingKeymapPanel => true,
        _ => false,
    }
}
