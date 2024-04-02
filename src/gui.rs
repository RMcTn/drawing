use std::ffi::CString;

use raylib::{
    color::Color,
    drawing::{RaylibDraw, RaylibDrawHandle, RaylibMode2D},
    math::{rrect, rvec2, Rectangle, Vector2},
    rgui::RaylibDrawGui,
    text::{measure_text_ex, Font, WeakFont},
};

use crate::{state::State, Brush, BrushType, Keymap};

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

pub fn draw_keymap(
    drawing: &mut RaylibDrawHandle,
    keymap: &Keymap,
    keymap_panel_bounds: Rectangle,
    font: &WeakFont,
    font_size: f32,
    letter_spacing: f32,
) {
    // TODO: FIXME: Text will happily overflow the bounds of the panel if it's long enough

    drawing.gui_group_box(
        keymap_panel_bounds,
        Some(&CString::new("Keymappings").unwrap()),
    );

    let spacing_y = 30.0;
    let spacing_x = 30.0;

    let key_hold_bounds = rrect(
        keymap_panel_bounds.x + spacing_x,
        keymap_panel_bounds.y + spacing_y,
        (keymap_panel_bounds.width / 2.0) - spacing_x,
        keymap_panel_bounds.height,
    );
    let key_press_bounds = rrect(
        key_hold_bounds.x + key_hold_bounds.width + spacing_x,
        keymap_panel_bounds.y + spacing_y,
        (keymap_panel_bounds.width / 2.0) - spacing_x,
        keymap_panel_bounds.height,
    );
    let mut last_y_pos = key_hold_bounds.y;
    // TODO: Pretty print
    // TODO: Scrolling
    for (key, command) in &keymap.on_hold {
        let str = format!("{:?} - {:?}", key, command);
        let text_measurements = measure_text_ex(&font, &str, font_size, letter_spacing);
        let text_y_pos = last_y_pos + spacing_y + text_measurements.y;
        drawing.draw_text_rec(
            &font,
            &str,
            rrect(
                key_hold_bounds.x,
                text_y_pos,
                key_hold_bounds.width,
                key_hold_bounds.height,
            ),
            font_size,
            letter_spacing,
            true,
            Color::GOLD,
        );
        last_y_pos = text_y_pos;
    }

    let mut last_y_pos = key_press_bounds.y;
    for (key, command) in &keymap.on_press {
        let str = format!("{:?} - {:?}", key, command);
        let text_measurements = measure_text_ex(&font, &str, font_size, letter_spacing);
        let text_y_pos = last_y_pos + spacing_y + text_measurements.y;
        drawing.draw_text_rec(
            &font,
            &str,
            rrect(
                key_press_bounds.x,
                text_y_pos,
                key_press_bounds.width,
                key_press_bounds.height,
            ),
            font_size,
            letter_spacing,
            true,
            Color::GOLD,
        );
        last_y_pos = text_y_pos;
    }
}
