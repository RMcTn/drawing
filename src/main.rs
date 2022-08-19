use raylib::prelude::{Vector2, *};

#[derive(Debug)]
struct Point {
    x: f32,
    y: f32,
}

struct Stroke {
    // TODO(reece): Want to turn this into an enum for the types of stroke? paint/erase stroke
    // for example
    points: Vec<Point>,
    color: Color,
    brush_size: f32,
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
    println!("Hello, world!");
    let screen_width = 1280;
    let screen_height = 720;

    let (mut rl, thread) = raylib::init()
        .size(screen_width, screen_height)
        .title("Window")
        .build();

    rl.set_target_fps(60);

    let brush_size = 10.0;

    let mut lines: Vec<Stroke> = Vec::new();
    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size,
    };

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
    let mut draw_x_offset = 0.0;
    let mut draw_y_offset = 0.0;
    let mut last_mouse_pos = rl.get_mouse_position();

    while !rl.window_should_close() {
        // TODO(reece): Zooming, but not big priority
        let mouse_pos = rl.get_mouse_position();
        if rl.is_key_down(KeyboardKey::KEY_A) {
            draw_x_offset -= 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_D) {
            draw_x_offset += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_S) {
            draw_y_offset += 5.0;
        }
        if rl.is_key_down(KeyboardKey::KEY_W) {
            draw_y_offset -= 5.0;
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

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            // Dragging

            // TODO(reece): To be tested on an actual mouse so left click + right click can be done
            // together
            let mouse_diff = mouse_pos - last_mouse_pos;
            draw_x_offset -= mouse_diff.x;
            draw_y_offset -= mouse_diff.y;
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
                x: mouse_pos.x + draw_x_offset,
                y: mouse_pos.y + draw_y_offset,
            };
            working_stroke.points.push(point);
        }
        if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) {
            // Finished drawing
            if is_drawing {
                lines.push(working_stroke);
                working_stroke = Stroke::new(Color::BLACK, brush.brush_size);
            }
            is_drawing = false;
        }

        last_mouse_pos = mouse_pos;

        let mut drawing = rl.begin_drawing(&thread);

        drawing.clear_background(Color::WHITE);
        for line in &lines {
            draw_stroke(
                &mut drawing,
                &line,
                line.brush_size,
                draw_x_offset,
                draw_y_offset,
            );
        }

        // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
        draw_stroke(
            &mut drawing,
            &working_stroke,
            working_stroke.brush_size,
            draw_x_offset,
            draw_y_offset,
        );

        let brush_type_str = match &brush.brush_type {
            BrushType::Drawing => "Drawing",
            BrushType::Erasing => "Erasing",
        };
        let brush_size_str = brush.brush_size.to_string();
        drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
        drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
    }
}

fn draw_stroke(
    drawing: &mut RaylibDrawHandle,
    stroke: &Stroke,
    brush_size: f32,
    x_offset: f32,
    y_offset: f32,
) {
    if stroke.points.len() == 0 {
        return;
    }
    for i in 0..stroke.points.len() - 1 {
        let point = &stroke.points[i];
        let next_point = &stroke.points[i + 1];

        let first_vec = Vector2 {
            x: point.x - x_offset,
            y: point.y - y_offset,
        };
        let last_vec = Vector2 {
            x: next_point.x - x_offset,
            y: next_point.y - y_offset,
        };

        // We're drawing the line + circle here just cause it looks a bit better (circle hides the blockiness of the line)
        drawing.draw_line_ex(first_vec, last_vec, brush_size, stroke.color);
        drawing.draw_circle_v(last_vec, brush_size / 2.0, stroke.color);
    }
}
