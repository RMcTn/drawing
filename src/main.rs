use std::{
    fmt::Display,
    fs::File,
    io::Write,
    thread,
    time::{self, Instant},
};

use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};
use slotmap::{DefaultKey, SlotMap};

const SAVE_FILENAME: &'static str = "strokes_json.txt";

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

#[derive(Debug, Deserialize, Serialize)]
enum Action {
    AddStroke(DefaultKey),
    RemoveStroke(DefaultKey),
}

#[derive(Deserialize, Serialize)]
struct State {
    strokes: Strokes,
    undo_actions: Vec<Action>,
    redo_actions: Vec<Action>,
    stroke_graveyard: Strokes,
    text: Vec<Text>,
}

impl State {
    /// Adds the stroke to the 'alive' stroke list
    fn add_stroke_with_undo(&mut self, stroke: Stroke) {
        let key = self.add_stroke(stroke);
        self.undo_actions.push(Action::AddStroke(key));
    }

    /// Adds the stroke to the 'alive' stroke list at
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
            return Some(self.strokes.insert(stroke));
        }
        dbg!(
            "Tried to restore stroke with key {} but it couldn't find it",
            key
        );

        None
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
                        dbg!(
                            "Tried to restore stroke with key {} but it couldn't find it",
                            key
                        );
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
    position: Vector2,
}

fn save(state: &State) -> Result<(), std::io::Error> {
    // TODO: FIXME: There's no versioning for save files at the moment
    // so anything new isn't backwards compatible
    let output = serde_json::to_string(&state)?;
    let mut file = File::create(SAVE_FILENAME)?;
    file.write_all(output.as_bytes())?;
    Ok(())
}

fn load() -> Result<State, std::io::Error> {
    let contents = std::fs::read_to_string(SAVE_FILENAME)?;
    let state: State = serde_json::from_str(&contents)?;
    return Ok(state);
}

type CameraZoomPercentageDiff = i32;
type DiffPerSecond = i32;
#[derive(Debug, PartialEq, Eq, Hash)]
enum Command {
    Save,
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

    let mut camera = Camera2D {
        offset: rvec2(screen_width / 2, screen_height / 2),
        target: rvec2(0, 0),
        rotation: 0.0,
        zoom: 1.0,
    };

    let initial_brush_size = 10.0;

    let strokes = SlotMap::new();
    let stroke_graveyard = SlotMap::new();
    let text_things = Vec::with_capacity(10);
    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };
    let mut state = State {
        strokes,
        undo_actions: Vec::new(),
        redo_actions: Vec::new(),
        stroke_graveyard,
        text: text_things,
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
        camera.offset = rvec2(screen_width / 2, screen_height / 2);

        let mouse_pos = rl.get_mouse_position();
        let drawing_pos = rl.get_screen_to_world2D(mouse_pos, camera);

        if current_tool == Tool::Text {
            loop {
                let char_and_key_pressed = get_char_and_key_pressed(&mut rl);
                let ch = char_and_key_pressed.0;
                let key = char_and_key_pressed.1;
                if key.is_none() && ch.is_none() {
                    break;
                }

                if ch.is_some() {
                    if working_text.is_none() {
                        working_text = Some(Text {
                            content: "".to_string(),
                            position: drawing_pos,
                        })
                    }
                    let ch = ch.unwrap();
                    let ch = char::from_u32(ch as u32);
                    match ch {
                        Some(c) => working_text.as_mut().unwrap().content.push(c), // Was a safe
                        // unwrap at the time
                        None => (), // TODO: FIXME: Some sort of logging/let the user know for
                                    // unrepresentable character?
                    }
                }

                if let Some(key) = key {
                    if key == KeyboardKey::KEY_ENTER {
                        dbg!("Exiting text tool");
                        current_tool = Tool::Brush;
                        // TODO: Don't save the text if the content is empty
                        state.text.push(working_text.unwrap());
                        working_text = None;
                    }
                    // TODO: Handle holding in backspace
                    if key == KeyboardKey::KEY_BACKSPACE {
                        dbg!("Backspace is down");
                        if let Some(text) = working_text.as_mut() {
                            let _removed_char = text.content.pop();
                        }
                    }
                }
            }
        }

        if current_tool != Tool::Text {
            for (key, command) in keymap.on_press.iter() {
                if rl.is_key_pressed(*key) {
                    match command {
                        Command::ToggleDebugging => debugging = !debugging,
                        Command::Save => save(&state).unwrap(),
                        Command::Load => state = load().unwrap(),
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
                            current_tool = Tool::Text;
                            // TODO: Exit text mode without 'saving'
                        }

                        c => todo!(
                        "Unimplemented command, or this isn't meant to be a press command: {:?}",
                        c
                    ),
                    }
                }
            }
            for (key, command) in keymap.on_hold.iter() {
                if rl.is_key_down(*key) {
                    match command {
                        Command::CameraZoom(percentage_diff) => {
                            // NOTE: There will be rounding errors here, but we can format the zoom
                            // string
                            camera.zoom += *percentage_diff as f32 / 100.0;
                        }
                        Command::PanCameraHorizontal(diff_per_sec) => {
                            camera.target.x += *diff_per_sec as f32 * delta_time;
                        }
                        Command::PanCameraVertical(diff_per_sec) => {
                            camera.target.y += *diff_per_sec as f32 * delta_time;
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

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            apply_mouse_drag_to_camera(mouse_pos, last_mouse_pos, &mut camera);
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
            apply_mouse_wheel_zoom(mouse_wheel_diff, &mut camera);
        }

        if rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL) {
            apply_mouse_wheel_brush_size(mouse_wheel_diff, &mut brush);
        }

        clamp_brush_size(&mut brush);

        clamp_camera_zoom(&mut camera);

        last_mouse_pos = mouse_pos;

        let mut drawing = rl.begin_drawing(&thread);
        {
            let mut drawing_camera = drawing.begin_mode2D(camera);

            drawing_camera.clear_background(Color::WHITE);
            for (_, stroke) in &state.strokes {
                draw_stroke(&mut drawing_camera, &stroke, stroke.brush_size);
            }
            for text in &state.text {
                drawing_camera.draw_text(
                    &text.content,
                    text.position.x as i32,
                    text.position.y as i32,
                    16,
                    Color::BLACK,
                );
            }

            // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
            draw_stroke(
                &mut drawing_camera,
                &working_stroke,
                working_stroke.brush_size,
            );

            if let Some(working_text) = &working_text {
                drawing_camera.draw_text(
                    &working_text.content,
                    working_text.position.x as i32,
                    working_text.position.y as i32,
                    16,
                    Color::BLACK,
                );
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
                    rvec2((-screen_width * 10) as f32, camera.target.y),
                    rvec2((screen_width * 10) as f32, camera.target.y),
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
        let zoom_str = format!("Zoom: {:.2}", camera.zoom);
        drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
        drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
        drawing.draw_text(&zoom_str, 5, 60, 30, Color::RED);

        let tool_str = format!("Tool: {:?}", current_tool);
        drawing.draw_text(&tool_str, 5, 90, 30, Color::RED);

        if debugging {
            let target_str = format!("target {:?}", camera.target);
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

fn draw_stroke(drawing: &mut RaylibMode2D<RaylibDrawHandle>, stroke: &Stroke, brush_size: f32) {
    if stroke.points.len() == 0 {
        return;
    }
    for i in 0..stroke.points.len() - 1 {
        let point = &stroke.points[i];
        let next_point = &stroke.points[i + 1];

        let first_vec = Vector2 {
            x: point.x,
            y: point.y,
        };
        let last_vec = Vector2 {
            x: next_point.x,
            y: next_point.y,
        };

        // We're drawing the line + circle here just cause it looks a bit better (circle hides the blockiness of the line)
        drawing.draw_line_ex(first_vec, last_vec, brush_size, stroke.color);
        // Half the brush size here since draw call wants radius
        drawing.draw_circle_v(last_vec, brush_size / 2.0, stroke.color); // @SPEEDUP This is slow as fuck
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
    let mouse_wheel_amplifying = 1.50;
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

/// A key press and a char press are treated differently in Raylib it looks like.
/// Key presses are always uppercase (i.e 'a' will be KEY_A, so will 'A').
/// Char presses are the individual characters that have been pressed, so can differentiate between
/// uppercase and lowercase (same with symbols)
fn get_char_and_key_pressed(raylib: &mut RaylibHandle) -> (Option<i32>, Option<KeyboardKey>) {
    let char_pressed = unsafe { raylib::ffi::GetCharPressed() };

    let key_pressed = raylib.get_key_pressed();

    if char_pressed == 0 {
        return (None, key_pressed);
    }

    return (Some(char_pressed), key_pressed);
}
