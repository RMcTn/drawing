use raylib::{
    color::Color,
    drawing::{RaylibDraw, RaylibDrawHandle, RaylibMode2D},
    math::{rvec2, Rectangle, Vector2},
};

use crate::{state::State, Brush, BrushType};

pub fn is_clicking_gui(mouse_pos: Vector2, bounds: Rectangle) -> bool {
    return bounds.check_collision_point_rec(mouse_pos);
}

pub fn draw_info_ui(drawing: &mut RaylibDrawHandle, state: &State, brush: &Brush) {
    let brush_type_str = match &brush.brush_type {
        BrushType::Drawing => "Drawing",
        BrushType::Deleting => "Deleting",
    };
    let brush_size_str = format!("Brush size: {}", brush.brush_size.to_string());
    let zoom_str = format!("Zoom: {:.2}", state.camera.zoom);
    drawing.draw_text(brush_type_str, 5, 5, 30, Color::RED);
    drawing.draw_text(&brush_size_str, 5, 30, 30, Color::RED);
    drawing.draw_text(&zoom_str, 5, 60, 30, Color::RED);

    let mode_str = format!("Mode: {:?}", state.mode);
    drawing.draw_text(&mode_str, 5, 90, 30, Color::RED);
}

pub fn debug_draw_info(
    drawing: &mut RaylibDrawHandle,
    state: &State,
    drawing_pos: Vector2,
    current_fps: u32,
) {
    let target_str = format!("target {:?}", state.camera.target);
    drawing.draw_text(&target_str, 5, 120, 30, Color::RED);
    let drawing_pos_str = format!("draw pos {:?}", drawing_pos);
    drawing.draw_text(&drawing_pos_str, 5, 150, 30, Color::RED);
    let number_of_strokes_str = format!("Total strokes: {}", state.strokes.len());
    drawing.draw_text(&number_of_strokes_str, 5, 180, 30, Color::RED);
    let fps_str = format!("FPS: {}", current_fps);
    drawing.draw_text(&fps_str, 5, 210, 30, Color::RED);
}

pub fn debug_draw_center_crosshair(
    drawing: &mut RaylibMode2D<'_, RaylibDrawHandle<'_>>,
    state: &State,
    screen_width: i32,
    screen_height: i32,
) {
    drawing.draw_line_ex(
        rvec2(state.camera.target.x, (-screen_height * 10) as f32),
        rvec2(state.camera.target.x, (screen_height * 10) as f32),
        5.0,
        Color::PURPLE,
    );
    drawing.draw_line_ex(
        rvec2((-screen_width * 10) as f32, state.camera.target.y),
        rvec2((screen_width * 10) as f32, state.camera.target.y),
        5.0,
        Color::PURPLE,
    );
}
