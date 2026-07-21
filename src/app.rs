use crate::gui::{
    debug_draw_center_crosshair, draw_color_dropper_icon, draw_color_dropper_preview, draw_info_ui,
    draw_keymap, is_clicking_gui,
};
use crate::replay::{load_replay, play_replay, replay_inputs};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use log::{debug, error};
use raylib::prelude::{Vector2, *};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};
use std::{
    cmp,
    collections::HashMap,
    fmt::Display,
    path::PathBuf,
    thread,
    time::{self, Duration, Instant},
};

use crate::input::{
    get_char_pressed, is_mouse_button_down, is_mouse_button_pressed, paste_clipboard,
    process_key_down_events, process_key_pressed_events, was_mouse_button_released,
};
use crate::render::{
    draw_bounding_boxes, draw_brush_marker, draw_stroke, draw_thing, draw_thing_at_offset,
};
use crate::state::{ForegroundColor, State, TextColor, TextSize};
use crate::{gui::debug_draw_info, input::append_input_to_working_text};

pub const RECORDING_OUTPUT_PATH: &str = "recording.rae";

#[derive(Debug)]
pub struct TestSettings {
    pub save_after_replay: bool,
    pub save_path: PathBuf,
    pub quit_after_replay: bool,
}

pub fn run(replay_path: Option<PathBuf>, test_options: Option<TestSettings>) {
    let keymap = default_keymap();
    let mut debugging = false;

    let mut screen_width = 1280;
    let mut screen_height = 720;

    let (mut rl, rl_thread) = raylib::init()
        .size(screen_width, screen_height)
        .resizable()
        .title("Window")
        .build();

    let mut automation_events_list = rl.load_automation_event_list(None);
    rl.set_automation_event_list(&mut automation_events_list);
    let mut automation_events = automation_events_list.events();

    let color_picker_scaling_factor = 4; // TODO: Make other GUI things scalable.
                                         // TODO: Configurable scaling

    let color_dropper_icon_bytes = include_bytes!("../assets/color-dropper.png").to_vec();
    let color_dropper_icon_image = Image::load_image_from_mem(".png", &color_dropper_icon_bytes)
        .expect("Couldn't create color dropper icon from packaged color dropper image");
    let color_dropper_icon = rl
        .load_texture_from_image(&rl_thread, &color_dropper_icon_image)
        .expect("Couldn't find color dropper icon file");
    let color_dropper_width = color_dropper_icon.width(); // REFACTOR: Will want something similar
                                                          // for other tool icons
    let color_dropper_height = color_dropper_icon.height();
    let color_dropper_scaled_width = color_dropper_width * color_picker_scaling_factor;
    let color_dropper_scaled_height = color_dropper_height * color_picker_scaling_factor;
    let color_dropper_source_rect = rrect(0, 0, color_dropper_width, color_dropper_height);

    let target_fps = 60;
    let seconds_per_frame = 1.0 / target_fps as f32;
    let duration_per_frame = time::Duration::from_secs_f32(seconds_per_frame);

    let camera = Camera2D {
        offset: rvec2(screen_width / 2, screen_height / 2),
        target: rvec2(0, 0),
        rotation: 0.0,
        zoom: 1.0,
    };

    let outline_color = Color::BLACK; // TODO: Using black as a stand in until we do something that
                                      // reacts to the background color

    let initial_brush_size = 10.0;

    let mut brush = Brush {
        brush_type: BrushType::Drawing,
        brush_size: initial_brush_size,
    };

    let mut state = State {
        things: SlotMap::with_key(),
        undo_actions: Vec::new(),
        redo_actions: Vec::new(),
        things_graveyard: SlotMap::with_key(),
        output_path: None,
        camera,
        background_color: Default::default(),
        foreground_color: Default::default(),
        mode: Mode::UsingTool(Tool::Brush),
        mouse_pos: rvec2(0, 0),
        text_size: TextSize(50),
        text_color: Default::default(),
        is_recording_inputs: false,
        is_playing_inputs: false,
        current_play_frame: 0,
        play_frame_counter: 0,
        selected_things: vec![],
        mouse_drag_box: None,
    };

    if let Some(replay_path) = replay_path {
        if let Some(()) = load_replay(
            &replay_path,
            &rl,
            &mut automation_events_list,
            &mut automation_events,
        ) {
            play_replay(&mut state);
        } else {
            error!("Could not load replay")
        }
    }

    let mut is_drawing = false;
    let mut working_stroke = Stroke::new(ForegroundColor::default().0, brush.brush_size);
    let mut working_text: Option<Text> = None;
    let mut working_move: Option<(Vector2, Vector2)> = None;
    let mut working_resize: Option<WorkingResize> = None;
    let mut image_textures: HashMap<ThingKey, (usize, Texture2D)> = HashMap::new();
    let mut last_mouse_pos = rl.get_mouse_position();

    let mut color_picker_info: Option<GuiColorPickerInfo> = None;

    let font = rl.get_font_default();

    let ui_font_size = 20.0; // TODO: Make user configurable

    let mut time_since_last_text_deletion = Duration::ZERO;
    let delay_between_text_deletions = Duration::from_millis(100); // TODO: Make user configurable

    let mut processed_press_commands: HashMap<PressCommand, bool> = keymap
        .on_press
        .iter()
        .map(|entry| (entry.1, false))
        .collect();

    let mut mouse_buttons_pressed_this_frame = HashMap::from([
        (MouseButton::MOUSE_BUTTON_LEFT, false),
        (MouseButton::MOUSE_BUTTON_RIGHT, false),
        (MouseButton::MOUSE_BUTTON_MIDDLE, false),
    ]);
    let mut mouse_buttons_pressed_last_frame = HashMap::from([
        (MouseButton::MOUSE_BUTTON_LEFT, false),
        (MouseButton::MOUSE_BUTTON_RIGHT, false),
        (MouseButton::MOUSE_BUTTON_MIDDLE, false),
    ]);
    while !rl.window_should_close() {
        let delta_time = rl.get_frame_time();
        let current_fps = rl.get_fps();
        // TODO: Hotkey configuration
        // TODO(reece): Have zoom follow the cursor i.e zoom into where the cursor is rather than
        // "top left corner"
        // TODO(reece): Improve how the lines look. Make a line renderer or something?
        // TODO(reece): BUG: Brush marker looks like it's a bit off centre from the mouse cursor
        // TODO(reece): Use shaders for line drawing?
        //
        // TODO(reece): Installable so it's searchable as a program
        // TODO(reece): Optimize this so we're not smashing the cpu/gpu whilst doing nothing (only
        // update on user input?)

        time_since_last_text_deletion += Duration::from_secs_f32(delta_time);

        let start_time = Instant::now();
        screen_width = rl.get_screen_width();
        screen_height = rl.get_screen_height();
        state.camera.offset = rvec2(screen_width / 2, screen_height / 2);

        state.mouse_pos = rl.get_mouse_position();
        let mouse_drawing_pos = rl.get_screen_to_world2D(state.mouse_pos, state.camera);

        let keymap_panel_padding_percent = 0.10;
        let keymap_panel_padding_x = screen_width as f32 * keymap_panel_padding_percent;
        let keymap_panel_padding_y = screen_height as f32 * keymap_panel_padding_percent;
        let keymap_panel_bounds = rrect(
            keymap_panel_padding_x,
            keymap_panel_padding_y,
            screen_width as f32 - (keymap_panel_padding_x * 2.0),
            screen_height as f32 - (keymap_panel_padding_y * 2.0),
        );

        let mut color_picker_closed_this_frame = false;

        // NOTE: Make sure any icons we don't want interfering with this color have a transparent
        // pixel at the mouse pos (or draw it away from the mouse pos a bit)
        let pixel_color_at_mouse_pos = rl.load_image_from_screen(&rl_thread).get_color(
            state.mouse_pos.x.clamp(0.0, (screen_width - 1) as f32) as i32,
            state.mouse_pos.y.clamp(0.0, (screen_height - 1) as f32) as i32,
        );

        // color picker activate check
        if (state.mode == Mode::UsingTool(Tool::Brush) || state.using_text_tool_or_typing())
            && is_mouse_button_down(
                &mut rl,
                MouseButton::MOUSE_BUTTON_RIGHT,
                &mut mouse_buttons_pressed_this_frame,
            )
        {
            debug!("Making colour picker active");
            let picker_width = 100;
            let picker_height = 100;
            color_picker_info = Some(GuiColorPickerInfo {
                initiation_pos: state.mouse_pos,
                bounds: rrect(
                    state.mouse_pos.x - (picker_width as f32 / 2.0),
                    state.mouse_pos.y - (picker_height as f32 / 2.0),
                    picker_width,
                    picker_height,
                ),
                picker_slider_x_padding: 30.0,
            });
        }

        // color picker closer check
        if let Some(picker_info) = &color_picker_info {
            if !is_clicking_gui(state.mouse_pos, picker_info.bounds_with_slider())
                && is_mouse_button_down(
                    &mut rl,
                    MouseButton::MOUSE_BUTTON_LEFT,
                    &mut mouse_buttons_pressed_this_frame,
                )
            {
                close_color_picker(&mut color_picker_info, &mut color_picker_closed_this_frame);
            }
        }

        let paste_pressed = rl.is_key_pressed(KeyboardKey::KEY_V)
            && (rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
                || rl.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL)
                || rl.is_key_down(KeyboardKey::KEY_LEFT_SUPER)
                || rl.is_key_down(KeyboardKey::KEY_RIGHT_SUPER));
        if paste_pressed {
            let text_target = if state.mode == Mode::TypingText {
                working_text.as_mut()
            } else {
                None
            };
            paste_clipboard(&mut state, mouse_drawing_pos, text_target);
        }

        match state.mode {
            Mode::UsingTool(tool) => match tool {
                Tool::Brush => {
                    // TODO: FIXME: Quite easy to accidentally draw when coming out of background
                    // color picker - Maybe a little delay before drawing after clicking off the
                    // picker?

                    if is_mouse_button_down(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mut mouse_buttons_pressed_this_frame,
                    ) && !is_color_picker_active(&color_picker_info)
                    {
                        if brush.brush_type == BrushType::Deleting {
                            let strokes_to_delete =
                                state.strokes_within_point(mouse_drawing_pos, brush.brush_size);
                            state.delete_strokes(strokes_to_delete);
                        } else {
                            // Drawing
                            if !is_drawing {
                                working_stroke =
                                    Stroke::new(state.foreground_color.0, brush.brush_size);
                                is_drawing = true;
                            }

                            let point = Point {
                                x: mouse_drawing_pos.x,
                                y: mouse_drawing_pos.y,
                            };
                            working_stroke.points.push(point);
                        }
                    }
                    if was_mouse_button_released(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mouse_buttons_pressed_last_frame,
                    ) {
                        dbg!("Left mouse release");
                        // Finished drawing
                        // TODO: FIXME: Do not allow text tool if currently drawing, otherwise we won't be able to end
                        // the brush stroke unless we change back to brush mode
                        if is_drawing {
                            let thing = Thing {
                                kind: Renderable::Stroke(working_stroke),
                            };
                            state.add_thing_with_undo(thing);
                            working_stroke =
                                Stroke::new(state.foreground_color.0, brush.brush_size);
                        }
                        is_drawing = false;
                    }
                }
                Tool::Text => {
                    if is_mouse_button_down(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mut mouse_buttons_pressed_this_frame,
                    ) && !is_color_picker_active(&color_picker_info)
                        && !color_picker_closed_this_frame
                    {
                        debug!("Hit left click on text tool");
                        // Start text
                        if working_text.is_none() {
                            working_text = Some(Text {
                                content: "".to_string(),
                                position: Some(mouse_drawing_pos),
                                size: state.text_size,
                                color: state.text_color,
                            });
                        }
                        state.mode = Mode::TypingText;
                    }
                }
                Tool::ColorPicker => {
                    if is_mouse_button_down(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mut mouse_buttons_pressed_this_frame,
                    ) {
                        // NOTE: This literally is whatever color is at the screen. This includes
                        // GUI elements! If it gets annoying enough, it can be changed, but this
                        // was simpler
                        state.foreground_color.0 = pixel_color_at_mouse_pos;

                        // TODO: Text colour picking as well
                        state.mode = Mode::UsingTool(Tool::Brush);
                    }
                }
                Tool::Selection => {
                    if is_mouse_button_down(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mut mouse_buttons_pressed_this_frame,
                    ) {
                        if let Some(drag_box) = state.mouse_drag_box {
                            let drag_box = BoundingBox2D {
                                min: drag_box.min,
                                max: rvec2(mouse_drawing_pos.x, mouse_drawing_pos.y),
                            };
                            state.mouse_drag_box = Some(drag_box);
                        } else {
                            let drag_box = BoundingBox2D {
                                min: rvec2(mouse_drawing_pos.x, mouse_drawing_pos.y),
                                max: rvec2(mouse_drawing_pos.x, mouse_drawing_pos.y),
                            };
                            state.mouse_drag_box = Some(drag_box);
                        }
                    } else {
                        let mut things_in_selection = vec![];
                        // Gather everything that was in the drag box
                        if let Some(drag_box) = state.mouse_drag_box {
                            for (thing_key, thing) in &state.things {
                                if let Some(bounding_box) = thing.bounding_box(&font) {
                                    if bounding_box.rect().check_collision_recs(&drag_box.rect()) {
                                        things_in_selection.push(thing_key);
                                    }
                                }
                            }
                            if !things_in_selection.is_empty() {
                                state.mode = Mode::UsingTool(Tool::Move);
                                state.selected_things = things_in_selection;
                            } else {
                                state.selected_things.clear();
                            };
                        }
                        state.mouse_drag_box = None;
                    }
                }
                Tool::Move => {
                    if is_mouse_button_down(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mut mouse_buttons_pressed_this_frame,
                    ) {
                        if let Some(resize) = working_resize.as_mut() {
                            let scale = resize_scale(resize, mouse_drawing_pos);
                            state.preview_resize(&mut resize.changes, resize.anchor, scale);
                        } else if let Some(working_move) = working_move.as_mut() {
                            working_move.1 = mouse_drawing_pos;
                        } else {
                            let resize_bounds = resizable_selection_bounds(&state, &font)
                                .map(|bounds| resize_handle(bounds, state.camera.zoom));
                            if resize_bounds.is_some_and(|handle| {
                                handle.check_collision_point_rec(mouse_drawing_pos)
                            }) {
                                if let Some(bounds) = resizable_selection_bounds(&state, &font) {
                                    working_resize = Some(WorkingResize {
                                        anchor: bounds.min,
                                        diagonal: mouse_drawing_pos - bounds.min,
                                        changes: state
                                            .capture_resize_changes(&state.selected_things),
                                    });
                                }
                            } else {
                                working_move = Some((mouse_drawing_pos, mouse_drawing_pos));
                            }
                        }
                    }

                    if was_mouse_button_released(
                        &mut rl,
                        MouseButton::MOUSE_BUTTON_LEFT,
                        &mouse_buttons_pressed_last_frame,
                    ) {
                        if let Some(mut resize) = working_resize.take() {
                            let scale = resize_scale(&resize, mouse_drawing_pos);
                            state.preview_resize(&mut resize.changes, resize.anchor, scale);
                            state.commit_resize_with_undo(resize.changes);
                        } else if let Some(working_move) = working_move {
                            let move_diff = working_move.1 - working_move.0;

                            if move_diff.x.abs() > 0.0 || move_diff.y.abs() > 0.0 {
                                state.move_things_with_undo(
                                    &state.selected_things.clone(),
                                    move_diff,
                                );
                            }
                        }
                        working_move = None;
                        state.selected_things.clear();
                        state.mode = Mode::UsingTool(Tool::Brush);
                    }
                }
            },
            Mode::PickingBackgroundColor(color_picker) => {
                if is_mouse_button_pressed(
                    &mut rl,
                    MouseButton::MOUSE_BUTTON_LEFT,
                    &mut mouse_buttons_pressed_this_frame,
                ) && !is_clicking_gui(state.mouse_pos, color_picker.bounds_with_slider())
                {
                    state.mode = Mode::UsingTool(Tool::Brush);
                }
            }
            Mode::TypingText => {
                if rl.is_key_down(KeyboardKey::KEY_BACKSPACE)
                    && time_since_last_text_deletion >= delay_between_text_deletions
                {
                    if let Some(text) = working_text.as_mut() {
                        let _removed_char = text.content.pop();
                    }
                    time_since_last_text_deletion = Duration::ZERO;
                }

                if rl.is_key_down(KeyboardKey::KEY_ENTER) {
                    dbg!("Exiting text tool");
                    if let Some(mut text) = working_text {
                        if !text.content.is_empty() {
                            text.color = state.text_color;
                            text.size = state.text_size;
                            let thing = Thing {
                                kind: Renderable::Text(text),
                            };
                            state.add_thing_with_undo(thing);
                        }
                    }

                    working_text = None;
                    state.mode = Mode::UsingTool(Tool::Brush);
                    close_color_picker(&mut color_picker_info, &mut color_picker_closed_this_frame);
                }

                let char_pressed = if paste_pressed {
                    None
                } else {
                    get_char_pressed()
                };

                // TODO: FIXME: BUG: Raylib's event automation doesn't track chars pressed (probably due to
                // platform differences). If we relied on key pressed instead, then:
                //      - We wouldn't be able to differ between uppercase and lowercase (KEY_A
                //      doesn't tell you if it's lower or uppercase)
                //      - We'd need to make our own "repeat key" logic, as holding a key looks like
                //      it only gets 1 key pressed raylib event fired off (makes sense)

                if let Some(ch) = char_pressed {
                    append_input_to_working_text(
                        ch,
                        &mut working_text,
                        state.text_size,
                        state.text_color,
                    )
                }
            }
            Mode::ShowingKeymapPanel => {
                if is_mouse_button_pressed(
                    &mut rl,
                    MouseButton::MOUSE_BUTTON_LEFT,
                    &mut mouse_buttons_pressed_this_frame,
                ) && !is_clicking_gui(state.mouse_pos, keymap_panel_bounds)
                {
                    state.mode = Mode::default();
                }
            }
        }

        if state.mode != Mode::TypingText {
            // TODO: FIXME: If these keymaps share keys (like S to move the camera, and ctrl + S to
            // save), then both will actions be triggered. Haven't thought about how to handle
            // that yet
            // Ctrl/Cmd+V must not also trigger the plain-V recording shortcut.
            if !paste_pressed {
                process_key_pressed_events(
                    &keymap,
                    &mut debugging,
                    &mut rl,
                    &mut brush,
                    &mut state,
                    &mut processed_press_commands,
                    &mut automation_events_list,
                    &mut automation_events,
                );
            }
            process_key_down_events(
                &keymap,
                screen_width,
                screen_height,
                &mut rl,
                &mut brush,
                &mut state,
                delta_time,
            );
        }

        // TODO: Configurable mouse buttons
        if is_mouse_button_down(
            &mut rl,
            MouseButton::MOUSE_BUTTON_MIDDLE,
            &mut mouse_buttons_pressed_this_frame,
        ) {
            apply_mouse_drag_to_camera(state.mouse_pos, last_mouse_pos, &mut state.camera);
        }

        let mouse_wheel_diff = rl.get_mouse_wheel_move();
        if rl.is_key_up(KeyboardKey::KEY_LEFT_CONTROL) {
            apply_mouse_wheel_zoom(mouse_wheel_diff, &mut state.camera);
        }

        if rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL) {
            if state.mode == Mode::UsingTool(Tool::Brush) {
                apply_mouse_wheel_brush_size(mouse_wheel_diff, &mut brush);
            }

            if state.mode == Mode::UsingTool(Tool::Text) || state.mode == Mode::TypingText {
                apply_mouse_wheel_text_size(mouse_wheel_diff, &mut state.text_size);
            }
        }

        clamp_brush_size(&mut brush);

        clamp_camera_zoom(&mut state.camera);

        last_mouse_pos = state.mouse_pos;

        let camera_view_boundary = rrect(
            state.camera.offset.x / state.camera.zoom + state.camera.target.x
                - screen_width as f32 / state.camera.zoom,
            state.camera.offset.y / state.camera.zoom + state.camera.target.y
                - (screen_height as f32 / state.camera.zoom),
            screen_width as f32 / state.camera.zoom,
            screen_height as f32 / state.camera.zoom,
        );

        if state.is_playing_inputs {
            let should_quit = replay_inputs(&mut state, &test_options, &automation_events);
            if should_quit {
                return;
            }
        }

        sync_image_textures(&mut image_textures, &state.things, &mut rl, &rl_thread);

        {
            let mut drawing = rl.begin_drawing(&rl_thread);
            {
                let mut drawing_camera = drawing.begin_mode2D(state.camera);

                drawing_camera.clear_background(state.background_color.0);

                // Render from back to front. Each pass preserves insertion order within its layer
                // and avoids allocating or sorting a render list every frame.
                for layer in RENDER_LAYERS {
                    for (thing_key, thing) in &state.things {
                        if thing.kind.render_layer() != layer {
                            continue;
                        }

                        let is_dragging_selected = state.mode == Mode::UsingTool(Tool::Move)
                            && working_move.is_some()
                            && state.selected_things.contains(&thing_key);
                        if !is_dragging_selected
                            && is_thing_in_camera_view(&camera_view_boundary, thing)
                        {
                            draw_thing(
                                &mut drawing_camera,
                                thing,
                                image_textures.get(&thing_key).map(|(_, texture)| texture),
                            );
                        }
                    }
                }

                // Debug and selection decoration are overlays and should not be covered by content.
                if debugging {
                    draw_bounding_boxes(&state.things, &mut drawing_camera, &font);
                }
                if working_move.is_none() {
                    for selected_key in &state.selected_things {
                        if let Some(thing) = state.things.get(*selected_key) {
                            if let Some(bounds) = thing.bounding_box(&font) {
                                drawing_camera.draw_rectangle_lines_ex(
                                    bounds.rect(),
                                    1.0,
                                    Color::DARKRED,
                                );
                            }
                        }
                    }
                }

                if let Some(drag_box) = state.mouse_drag_box {
                    drawing_camera.draw_rectangle_lines_ex(drag_box.rect(), 1.0, Color::TEAL);
                }

                // Draw move preview when dragging selected items
                if state.mode == Mode::UsingTool(Tool::Move) {
                    if let Some(working_move) = working_move {
                        let move_diff = working_move.1 - working_move.0;

                        // Keep the same back-to-front ordering in the move preview.
                        for layer in RENDER_LAYERS {
                            for selected_key in &state.selected_things {
                                if let Some(thing) = state.things.get(*selected_key) {
                                    if thing.kind.render_layer() == layer {
                                        draw_thing_at_offset(
                                            &mut drawing_camera,
                                            thing,
                                            move_diff,
                                            image_textures
                                                .get(selected_key)
                                                .map(|(_, texture)| texture),
                                        );
                                    }
                                }
                            }
                        }

                        // Draw bounding boxes for the preview
                        for selected_key in &state.selected_things {
                            if let Some(thing) = state.things.get(*selected_key) {
                                if let Some(mut bbox) = thing.bounding_box(&font) {
                                    // Offset the bounding box to show preview position
                                    bbox.min.x += move_diff.x;
                                    bbox.min.y += move_diff.y;
                                    bbox.max.x += move_diff.x;
                                    bbox.max.y += move_diff.y;
                                    drawing_camera.draw_rectangle_lines_ex(
                                        bbox.rect(),
                                        1.0,
                                        Color::LIME,
                                    );
                                }
                            }
                        }
                    }

                    if working_move.is_none() {
                        if let Some(bounds) = resizable_selection_bounds(&state, &font) {
                            drawing_camera.draw_rectangle_rec(
                                resize_handle(bounds, state.camera.zoom),
                                Color::DARKRED,
                            );
                        }
                    }
                }

                // TODO: Do we want to treat the working_stroke as a special case to draw?
                draw_stroke(
                    &mut drawing_camera,
                    &working_stroke,
                    working_stroke.brush_size,
                );

                // Draw "world space" GUI elements for the current mode
                if should_show_brush_marker(state.mode) {
                    draw_brush_marker(&mut drawing_camera, mouse_drawing_pos, &brush);
                }

                if state.mode == Mode::UsingTool(Tool::Text) {
                    drawing_camera.draw_text(
                        "Your text here",
                        mouse_drawing_pos.x as i32,
                        mouse_drawing_pos.y as i32,
                        state.text_size.0 as i32,
                        state.text_color.0,
                    );
                }

                if let Some(working_text) = &working_text {
                    if let Some(pos) = working_text.position {
                        drawing_camera.draw_text(
                            &working_text.content,
                            pos.x as i32,
                            pos.y as i32,
                            state.text_size.0 as i32,
                            state.text_color.0,
                        );
                    }
                }

                if debugging {
                    debug_draw_center_crosshair(
                        &mut drawing_camera,
                        &state,
                        screen_width,
                        screen_height,
                    );
                }
            }

            // Draw non "world space" GUI elements for the current mode
            match state.mode {
                Mode::UsingTool(Tool::ColorPicker) => {
                    draw_color_dropper_preview(
                        &mut drawing,
                        state.mouse_pos,
                        screen_height,
                        outline_color,
                        pixel_color_at_mouse_pos,
                    );

                    draw_color_dropper_icon(
                        &mut drawing,
                        state.mouse_pos,
                        color_dropper_scaled_width,
                        color_dropper_scaled_height,
                        &color_dropper_icon,
                        color_dropper_source_rect,
                    );
                }
                Mode::PickingBackgroundColor(_) => {}
                Mode::TypingText => {}
                Mode::ShowingKeymapPanel => {}
                Mode::UsingTool(_) => {}
            }

            if let Mode::PickingBackgroundColor(color_picker) = state.mode {
                state.background_color.0 =
                    drawing.gui_color_picker(color_picker.bounds, "", state.background_color.0);
            }

            if let Some(picker_info) = &mut color_picker_info {
                if state.using_text_tool_or_typing() {
                    state.text_color.0 =
                        drawing.gui_color_picker(picker_info.bounds, "", state.text_color.0);
                    if let Some(ref mut text) = working_text {
                        text.color = state.text_color;
                    }
                }

                if state.mode == Mode::UsingTool(Tool::Brush) && !is_drawing {
                    // Hide when not drawing
                    state.foreground_color.0 =
                        drawing.gui_color_picker(picker_info.bounds, "", state.foreground_color.0);
                }
                // TODO: Scale the GUI?
                if debugging {
                    drawing.draw_rectangle_lines_ex(
                        picker_info.bounds_with_slider(),
                        1.0,
                        Color::GOLD,
                    );
                }
            }

            if state.mode == Mode::ShowingKeymapPanel {
                let letter_spacing = 4.0;
                draw_keymap(
                    &mut drawing,
                    &keymap,
                    keymap_panel_bounds,
                    &font,
                    ui_font_size,
                    letter_spacing,
                );
            }

            draw_info_ui(&mut drawing, &state, &brush);

            if debugging {
                debug_draw_info(&mut drawing, &state, mouse_drawing_pos, current_fps);
            }
        }

        let elapsed = start_time.elapsed();
        if elapsed < duration_per_frame {
            let time_to_sleep = duration_per_frame - elapsed;
            thread::sleep(time_to_sleep);
        }

        for (button, was_pressed) in mouse_buttons_pressed_last_frame.iter_mut() {
            *was_pressed = *mouse_buttons_pressed_this_frame.get(button).unwrap();
        }
        for (_, was_pressed) in mouse_buttons_pressed_this_frame.iter_mut() {
            *was_pressed = false;
        }
    }
}

const RESIZE_HANDLE_SCREEN_SIZE: f32 = 14.0;

struct WorkingResize {
    anchor: Vector2,
    diagonal: Vector2,
    changes: Vec<ResizeChange>,
}

fn resizable_selection_bounds(state: &State, font: &WeakFont) -> Option<BoundingBox2D> {
    state
        .selected_things
        .iter()
        .filter_map(|key| state.things.get(*key))
        .filter(|thing| matches!(thing.kind, Renderable::Text(_) | Renderable::Image(_)))
        .filter_map(|thing| thing.bounding_box(font))
        .fold(None::<BoundingBox2D>, |combined, bounds| {
            Some(match combined {
                None => bounds,
                Some(combined) => BoundingBox2D {
                    min: rvec2(
                        combined.min.x.min(bounds.min.x),
                        combined.min.y.min(bounds.min.y),
                    ),
                    max: rvec2(
                        combined.max.x.max(bounds.max.x),
                        combined.max.y.max(bounds.max.y),
                    ),
                },
            })
        })
}

fn resize_handle(bounds: BoundingBox2D, zoom: f32) -> Rectangle {
    let size = RESIZE_HANDLE_SCREEN_SIZE / zoom;
    rrect(
        bounds.max.x - size / 2.0,
        bounds.max.y - size / 2.0,
        size,
        size,
    )
}

fn resize_scale(resize: &WorkingResize, mouse_position: Vector2) -> f32 {
    let current_diagonal = mouse_position - resize.anchor;
    let denominator = resize.diagonal.x.powi(2) + resize.diagonal.y.powi(2);
    if denominator <= f32::EPSILON {
        return 1.0;
    }

    ((current_diagonal.x * resize.diagonal.x + current_diagonal.y * resize.diagonal.y)
        / denominator)
        .max(0.05)
}

fn sync_image_textures(
    textures: &mut HashMap<ThingKey, (usize, Texture2D)>,
    things: &Things,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
) {
    textures.retain(|key, _| {
        matches!(
            things.get(*key).map(|thing| &thing.kind),
            Some(Renderable::Image(_))
        )
    });

    for (key, thing) in things {
        let Renderable::Image(canvas_image) = &thing.kind else {
            continue;
        };
        // SlotMap keys can be reused when a different drawing is loaded. The allocation identity
        // lets us invalidate that key without hashing every embedded image on every frame.
        let data_identity = canvas_image.png_data.as_ptr() as usize;
        if textures
            .get(&key)
            .is_some_and(|(cached_identity, _)| *cached_identity == data_identity)
        {
            continue;
        }

        let image = match Image::load_image_from_mem(".png", &canvas_image.png_data) {
            Ok(image) => image,
            Err(err) => {
                error!("Could not decode pasted image: {err}");
                continue;
            }
        };
        match rl.load_texture_from_image(thread, &image) {
            Ok(texture) => {
                textures.insert(key, (data_identity, texture));
            }
            Err(err) => error!("Could not upload pasted image to the GPU: {err}"),
        }
    }
}

fn apply_mouse_drag_to_camera(mouse_pos: Vector2, last_mouse_pos: Vector2, camera: &mut Camera2D) {
    // TODO(reece): Dragging and drawing can be done together at the moment, but it's very jaggy
    let mouse_diff = mouse_pos - last_mouse_pos;
    camera.target.x -= mouse_diff.x / camera.zoom;
    camera.target.y -= mouse_diff.y / camera.zoom;
}

fn apply_mouse_wheel_zoom(mouse_wheel_diff: f32, camera: &mut Camera2D) {
    let mouse_wheel_zoom_dampening = 0.065;
    // TODO: FIXME: This stuff "works" but it's an awful experience. Seems way worse when the window is a
    // smaller portion of the overall screen size due to scaling
    camera.zoom += mouse_wheel_diff * mouse_wheel_zoom_dampening;
}

fn apply_mouse_wheel_brush_size(mouse_wheel_diff: f32, brush: &mut Brush) {
    let mouse_wheel_amplifying = 3.50;
    brush.brush_size += mouse_wheel_diff * mouse_wheel_amplifying;
}

fn apply_mouse_wheel_text_size(mouse_wheel_diff: f32, current_text_size: &mut TextSize) {
    // TODO: FIXME: Decreasing text size can be different from increasing text size due to text
    // size being a float. It might be easier if we treat text size as a float, then cast down when
    // we need it to draw. Idk if that 'makes sense' as a float, but it would make working with it
    // easier

    // TODO: FIXME: Scrolling to change text size feels really shitty as the text size gets larger
    let new_size_diff = cmp::max(mouse_wheel_diff as u32, 1);

    if mouse_wheel_diff == 0.0 {
        return;
    }

    if mouse_wheel_diff > 0.0 {
        current_text_size.0 = current_text_size.0.saturating_add(new_size_diff)
    } else {
        current_text_size.0 = current_text_size.0.saturating_sub(new_size_diff)
    }
}

fn clamp_camera_zoom(camera: &mut Camera2D) {
    camera.zoom = camera.zoom.clamp(0.1, 10.0);
}

fn clamp_brush_size(brush: &mut Brush) {
    if brush.brush_size < 1.0 {
        brush.brush_size = 1.0;
    }
}

fn is_thing_in_camera_view(camera_boundary: &Rectangle, thing: &Thing) -> bool {
    match &thing.kind {
        Renderable::Image(image) => camera_boundary.check_collision_recs(&image.rect),
        Renderable::Text(text) => text.position.is_some_and(|position| {
            let approximate_bounds = rrect(
                position.x,
                position.y,
                (text.size.0 as usize * text.content.len()) as f32,
                text.size.0 as f32,
            );
            camera_boundary.check_collision_recs(&approximate_bounds)
        }),
        Renderable::Stroke(stroke) => is_stroke_in_camera_view(camera_boundary, stroke),
    }
}

fn is_stroke_in_camera_view(camera_boundary: &Rectangle, stroke: &Stroke) -> bool {
    for point in &stroke.points {
        if camera_boundary.check_collision_point_rec(point) {
            return true;
        }
    }
    return false;
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Point {
    pub x: f32,
    pub y: f32,
}

impl From<&Point> for ffi::Vector2 {
    fn from(val: &Point) -> Self {
        ffi::Vector2 { x: val.x, y: val.y }
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_str = format!("{},{}", self.x, self.y);
        f.write_str(&display_str)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Stroke {
    pub points: Vec<Point>,
    pub color: Color,
    pub brush_size: f32,
    // TODO(reece): Could store the brush used in the stroke so we know the parameters of each
    // stroke
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum Renderable {
    Stroke(Stroke),
    Text(Text),
    Image(CanvasImage),
}

#[derive(Clone, Copy, PartialEq)]
enum RenderLayer {
    Image,
    Text,
    Stroke,
}

const RENDER_LAYERS: [RenderLayer; 3] =
    [RenderLayer::Image, RenderLayer::Text, RenderLayer::Stroke];

impl Renderable {
    fn render_layer(&self) -> RenderLayer {
        match self {
            Self::Image(_) => RenderLayer::Image,
            Self::Text(_) => RenderLayer::Text,
            Self::Stroke(_) => RenderLayer::Stroke,
        }
    }
}

/// A pasted image is positioned and selected like every other renderable. The compressed PNG is
/// embedded in the drawing so a save remains a single portable file; only the GPU texture is kept
/// outside serialized state.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct CanvasImage {
    pub rect: Rectangle,
    #[serde(with = "base64_bytes")]
    pub png_data: Vec<u8>,
}

mod base64_bytes {
    use super::*;
    use serde::de::Error as _;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64.decode(encoded).map_err(D::Error::custom)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Thing {
    pub kind: Renderable,
}

#[derive(Debug, Copy, Clone)]
pub struct BoundingBox2D {
    // Looks like Raylib doesn't like negative widths or heights for rectangle
    // drawing https://github.com/raysan5/raylib/issues/671, so we just keep track of the points
    // that we can convert to a rectangle later
    pub min: Vector2,
    pub max: Vector2,
}

impl BoundingBox2D {
    pub fn rect(&self) -> Rectangle {
        let x = self.min.x.min(self.max.x);
        let y = self.min.y.min(self.max.y);
        let width = (self.min.x - self.max.x).abs();
        let height = (self.min.y - self.max.y).abs();

        Rectangle {
            x,
            y,
            width,
            height,
        }
    }
}

impl Thing {
    pub fn bounding_box(&self, font: &raylib::text::WeakFont) -> Option<BoundingBox2D> {
        match &self.kind {
            Renderable::Stroke(stroke) => {
                let (min_x, max_x, min_y, max_y) = stroke.points.iter().fold(
                    (
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                    ),
                    |(min_x, max_x, min_y, max_y), point| {
                        (
                            min_x.min(point.x),
                            max_x.max(point.x),
                            min_y.min(point.y),
                            max_y.max(point.y),
                        )
                    },
                );

                return Some(BoundingBox2D {
                    min: rvec2(min_x, min_y),
                    max: rvec2(max_x, max_y),
                });
            }
            Renderable::Text(text) => {
                if let Some(position) = text.position {
                    let c_text = std::ffi::CString::new(text.content.as_str()).unwrap();
                    let ffi_font: &raylib::ffi::Font = font.as_ref();
                    let text_dimensions = unsafe {
                        raylib::ffi::MeasureTextEx(
                            *ffi_font,
                            c_text.as_ptr(),
                            text.size.0 as f32,
                            0.0,
                        )
                    };

                    // Add a proportional buffer to the measured dimensions. Bounding box is too
                    // small in longer strings otherwise.
                    // This scales with the actual text size, not a fixed amount
                    let width_buffer = text_dimensions.x * 0.25;

                    return Some(BoundingBox2D {
                        min: position,
                        max: rvec2(
                            position.x + text_dimensions.x + width_buffer,
                            position.y + text_dimensions.y,
                        ),
                    });
                }
                None
            }
            Renderable::Image(image) => Some(BoundingBox2D {
                min: rvec2(image.rect.x, image.rect.y),
                max: rvec2(
                    image.rect.x + image.rect.width,
                    image.rect.y + image.rect.height,
                ),
            }),
        }
    }
}

impl Stroke {
    pub fn new(color: Color, brush_size: f32) -> Self {
        let default_num_of_points = 30;
        Stroke {
            points: Vec::with_capacity(default_num_of_points),
            color,
            brush_size,
        }
    }
}

// TODO: things and things_graveyard should have different key types probably
new_key_type! { pub(crate) struct ThingKey; }
pub(crate) type Things = SlotMap<ThingKey, Thing>;

new_key_type! { pub(crate) struct TextKey; }
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub(crate) enum ResizeGeometry {
    Text { position: Vector2, size: TextSize },
    Image { rect: Rectangle },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub(crate) struct ResizeChange {
    pub key: ThingKey,
    pub before: ResizeGeometry,
    pub after: ResizeGeometry,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum Action {
    AddThing(ThingKey),
    RemoveThing(ThingKey),
    MoveThings(Vec<ThingKey>, Vector2),
    ResizeThings(Vec<ResizeChange>),
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Text {
    pub content: String,
    pub position: Option<Vector2>,
    #[serde(default)]
    pub size: TextSize,
    #[serde(default)]
    pub color: TextColor,
    // TODO: Add support for different fonts per text
}

type CameraZoomPercentageDiff = i32;
type DiffPerSecond = i32;

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum HoldCommand {
    CameraZoom(CameraZoomPercentageDiff),
    PanCameraHorizontal(DiffPerSecond),
    PanCameraVertical(DiffPerSecond),
    ChangeBrushSize(DiffPerSecond),
    ChangeTextSize(DiffPerSecond),
    SpawnBrushStrokes,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) enum PressCommand {
    Undo,
    Redo,
    UseTextTool,
    ToggleDebugging,
    Save,
    SaveAs,
    Load,
    ChangeBrushType(BrushType),
    PickBackgroundColor,
    ToggleKeymapWindow,
    UseColorPicker,
    ToggleRecording,
    LoadAndPlayRecordedInputs,
    UseSelectionPicker,
}

type KeyboardKeyCombo = Vec<KeyboardKey>;
type PressKeyMappings = Vec<(KeyboardKeyCombo, PressCommand)>;
type HoldKeyMappings = Vec<(KeyboardKey, HoldCommand)>;

pub(crate) struct Keymap {
    pub on_press: PressKeyMappings,
    pub on_hold: HoldKeyMappings,
}

fn default_keymap() -> Keymap {
    let on_press = PressKeyMappings::from([
        (vec![KeyboardKey::KEY_M], PressCommand::ToggleDebugging),
        (
            vec![KeyboardKey::KEY_S, KeyboardKey::KEY_LEFT_CONTROL],
            PressCommand::Save,
        ),
        (
            vec![
                KeyboardKey::KEY_S,
                KeyboardKey::KEY_LEFT_CONTROL,
                KeyboardKey::KEY_LEFT_ALT,
            ],
            PressCommand::SaveAs,
        ),
        (
            vec![KeyboardKey::KEY_O, KeyboardKey::KEY_LEFT_CONTROL],
            PressCommand::Load,
        ),
        (vec![KeyboardKey::KEY_Z], PressCommand::Undo),
        (vec![KeyboardKey::KEY_R], PressCommand::Redo),
        (
            vec![KeyboardKey::KEY_E],
            PressCommand::ChangeBrushType(BrushType::Deleting),
        ),
        (
            vec![KeyboardKey::KEY_Q],
            PressCommand::ChangeBrushType(BrushType::Drawing),
        ),
        (vec![KeyboardKey::KEY_T], PressCommand::UseTextTool),
        (vec![KeyboardKey::KEY_B], PressCommand::PickBackgroundColor),
        (
            vec![KeyboardKey::KEY_SLASH],
            PressCommand::ToggleKeymapWindow,
        ),
        (vec![KeyboardKey::KEY_C], PressCommand::UseColorPicker),
        (vec![KeyboardKey::KEY_V], PressCommand::ToggleRecording),
        (
            vec![KeyboardKey::KEY_APOSTROPHE],
            PressCommand::LoadAndPlayRecordedInputs,
        ),
        (vec![KeyboardKey::KEY_G], PressCommand::UseSelectionPicker),
    ]);
    let on_hold = HoldKeyMappings::from([
        (KeyboardKey::KEY_A, HoldCommand::PanCameraHorizontal(-250)),
        (KeyboardKey::KEY_D, HoldCommand::PanCameraHorizontal(250)),
        (KeyboardKey::KEY_S, HoldCommand::PanCameraVertical(250)),
        (KeyboardKey::KEY_W, HoldCommand::PanCameraVertical(-250)),
        (KeyboardKey::KEY_L, HoldCommand::CameraZoom(-5)),
        (KeyboardKey::KEY_K, HoldCommand::CameraZoom(5)),
        (
            KeyboardKey::KEY_LEFT_BRACKET,
            HoldCommand::ChangeBrushSize(-50),
        ),
        (
            KeyboardKey::KEY_RIGHT_BRACKET,
            HoldCommand::ChangeBrushSize(50),
        ),
        (
            KeyboardKey::KEY_LEFT_BRACKET,
            HoldCommand::ChangeTextSize(-50),
        ),
        (
            KeyboardKey::KEY_RIGHT_BRACKET,
            HoldCommand::ChangeTextSize(50),
        ),
        (KeyboardKey::KEY_H, HoldCommand::SpawnBrushStrokes),
    ]);

    return Keymap { on_press, on_hold };
}

pub(crate) struct Brush {
    pub brush_type: BrushType,
    pub brush_size: f32,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) enum BrushType {
    Drawing,
    Deleting,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) enum Tool {
    Brush,
    Text,
    ColorPicker,
    Selection,
    Move,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Mode {
    UsingTool(Tool),
    PickingBackgroundColor(GuiColorPickerInfo),
    TypingText,
    ShowingKeymapPanel,
}

impl Default for Mode {
    fn default() -> Self {
        Self::UsingTool(Tool::Brush)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) struct GuiColorPickerInfo {
    pub initiation_pos: Vector2,
    pub bounds: Rectangle,
    /// The given bounds for the rgui color picker doesn't include the color slider bar at the
    /// side. Haven't looked too deeply into it, but the slider seems to be the same width
    /// regardless of the size of the color picker.
    pub picker_slider_x_padding: f32,
}

impl GuiColorPickerInfo {
    /// Returns the bounds of the color picker, including the color slider bar at the side.
    fn bounds_with_slider(&self) -> Rectangle {
        let mut bounds_with_picker = self.bounds;
        bounds_with_picker.width += self.picker_slider_x_padding;
        return bounds_with_picker;
    }
}
fn should_show_brush_marker(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::UsingTool(Tool::Brush) | Mode::ShowingKeymapPanel
    )
}

fn is_color_picker_active(color_picker_info: &Option<GuiColorPickerInfo>) -> bool {
    // TODO: REFACTOR: Feels like this should be a part of state, or gui state?
    return color_picker_info.is_some();
}

fn close_color_picker(
    color_picker_info: &mut Option<GuiColorPickerInfo>,
    color_picker_closed_this_frame: &mut bool,
) {
    // TODO: REFACTOR: This also feels like a gui state thing
    *color_picker_info = None;
    *color_picker_closed_this_frame = true;
}
