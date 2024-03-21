use raylib::drawing::{RaylibDraw, RaylibDrawHandle, RaylibMode2D};
use raylib::math::Vector2;
use crate::Stroke;

pub fn draw_stroke(drawing: &mut RaylibMode2D<RaylibDrawHandle>, stroke: &Stroke, brush_size: f32) {
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
