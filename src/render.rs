use crate::{Brush, Stroke};
use raylib::color::Color;
use raylib::drawing::{RaylibDraw, RaylibDrawHandle, RaylibMode2D};
use raylib::math::{rvec2, Vector2};

pub fn draw_stroke(drawing: &mut RaylibMode2D<RaylibDrawHandle>, stroke: &Stroke, brush_size: f32) {
    if stroke.points.len() == 0 {
        return;
    }

    let points: &Vec<Vector2> = &stroke.points.iter().map(|p| rvec2(p.x, p.y)).collect();
    drawing.draw_spline_basis(points, brush_size, stroke.color);
}

pub fn draw_brush_marker(
    drawing: &mut RaylibMode2D<RaylibDrawHandle>,
    drawing_pos: Vector2,
    brush: &Brush,
) {
    drawing.draw_circle_lines(
        drawing_pos.x as i32,
        drawing_pos.y as i32,
        // Draw circle wants radius
        brush.brush_size / 2.0,
        Color::BLACK,
    );
}
