use std::{
    collections::HashMap,
    fmt::Display,
    fs::File,
    io::Write,
    thread,
    time::{self, Instant},
};

use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};

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

type Strokes = Vec<Option<Stroke>>;

#[derive(Deserialize, Serialize)]
struct State {
    strokes: Strokes,
    stroke_graveyard: Vec<Stroke>,
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
#[derive(Debug, PartialEq, Eq, Hash)]
enum Command {
    Save,
    Load,
    ChangeBrushType(BrushType),
    ToggleDebugging,
    PanCameraHorizontal(i32),
    PanCameraVertical(i32),
    Undo,
    Redo,
    ChangeBrushSize(CameraZoomPercentageDiff),
    CameraZoom(CameraZoomPercentageDiff),
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
    ]);
    let on_hold = KeyMappings::from([
        (KeyboardKey::KEY_A, Command::PanCameraHorizontal(-5)),
        (KeyboardKey::KEY_D, Command::PanCameraHorizontal(5)),
        (KeyboardKey::KEY_S, Command::PanCameraVertical(5)),
        (KeyboardKey::KEY_W, Command::PanCameraVertical(-5)),
        // (KeyboardKey::KEY_Z, Command::Save),
        // (KeyboardKey::KEY_R, Command::Save),
        // (KeyboardKey::KEY_E, Command::Save),
        // (KeyboardKey::KEY_Q, Command::Save),
        // (KeyboardKey::KEY_LEFT_BRACKET, Command::Save),
        // (KeyboardKey::KEY_RIGHT_BRACKET, Command::Save),
        (KeyboardKey::KEY_L, Command::CameraZoom(-5)),
        (KeyboardKey::KEY_K, Command::CameraZoom(5)),
    ]);

    return Keymap { on_press, on_hold };
}

struct Brush {
    brush_type: BrushType,
    brush_size: f32,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum BrushType {
    Drawing,
    Deleting,
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

    let strokes: Strokes = Vec::with_capacity(10);
    let stroke_graveyard: Vec<Stroke> = Vec::with_capacity(10);
    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };
    let mut state = State {
        strokes,
        stroke_graveyard,
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
    let mut last_mouse_pos = rl.get_mouse_position();

    while !rl.window_should_close() {
        let current_fps = rl.get_fps();
        // TODO: Hotkey configuration
        // TODO(reece): Have zoom follow the cursor i.e zoom into where the cursor is rather than
        // "top left corner"
        // TODO(reece): Improve how the lines look. Make a line renderer or something?
        // TODO(reece): BUG: Brush marker looks like it's a bit off centre from the mouse cursor
        // TODO(reece): Use shaders for line drawing?
        //
        // TODO(reece): Saving/loading?
        // TODO(reece): Installable so it's searchable as a program
        // TODO(reece): Optimize this so we're not smashing the cpu/gpu whilst doing nothing (only
        // update on user input?)

        let start_time = Instant::now();
        screen_width = rl.get_screen_width();
        screen_height = rl.get_screen_height();
        camera.offset = rvec2(screen_width / 2, screen_height / 2);

        let mouse_pos = rl.get_mouse_position();
        let drawing_pos = rl.get_screen_to_world2D(mouse_pos, camera);

        for (key, command) in keymap.on_press.iter() {
            if rl.is_key_pressed(*key) {
                match command {
                    Command::ToggleDebugging => debugging = !debugging,
                    Command::Save => save(&state).unwrap(),
                    Command::Load => state = load().unwrap(),
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
                    Command::PanCameraHorizontal(diff) => {
                        camera.target.x += *diff as f32;
                    }
                    Command::PanCameraVertical(diff) => {
                        camera.target.y += *diff as f32;
                    }
                    c => todo!(
                        "Unimplemented command, or this isn't meant to be a push command: {:?}",
                        c
                    ),
                }
            }
        }

        if rl.is_key_pressed(KeyboardKey::KEY_Z) {
            undo_stroke(&mut state.strokes, &mut state.stroke_graveyard);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            redo_stroke(&mut state.strokes, &mut state.stroke_graveyard);
        }

        if rl.is_key_pressed(KeyboardKey::KEY_E) {
            brush.brush_type = BrushType::Deleting;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_Q) {
            // TODO(reece): Want to check if a brush stroke is already happening? Could just cut
            // the working stroke off when changing brush type
            brush.brush_type = BrushType::Drawing;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_LEFT_BRACKET) {
            // TODO(reece): Changing brush size mid stroke doesn't affect the stroke. Is this the
            // behaviour we want?
            // TODO(reece): Make the brush size changing delay based rather than just key pressed,
            // so we can change brush sizes faster
            brush.brush_size -= 5.0;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_RIGHT_BRACKET) {
            brush.brush_size += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_H) {
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

                state.strokes.push(Some(generated_stroke));
            }
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            apply_mouse_drag_to_camera(mouse_pos, last_mouse_pos, &mut camera);
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_MIDDLE_BUTTON) {
            delete_stroke(
                &mut state.strokes,
                &mut state.stroke_graveyard,
                &drawing_pos,
                brush.brush_size,
            );
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
            if brush.brush_type == BrushType::Deleting {
                delete_stroke(
                    &mut state.strokes,
                    &mut state.stroke_graveyard,
                    &drawing_pos,
                    brush.brush_size,
                )
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
        if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) {
            // Finished drawing
            if is_drawing {
                state.strokes.push(Some(working_stroke));
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
            for line in &state.strokes {
                if let Some(line) = line {
                    draw_stroke(&mut drawing_camera, &line, line.brush_size);
                }
            }

            // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
            draw_stroke(
                &mut drawing_camera,
                &working_stroke,
                working_stroke.brush_size,
            );

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

        if debugging {
            let target_str = format!("target {:?}", camera.target);
            drawing.draw_text(&target_str, 5, 90, 30, Color::RED);
            let drawing_pos_str = format!("draw pos {:?}", drawing_pos);
            drawing.draw_text(&drawing_pos_str, 5, 120, 30, Color::RED);
            let number_of_strokes_str = format!("Total strokes: {}", state.strokes.len());
            drawing.draw_text(&number_of_strokes_str, 5, 150, 30, Color::RED);
            let fps_str = format!("FPS: {}", current_fps);

            drawing.draw_text(&fps_str, 5, 180, 30, Color::RED);
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

fn undo_stroke(strokes: &mut Strokes, stroke_graveyard: &mut Vec<Stroke>) {
    // @NOTE: This will pop None's off the strokes until we find a Some
    loop {
        if let Some(undone_stroke) = strokes.pop() {
            if let Some(stroke) = undone_stroke {
                stroke_graveyard.push(stroke);
                return;
            } else {
                continue;
            }
        } else {
            break;
        }
    }
}

fn redo_stroke(strokes: &mut Strokes, stroke_graveyard: &mut Vec<Stroke>) {
    // @TODO This can't be a pop since the last stroke could be None. Could loop and pop
    match stroke_graveyard.pop() {
        None => {} // Nothing to undo
        Some(redone_stroke) => strokes.push(Some(redone_stroke)),
    }
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

fn delete_stroke(
    strokes: &mut Strokes,
    stroke_graveyard: &mut Vec<Stroke>,
    mouse_point: &Vector2,
    brush_size: f32,
) {
    // @SPEEDUP: Can we do anything smarter than looping through every single stroke + point when
    // trying to delete?
    for i in 0..strokes.len() {
        let stroke = &mut strokes[i];

        if let Some(stroke_item) = stroke {
            for j in 0..stroke_item.points.len() {
                let point = &stroke_item.points[j];
                // @BUG: Still some weirdness when it comes to deleting small lines it feels like
                if check_collision_circles(
                    Vector2 {
                        x: point.x,
                        y: point.y,
                    },
                    stroke_item.brush_size / 2.0,
                    mouse_point,
                    brush_size / 2.0,
                ) {
                    let deleted_stroke = std::mem::replace(stroke, None).unwrap(); // Pretty sure this can't panic because of the `if let` above
                    stroke_graveyard.push(deleted_stroke);
                    break;
                }
            }
        }
    }
}
