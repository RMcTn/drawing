use crate::app::{Brush, Renderable, Stroke, Thing, Things};
use raylib::color::Color;
use raylib::drawing::{RaylibDraw, RaylibDrawHandle, RaylibMode2D};
use raylib::math::{rrect, rvec2, Vector2};
use raylib::text::WeakFont;
use raylib::texture::Texture2D;

pub fn draw_stroke(drawing: &mut RaylibMode2D<RaylibDrawHandle>, stroke: &Stroke, brush_size: f32) {
    if stroke.points.is_empty() {
        return;
    }

    let points: &Vec<Vector2> = &stroke.points.iter().map(|p| rvec2(p.x, p.y)).collect();
    drawing.draw_spline_basis(points, brush_size, stroke.color);
}

pub fn draw_stroke_at_offset(
    drawing: &mut RaylibMode2D<RaylibDrawHandle>,
    stroke: &Stroke,
    brush_size: f32,
    offset: Vector2,
) {
    if stroke.points.is_empty() {
        return;
    }

    let points: Vec<Vector2> = stroke
        .points
        .iter()
        .map(|p| rvec2(p.x + offset.x, p.y + offset.y))
        .collect();
    drawing.draw_spline_basis(&points, brush_size, stroke.color);
}

pub fn draw_thing_at_offset(
    drawing: &mut RaylibMode2D<RaylibDrawHandle>,
    thing: &Thing,
    offset: Vector2,
    image_texture: Option<&Texture2D>,
) {
    match &thing.kind {
        Renderable::Stroke(stroke) => {
            draw_stroke_at_offset(drawing, stroke, stroke.brush_size, offset);
        }
        Renderable::Text(text) => {
            if let Some(pos) = text.position {
                let offset_pos = Vector2 {
                    x: pos.x + offset.x,
                    y: pos.y + offset.y,
                };
                drawing.draw_text(
                    &text.content,
                    offset_pos.x as i32,
                    offset_pos.y as i32,
                    text.size.0 as i32,
                    text.color.0,
                );
            }
        }
        Renderable::Image(image) => {
            if let Some(texture) = image_texture {
                let mut destination = image.rect;
                destination.x += offset.x;
                destination.y += offset.y;
                drawing.draw_texture_pro(
                    texture,
                    rrect(0, 0, texture.width, texture.height),
                    destination,
                    rvec2(0, 0),
                    0.0,
                    Color::WHITE,
                );
            }
        }
    }
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

pub fn draw_bounding_boxes(
    things: &Things,
    drawing_camera: &mut RaylibMode2D<'_, RaylibDrawHandle<'_>>,
    font: &WeakFont,
) {
    for (_, thing) in things {
        let bounding_box = thing.bounding_box(font).unwrap();
        drawing_camera.draw_rectangle_lines_ex(bounding_box.rect(), 1.0, Color::PURPLE);
    }
}
