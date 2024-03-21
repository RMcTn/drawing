use std::{
    fmt::Display,
    path::PathBuf,
    thread,
    time::{self, Instant},
};

use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, DefaultKey, SlotMap};

use crate::input::append_input_to_working_text;
use input::{get_char_and_key_pressed, process_key_down_events, process_key_pressed_events};
use render::draw_stroke;

mod input;
mod persistence;
mod render;

#[derive(Debug, Deserialize, Serialize)]
struct Point {
    x: f32,
    y: f32,
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

#[derive(Deserialize, Serialize)]
struct State {
    strokes: Strokes,
    undo_actions: Vec<Action>,
    redo_actions: Vec<Action>,
    stroke_graveyard: Strokes,
    text: SlotMap<TextKey, Text>,
    text_graveyard: SlotMap<TextKey, Text>,
    output_path: Option<PathBuf>,
    #[serde(with = "Camera2DDef")]
    #[serde(default)]
    camera: Camera2D,
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Camera2D")]
struct Camera2DDef {
    offset: Vector2,
    target: Vector2,
    rotation: f32,
    zoom: f32,
}

impl State {
    fn add_stroke_with_undo(&mut self, stroke: Stroke) {
        let key = self.add_stroke(stroke);
        self.undo_actions.push(Action::AddStroke(key));
    }

    fn add_stroke(&mut self, stroke: Stroke) -> DefaultKey {
        self.strokes.insert(stroke)
    }

    fn remove_stroke(&mut self, key: DefaultKey) -> Option<DefaultKey> {
        if let Some(stroke) = self.strokes.remove(key) {
            return Some(self.add_stroke_to_graveyard(stroke));
        }
        dbg!(
            "Tried to remove stroke with key {} but it was already gone",
            key
        );

        None
    }

    fn add_stroke_to_graveyard(&mut self, stroke: Stroke) -> DefaultKey {
        self.stroke_graveyard.insert(stroke)
    }

    fn restore_stroke(&mut self, key: DefaultKey) -> Option<DefaultKey> {
        if let Some(stroke) = self.stroke_graveyard.remove(key) {
            return Some(self.add_stroke(stroke));
        }
        dbg!(
            "Tried to restore stroke with key {} but it couldn't find it",
            key
        );

        None
    }

    fn add_text_with_undo(&mut self, text: Text) {
        let key = self.add_text(text);
        self.undo_actions.push(Action::AddText(key));
    }

    fn add_text(&mut self, text: Text) -> TextKey {
        self.text.insert(text)
    }

    fn restore_text(&mut self, key: TextKey) -> Option<TextKey> {
        if let Some(text) = self.text_graveyard.remove(key) {
            return Some(self.add_text(text));
        }
        dbg!(
            "Tried to restore text with key {} but it couldn't find it",
            key
        );

        None
    }

    fn remove_text(&mut self, key: TextKey) -> Option<TextKey> {
        if let Some(text) = self.text.remove(key) {
            return Some(self.add_text_to_graveyard(text));
        }
        dbg!(
            "Tried to remove text with key {} but it was already gone",
            key
        );

        None
    }

    fn add_text_to_graveyard(&mut self, text: Text) -> TextKey {
        self.text_graveyard.insert(text)
    }

    fn undo(&mut self) {
        loop {
            if let Some(action) = self.undo_actions.pop() {
                match action {
                    Action::AddStroke(key) => {
                        if let Some(new_key) = self.remove_stroke(key) {
                            self.redo_actions.push(Action::AddStroke(new_key));
                            break;
                        }
                    }
                    Action::RemoveStroke(key) => {
                        if let Some(new_key) = self.restore_stroke(key) {
                            self.redo_actions.push(Action::RemoveStroke(new_key));
                            break;
                        }
                    }
                    Action::AddText(key) => {
                        if let Some(new_key) = self.remove_text(key) {
                            self.redo_actions.push(Action::AddText(new_key));
                            break;
                        }
                    }
                    Action::RemoveText(key) => {
                        if let Some(new_key) = self.restore_text(key) {
                            self.redo_actions.push(Action::RemoveText(new_key));
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    fn redo(&mut self) {
        loop {
            if let Some(action) = self.redo_actions.pop() {
                match action {
                    Action::AddStroke(key) => {
                        if let Some(new_key) = self.restore_stroke(key) {
                            self.undo_actions.push(Action::AddStroke(new_key));
                            break;
                        }
                    }
                    Action::RemoveStroke(key) => {
                        if let Some(new_key) = self.remove_stroke(key) {
                            self.undo_actions.push(Action::RemoveStroke(new_key));
                            break;
                        }
                    }
                    Action::AddText(key) => {
                        if let Some(new_key) = self.restore_text(key) {
                            self.undo_actions.push(Action::AddText(new_key));
                            break;
                        }
                    }
                    Action::RemoveText(key) => {
                        if let Some(new_key) = self.remove_text(key) {
                            self.undo_actions.push(Action::RemoveText(new_key));
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    fn strokes_within_point(&self, mouse_point: Vector2, brush_size: f32) -> Vec<DefaultKey> {
        let mut strokes = vec![];
        for (k, stroke) in &self.strokes {
            for point in &stroke.points {
                if check_collision_circles(
                    Vector2 {
                        x: point.x,
                        y: point.y,
                    },
                    stroke.brush_size / 2.0,
                    mouse_point,
                    brush_size / 2.0,
                ) {
                    strokes.push(k);
                    break;
                }
            }
        }
        strokes
    }

    fn delete_strokes(&mut self, stroke_keys: Vec<DefaultKey>) {
        for key in stroke_keys {
            if let Some(new_key) = self.remove_stroke(key) {
                self.undo_actions.push(Action::RemoveStroke(new_key));
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Text {
    content: String,
    position: Option<Vector2>,
}

type CameraZoomPercentageDiff = i32;
type DiffPerSecond = i32;

#[derive(Debug, PartialEq, Eq, Hash)]
enum Command {
    Save,
    SaveAs,
    Load,
    ChangeBrushType(BrushType),
    ToggleDebugging,
    PanCameraHorizontal(DiffPerSecond),
    PanCameraVertical(DiffPerSecond),
    Undo,
    Redo,
    ChangeBrushSize(DiffPerSecond),
    CameraZoom(CameraZoomPercentageDiff),
    SpawnBrushStrokes,
    UseTextTool,
}

type KeyMappings = Vec<(KeyboardKey, Command)>;

struct Keymap {
    // Does not support key combinations at the moment. Could be Vec<(Vec<KeyboardKey>, Command)>
    // if we wanted
    on_press: KeyMappings,
    on_hold: KeyMappings,
}

fn default_keymap() -> Keymap {
    let on_press = KeyMappings::from([
        (KeyboardKey::KEY_M, Command::ToggleDebugging),
        (KeyboardKey::KEY_O, Command::Save),
        (KeyboardKey::KEY_I, Command::SaveAs),
        (KeyboardKey::KEY_P, Command::Load),
        (KeyboardKey::KEY_Z, Command::Undo),
        (KeyboardKey::KEY_R, Command::Redo),
        (
            KeyboardKey::KEY_E,
            Command::ChangeBrushType(BrushType::Deleting),
        ),
        (
            KeyboardKey::KEY_Q,
            Command::ChangeBrushType(BrushType::Drawing),
        ),
        (KeyboardKey::KEY_T, Command::UseTextTool),
    ]);
    let on_hold = KeyMappings::from([
        (KeyboardKey::KEY_A, Command::PanCameraHorizontal(-250)),
        (KeyboardKey::KEY_D, Command::PanCameraHorizontal(250)),
        (KeyboardKey::KEY_S, Command::PanCameraVertical(250)),
        (KeyboardKey::KEY_W, Command::PanCameraVertical(-250)),
        (KeyboardKey::KEY_L, Command::CameraZoom(-5)),
        (KeyboardKey::KEY_K, Command::CameraZoom(5)),
        (KeyboardKey::KEY_LEFT_BRACKET, Command::ChangeBrushSize(-50)),
        (KeyboardKey::KEY_RIGHT_BRACKET, Command::ChangeBrushSize(50)),
        (KeyboardKey::KEY_H, Command::SpawnBrushStrokes),
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

    // rl.set_target_fps(60);

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
    let mut state = State {
        strokes: SlotMap::new(),
        undo_actions: Vec::new(),
        redo_actions: Vec::new(),
        stroke_graveyard: SlotMap::new(),
        text: SlotMap::with_key(),
        text_graveyard: SlotMap::with_key(),
        output_path: None,
        camera,
    };
    let mut current_tool = Tool::Brush;

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
    let mut working_text: Option<Text> = None;
    let mut last_mouse_pos = rl.get_mouse_position();

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
        // TODO: Only draw strokes that are visible in the camera (if this isn't already happening)

        let start_time = Instant::now();
        screen_width = rl.get_screen_width();
        screen_height = rl.get_screen_height();
        state.camera.offset = rvec2(screen_width / 2, screen_height / 2);

        let mouse_pos = rl.get_mouse_position();
        let drawing_pos = rl.get_screen_to_world2D(mouse_pos, state.camera);

        if current_tool == Tool::Text {
            loop {
                let char_and_key_pressed = get_char_and_key_pressed(&mut rl);
                let ch = char_and_key_pressed.0;
                let key = char_and_key_pressed.1;
                if ch.is_none() && key.is_none() {
                    break;
                }

                let ch = ch.unwrap();
                let key = key.unwrap();

                append_input_to_working_text(ch, &mut working_text);

                if key == KeyboardKey::KEY_ENTER {
                    dbg!("Exiting text tool");
                    current_tool = Tool::Brush;
                    if let Some(text) = working_text {
                        if !text.content.is_empty() {
                            state.add_text_with_undo(text);
                        }
                    }

                    working_text = None;
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

        if current_tool != Tool::Text {
            process_key_pressed_events(
                &keymap,
                &mut debugging,
                &mut rl,
                &mut brush,
                &mut state,
                &mut current_tool,
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

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            apply_mouse_drag_to_camera(mouse_pos, last_mouse_pos, &mut state.camera);
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_MIDDLE_BUTTON) && current_tool == Tool::Brush
        {
            let strokes_to_delete = state.strokes_within_point(drawing_pos, brush.brush_size);
            state.delete_strokes(strokes_to_delete);
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) && current_tool == Tool::Brush {
            if brush.brush_type == BrushType::Deleting {
                let strokes_to_delete = state.strokes_within_point(drawing_pos, brush.brush_size);
                state.delete_strokes(strokes_to_delete);
            } else {
                // Drawing
                if !is_drawing {
                    let brush_color = match &brush.brush_type {
                        // TODO(reece): Will want these colours to be dynamic (whatever user
                        // picked (drawing)/whatever bg colour is (erasing))
                        BrushType::Drawing => Color::BLACK,
                        BrushType::Deleting => Color::RED,
                    };

                    working_stroke = Stroke::new(brush_color, brush.brush_size);
                    is_drawing = true;
                }

                let point = Point {
                    x: drawing_pos.x,
                    y: drawing_pos.y,
                };
                working_stroke.points.push(point);
            }
        }
        if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) && current_tool == Tool::Brush {
            // Finished drawing
            // TODO: FIXME: Do not allow text tool if currently drawing, otherwise we won't be able to end
            // the brush stroke unless we change back to brush mode
            if is_drawing {
                state.add_stroke_with_undo(working_stroke);
                working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
            }
            is_drawing = false;
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

        last_mouse_pos = mouse_pos;

        let mut drawing = rl.begin_drawing(&thread);
        {
            let mut drawing_camera = drawing.begin_mode2D(state.camera);

            drawing_camera.clear_background(Color::WHITE);
            for (_, stroke) in &state.strokes {
                draw_stroke(&mut drawing_camera, &stroke, stroke.brush_size);
            }
            for (_, text) in &state.text {
                if let Some(pos) = text.position {
                    drawing_camera.draw_text(
                        &text.content,
                        pos.x as i32,
                        pos.y as i32,
                        16,
                        Color::BLACK,
                    );
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

            // Our brush marker
            drawing_camera.draw_circle_lines(
                drawing_pos.x as i32,
                drawing_pos.y as i32,
                // Draw circle wants radius
                brush.brush_size / 2.0,
                Color::BLACK,
            );

            if debugging {
                drawing_camera.draw_line_ex(
                    rvec2(camera.target.x, (-screen_height * 10) as f32),
                    rvec2(camera.target.x, (screen_height * 10) as f32),
                    5.0,
                    Color::PURPLE,
                );
                drawing_camera.draw_line_ex(
                    rvec2((-screen_width * 10) as f32, state.camera.target.y),
                    rvec2((screen_width * 10) as f32, state.camera.target.y),
                    5.0,
                    Color::PURPLE,
                );
            }
        }

        let brush_type_str = match &brush.brush_type {
            BrushType::Drawing => "Drawing",
            BrushType::Deleting => "Deleting",
        };
        let brush_size_str = format!("Brush size: {}", brush.brush_size.to_string());
        let zoom_str = format!("Zoom: {:.2}", state.camera.zoom);
        drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
        drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
        drawing.draw_text(&zoom_str, 5, 60, 30, Color::RED);

        let tool_str = format!("Tool: {:?}", current_tool);
        drawing.draw_text(&tool_str, 5, 90, 30, Color::RED);

        if debugging {
            let target_str = format!("target {:?}", state.camera.target);
            drawing.draw_text(&target_str, 5, 120, 30, Color::RED);
            let drawing_pos_str = format!("draw pos {:?}", drawing_pos);
            drawing.draw_text(&drawing_pos_str, 5, 150, 30, Color::RED);
            let number_of_strokes_str = format!("Total strokes: {}", state.strokes.len());
            drawing.draw_text(&number_of_strokes_str, 5, 180, 30, Color::RED);
            let fps_str = format!("FPS: {}", current_fps);
            drawing.draw_text(&fps_str, 5, 210, 30, Color::RED);
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
    // This stuff "works" but it's an awful experience. Seems way worse when the window is a
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
