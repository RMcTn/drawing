use std::{
    fmt::{format, Display},
    fs::File,
    io::{self, BufRead, Write},
    thread,
    time::{self, Instant},
};

use raylib::prelude::{Vector2, *};

#[derive(Debug)]
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

enum ParsingMode {
    Start,
    Stroke,
    AfterStroke,
}

fn chomp_trailing_empty_lines(lines: &mut Vec<&str>) {
    let mut lines_to_keep = lines.len();
    for line in lines.iter().rev() {
        if line.is_empty() {
            lines_to_keep -= 1;
        } else {
            break;
        }
    }
    lines.truncate(lines_to_keep);
}

fn read_from_file() -> Result<Strokes, std::io::Error> {
    // TODO(reece): What to do when parsing fails?
    // TODO(reece): Read in stroke_graveyard
    let contents = std::fs::read_to_string("strokes.txt")?;
    let mut lines: Vec<&str> = contents.split('\n').collect();

    chomp_trailing_empty_lines(&mut lines);

    let mut strokes = Strokes::new();
    let mut parsing_mode = ParsingMode::Start;

    let mut current_line_idx = 0;

    while current_line_idx < lines.len() {
        let current_line = lines[current_line_idx];
        match parsing_mode {
            ParsingMode::Start => {
                if current_line == "strokes" {
                    parsing_mode = ParsingMode::Stroke;
                    current_line_idx += 1;
                    continue;
                } else {
                    let error_msg = format!("Wasn't expecting {} whilst parsing", current_line);
                    eprintln!("{}", error_msg);
                    return Err(io::Error::new(io::ErrorKind::Other, error_msg));
                }
            }
            ParsingMode::AfterStroke => {
                if current_line.trim().is_empty() {
                    parsing_mode = ParsingMode::Stroke;
                    current_line_idx += 1;
                    continue;
                } else {
                    let error_msg =
                        format!("Wasn't expecting {} after parsing a Stroke", current_line);
                    eprintln!("{}", error_msg);
                    return Err(io::Error::new(io::ErrorKind::Other, error_msg));
                }
            }
            ParsingMode::Stroke => {
                let brush_size = current_line.parse().unwrap();
                current_line_idx += 1;

                let rgba_splits: Vec<&str> = lines[current_line_idx].split_whitespace().collect();
                if rgba_splits.len() != 4 {
                    // TODO(reece) proper error handling here
                    let error_msg = format!("Was expecting 4 values at line {}", current_line_idx);
                    eprintln!("{}", error_msg);
                    return Err(io::Error::new(io::ErrorKind::Other, error_msg));
                }

                let r = rgba_splits.get(0).unwrap().parse().unwrap();
                let g = rgba_splits.get(1).unwrap().parse().unwrap();
                let b = rgba_splits.get(2).unwrap().parse().unwrap();
                let a = rgba_splits.get(3).unwrap().parse().unwrap();

                let color = Color { r, g, b, a };
                current_line_idx += 1;

                let points_to_parse: usize = lines[current_line_idx].parse().unwrap();
                current_line_idx += 1;

                let mut points = Vec::with_capacity(points_to_parse);

                for _ in current_line_idx..points_to_parse + current_line_idx {
                    let line = lines[current_line_idx];

                    let split: Vec<&str> = line.split(",").collect();
                    if split.len() != 2 {
                        // TODO(reece) proper error handling here
                        let error_msg =
                            format!("Was expecting only 2 points at line {}", current_line_idx);
                        eprintln!("{}", error_msg);
                        return Err(io::Error::new(io::ErrorKind::Other, error_msg));
                    }
                    let x = split.first().unwrap().parse().unwrap();
                    let y = split.last().unwrap().parse().unwrap();
                    let point = Point { x, y };
                    points.push(point);
                    current_line_idx += 1;
                }

                let stroke = Stroke {
                    brush_size,
                    color,
                    points,
                };
                strokes.push(Some(stroke));
                parsing_mode = ParsingMode::AfterStroke;
            }
        }
    }

    return Ok(strokes);
}

fn write_to_file(strokes: &Strokes, stroke_graveyard: &Vec<Stroke>) -> Result<(), std::io::Error> {
    // Could have used serde, but learning is fun (:

    // TODO(reece): Do we want to do file versions as well? Not to concerned considering this is
    // just a personal program
    let mut file = File::create("strokes.txt")?;

    writeln!(file, "strokes")?;
    for stroke in strokes {
        if stroke.is_none() {
            continue;
        }

        let stroke = stroke.as_ref().unwrap();
        writeln!(file, "{}", stroke.brush_size)?;
        writeln!(
            file,
            "{} {} {} {}",
            stroke.color.r, stroke.color.g, stroke.color.b, stroke.color.a
        )?;
        writeln!(file, "{}", stroke.points.len())?;
        for point in &stroke.points {
            writeln!(file, "{}", point)?;
        }

        writeln!(file, "")?;
    }
    // TODO(reece): Write out stroke_graveyard
    Ok(())
}

struct Brush {
    brush_type: BrushType,
    brush_size: f32,
}

#[derive(PartialEq)]
enum BrushType {
    Drawing,
    // TODO(reece): Does it make sense any more to have erasing and deleting being different
    // things? Is erasing really useful?
    // Erasing here being just using the background colour (or white at the moment) to paint,
    // whereas Deleting is really delete it
    Erasing,
    Deleting,
}

fn main() {
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

    let mut strokes: Strokes = Vec::with_capacity(10);
    let mut stroke_graveyard: Vec<Stroke> = Vec::with_capacity(10);
    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
    let mut last_mouse_pos = rl.get_mouse_position();

    while !rl.window_should_close() {
        let current_fps = rl.get_fps();
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

        if rl.is_key_pressed(KeyboardKey::KEY_M) {
            debugging = !debugging;
        }

        if rl.is_key_down(KeyboardKey::KEY_A) {
            camera.target.x -= 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_D) {
            camera.target.x += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_S) {
            camera.target.y += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_W) {
            camera.target.y -= 5.0;
        }

        if rl.is_key_down(KeyboardKey::KEY_O) {
            write_to_file(&strokes, &stroke_graveyard).unwrap();
        }

        if rl.is_key_down(KeyboardKey::KEY_P) {
            strokes = read_from_file().unwrap();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_Z) {
            undo_stroke(&mut strokes, &mut stroke_graveyard);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            redo_stroke(&mut strokes, &mut stroke_graveyard);
        }

        if rl.is_key_pressed(KeyboardKey::KEY_E) {
            // TODO(reece): Want to check if a brush stroke is already happening? Could just cut
            // the working stroke off when changing brush type
            brush.brush_type = BrushType::Erasing;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            brush.brush_type = BrushType::Deleting;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_Q) {
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

                strokes.push(Some(generated_stroke));
            }
        }

        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            camera.zoom += 0.05;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_L) {
            camera.zoom -= 0.05;
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            apply_mouse_drag_to_camera(mouse_pos, last_mouse_pos, &mut camera);
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_MIDDLE_BUTTON) {
            delete_stroke(
                &mut strokes,
                &mut stroke_graveyard,
                &drawing_pos,
                brush.brush_size,
            );
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
            if brush.brush_type == BrushType::Deleting {
                delete_stroke(
                    &mut strokes,
                    &mut stroke_graveyard,
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
                        BrushType::Erasing => Color::WHITE,
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
                strokes.push(Some(working_stroke));
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
            for line in &strokes {
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
            BrushType::Erasing => "Erasing",
            BrushType::Deleting => "Deleting",
        };
        let brush_size_str = format!("Brush size: {}", brush.brush_size.to_string());
        let zoom_str = format!("Zoom: {}", camera.zoom.to_string());
        drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
        drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
        drawing.draw_text(&zoom_str, 5, 60, 30, Color::RED);

        if debugging {
            let target_str = format!("target {:?}", camera.target);
            drawing.draw_text(&target_str, 5, 90, 30, Color::RED);
            let drawing_pos_str = format!("draw pos {:?}", drawing_pos);
            drawing.draw_text(&drawing_pos_str, 5, 120, 30, Color::RED);
            let number_of_strokes_str = format!("Total strokes: {}", strokes.len());
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
