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
}

impl Stroke {
    fn new(color: Color) -> Self {
        let default_num_of_points = 30;
        Stroke {
            points: Vec::with_capacity(default_num_of_points),
            color,
        }
    }
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

    let mut is_drawing = false;
    let mut is_erasing = false;
    let mut working_stroke = Stroke::new(Color::BLACK);
    while !rl.window_should_close() {
        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
            if is_erasing {
                break;
            }
            if !is_drawing {
                working_stroke = Stroke::new(Color::BLACK);
                is_drawing = true;
            }
            let mouse_pos = rl.get_mouse_position();
            let point = Point {
                x: mouse_pos.x,
                y: mouse_pos.y,
            };
            working_stroke.points.push(point);
        }
        if rl.is_mouse_button_up(MouseButton::MOUSE_LEFT_BUTTON) {
            if is_drawing {
                lines.push(working_stroke);
                working_stroke = Stroke::new(Color::BLACK);
            }
            is_drawing = false;
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON) {
            if is_drawing {
                break;
            }
            if !is_erasing {
                working_stroke = Stroke::new(Color::WHITE);
                is_erasing = true;
            }
            let mouse_pos = rl.get_mouse_position();
            let point = Point {
                x: mouse_pos.x,
                y: mouse_pos.y,
            };
            working_stroke.points.push(point);
        }
        if rl.is_mouse_button_up(MouseButton::MOUSE_RIGHT_BUTTON) {
            if is_erasing {
                lines.push(working_stroke);
                working_stroke = Stroke::new(Color::BLACK);
            }
            is_erasing = false;
        }

        let mut drawing = rl.begin_drawing(&thread);

        drawing.clear_background(Color::WHITE);
        for line in &lines {
            draw_stroke(&mut drawing, &line, brush_size);
        }

        // TODO(reece): Do we want to treat the working_stroke as a special case to draw?
        draw_stroke(&mut drawing, &working_stroke, brush_size);
    }
}

fn draw_stroke(drawing: &mut RaylibDrawHandle, stroke: &Stroke, brush_size: f32) {
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
        drawing.draw_circle_v(last_vec, brush_size / 2.0, stroke.color);
    }
}
