use raylib::prelude::{Vector2, *};

#[derive(Debug)]
struct Point {
    x: f32,
    y: f32,
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

struct Brush {
    brush_type: BrushType,
    brush_size: f32,
}

enum BrushType {
    Drawing,
    Erasing,
}

fn main() {
    let screen_width = 1280;
    let screen_height = 720;

    let (mut rl, thread) = raylib::init()
        .size(screen_width, screen_height)
        .title("Window")
        .build();

    rl.set_target_fps(60);

    let mut camera = Camera2D {
        offset: rvec2(0, 0),
        target: rvec2(0, 0),
        rotation: 0.0,
        zoom: 1.0,
    };

    let brush_size = 10.0;

    let mut strokes: Vec<Stroke> = Vec::with_capacity(10);
    let mut stroke_graveyard: Vec<Stroke> = Vec::with_capacity(10);
    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size,
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
    let mut last_mouse_pos = rl.get_mouse_position();

    while !rl.window_should_close() {
        // TODO(reece): Have zoom follow the cursor i.e zoom into where the cursor is rather than
        // "top left corner"
        // TODO(reece): Ctrl + mousewheel for brush size changing
        let mouse_pos = rl.get_mouse_position();
        let drawing_pos = (mouse_pos - camera.offset) / camera.zoom;

        if rl.is_key_down(KeyboardKey::KEY_A) {
            camera.offset.x += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_D) {
            camera.offset.x -= 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_S) {
            camera.offset.y -= 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_W) {
            camera.offset.y += 5.0;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_Z) {
            // Undo
            match strokes.pop() {
                None => {} // Nothing to undo
                Some(undone_stroke) => stroke_graveyard.push(undone_stroke),
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            // Redo
            match stroke_graveyard.pop() {
                None => {} // Nothing to redo
                Some(redone_stroke) => strokes.push(redone_stroke),
            }
        }

        if rl.is_key_pressed(KeyboardKey::KEY_E) {
            // TODO(reece): Want to check if a brush stroke is already happening? Could just cut
            // the working stroke off when changing brush type
            brush.brush_type = BrushType::Erasing;
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
            if brush.brush_size < 0.0 {
                brush.brush_size = 0.0;
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_RIGHT_BRACKET) {
            brush.brush_size += 5.0;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_H) {
            // Create bunch of strokes with random coords in screen space for benchmark testing
        }

        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            camera.zoom += 0.05;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_L) {
            camera.zoom -= 0.05;
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            // Dragging

            // TODO(reece): To be tested on an actual mouse so left click + right click can be done
            // together
            let mouse_diff = mouse_pos - last_mouse_pos;
            camera.offset.x += mouse_diff.x;
            camera.offset.y += mouse_diff.y;
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
            // Drawing
            if !is_drawing {
                let brush_color = match &brush.brush_type {
                    // TODO(reece): Will want these colours to be dynamic (whatever user
                    // picked (drawing)/whatever bg colour is (erasing))
                    BrushType::Drawing => Color::BLACK,
                    BrushType::Erasing => Color::WHITE,
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
        if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) {
            // Finished drawing
            if is_drawing {
                strokes.push(working_stroke);
                working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
            }
            is_drawing = false;
        }

        camera.zoom += rl.get_mouse_wheel_move() * 0.1;

        if camera.zoom < 0.1 {
            camera.zoom = 0.1;
        }

        if camera.zoom > 10.0 {
            camera.zoom = 10.0;
        }

        last_mouse_pos = mouse_pos;

        let mut drawing = rl.begin_drawing(&thread);
        {
            let mut drawing_camera = drawing.begin_mode2D(camera);

            drawing_camera.clear_background(Color::WHITE);
            for line in &strokes {
                draw_stroke(&mut drawing_camera, &line, line.brush_size);
            }

            // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
            draw_stroke(
                &mut drawing_camera,
                &working_stroke,
                working_stroke.brush_size,
            );

            drawing_camera.draw_circle_lines(
                drawing_pos.x as i32,
                drawing_pos.y as i32,
                // Draw circle wants radius
                brush.brush_size / 2.0,
                Color::BLACK,
            );
        }

        let brush_type_str = match &brush.brush_type {
            BrushType::Drawing => "Drawing",
            BrushType::Erasing => "Erasing",
        };
        let brush_size_str = brush.brush_size.to_string();
        let zoom_str = camera.zoom.to_string();
        drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
        drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
        drawing.draw_text(&zoom_str, 5, 60, 30, Color::RED);
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
        drawing.draw_circle_v(last_vec, brush_size / 2.0, stroke.color);
    }
}
