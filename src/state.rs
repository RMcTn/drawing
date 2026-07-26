use std::path::PathBuf;

use raylib::check_collision_circles;
use raylib::math::Vector2;
use raylib::{camera::Camera2D, color::Color};
use serde::{Deserialize, Serialize};

use crate::app::{
    Action, BoundingBox2D, Mode, Renderable, ResizeChange, ResizeGeometry, Thing, ThingKey, Things,
    Tool,
};

#[derive(Deserialize, Serialize)]
pub struct BackgroundColor(pub Color);

impl Default for BackgroundColor {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}

#[derive(Deserialize, Serialize)]
pub struct ForegroundColor(pub Color);

impl Default for ForegroundColor {
    fn default() -> Self {
        Self(Color::BLACK)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct TextColor(pub Color);

impl Default for TextColor {
    fn default() -> Self {
        Self(Color::BLACK)
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct State {
    pub things: Things,
    pub undo_actions: Vec<Action>,
    pub redo_actions: Vec<Action>,
    pub things_graveyard: Things,
    pub output_path: Option<PathBuf>,
    #[serde(with = "Camera2DDef")]
    #[serde(default)]
    pub camera: Camera2D,
    #[serde(default)]
    pub background_color: BackgroundColor,
    #[serde(default)]
    pub foreground_color: ForegroundColor,
    #[serde(skip)] // Don't think we want to save mode yet
    pub mode: Mode,
    pub mouse_pos: Vector2,
    #[serde(default)]
    pub text_size: TextSize,
    #[serde(default)]
    pub text_color: TextColor,
    #[serde(skip)]
    pub is_recording_inputs: bool,
    #[serde(skip)]
    pub is_playing_inputs: bool,
    // Not sure if frame related things belong here...
    #[serde(skip)]
    pub current_play_frame: usize,
    #[serde(skip)]
    pub play_frame_counter: usize,
    #[serde(skip)]
    pub selected_things: Vec<ThingKey>,
    #[serde(skip)]
    pub mouse_drag_box: Option<BoundingBox2D>,
    /// Unicode code points queued by app-specific replay text events.
    #[serde(skip)]
    pub replay_text_input: Vec<u32>,
    #[serde(default)]
    pub locations: Vec<Vector2>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct TextSize(pub u32);
impl Default for TextSize {
    fn default() -> Self {
        Self(50)
    }
}

impl State {
    pub fn add_thing_with_undo(&mut self, thing: Thing) {
        let key = self.add_thing(thing);
        self.undo_actions.push(Action::AddThing(key));
    }

    pub fn add_thing(&mut self, thing: Thing) -> ThingKey {
        self.things.insert(thing)
    }

    pub fn remove_thing(&mut self, key: ThingKey) -> Option<ThingKey> {
        if let Some(thing) = self.things.remove(key) {
            return Some(self.add_thing_to_graveyard(thing));
        }
        dbg!(
            "Tried to remove thing with key {} but it was already gone",
            key
        );

        None
    }

    pub fn move_things_with_undo(&mut self, thing_keys: &[ThingKey], move_diff: Vector2) {
        self.apply_move_to_things(thing_keys, move_diff);
        self.undo_actions
            .push(Action::MoveThings(thing_keys.to_vec(), move_diff));
    }

    pub fn apply_move_to_things(&mut self, thing_keys: &[ThingKey], move_diff: Vector2) {
        for key in thing_keys {
            if let Some(thing) = self.things.get_mut(*key) {
                match &mut thing.kind {
                    Renderable::Stroke(stroke) => {
                        for point in &mut stroke.points {
                            point.x += move_diff.x;
                            point.y += move_diff.y;
                        }
                    }
                    Renderable::Text(text) => {
                        if let Some(position) = &mut text.position {
                            position.x += move_diff.x;
                            position.y += move_diff.y;
                        }
                    }
                    Renderable::Image(image) => {
                        image.rect.x += move_diff.x;
                        image.rect.y += move_diff.y;
                    }
                }
            }
        }
    }

    pub fn capture_resize_changes(&self, thing_keys: &[ThingKey]) -> Vec<ResizeChange> {
        thing_keys
            .iter()
            .filter_map(|key| {
                let geometry = match &self.things.get(*key)?.kind {
                    Renderable::Text(text) => ResizeGeometry::Text {
                        position: text.position?,
                        size: text.size,
                    },
                    Renderable::Image(image) => ResizeGeometry::Image { rect: image.rect },
                    Renderable::Stroke(_) => return None,
                };
                Some(ResizeChange {
                    key: *key,
                    before: geometry,
                    after: geometry,
                })
            })
            .collect()
    }

    pub fn preview_resize(&mut self, changes: &mut [ResizeChange], anchor: Vector2, scale: f32) {
        for change in changes {
            change.after = match change.before {
                ResizeGeometry::Text { position, size } => ResizeGeometry::Text {
                    position: anchor + (position - anchor) * scale,
                    size: TextSize(((size.0 as f32 * scale).round() as u32).max(1)),
                },
                ResizeGeometry::Image { rect } => ResizeGeometry::Image {
                    rect: raylib::math::rrect(
                        anchor.x + (rect.x - anchor.x) * scale,
                        anchor.y + (rect.y - anchor.y) * scale,
                        rect.width * scale,
                        rect.height * scale,
                    ),
                },
            };
            self.apply_resize_geometry(change.key, change.after);
        }
    }

    pub fn commit_resize_with_undo(&mut self, changes: Vec<ResizeChange>) {
        let changes: Vec<_> = changes
            .into_iter()
            .filter(|change| change.before != change.after)
            .collect();
        if !changes.is_empty() {
            self.undo_actions.push(Action::ResizeThings(changes));
        }
    }

    fn apply_resize_geometry(&mut self, key: ThingKey, geometry: ResizeGeometry) {
        let Some(thing) = self.things.get_mut(key) else {
            return;
        };
        match (&mut thing.kind, geometry) {
            (Renderable::Text(text), ResizeGeometry::Text { position, size }) => {
                text.position = Some(position);
                text.size = size;
            }
            (Renderable::Image(image), ResizeGeometry::Image { rect }) => image.rect = rect,
            _ => {}
        }
    }

    fn apply_resize_changes(&mut self, changes: &[ResizeChange], use_after: bool) {
        for change in changes {
            let geometry = if use_after {
                change.after
            } else {
                change.before
            };
            self.apply_resize_geometry(change.key, geometry);
        }
    }

    pub fn add_thing_to_graveyard(&mut self, thing: Thing) -> ThingKey {
        self.things_graveyard.insert(thing)
    }

    pub fn restore_thing(&mut self, key: ThingKey) -> Option<ThingKey> {
        if let Some(thing) = self.things_graveyard.remove(key) {
            return Some(self.add_thing(thing));
        }
        dbg!(
            "Tried to restore thing with key {} but it couldn't find it",
            key
        );

        None
    }

    pub fn undo(&mut self) {
        loop {
            if let Some(action) = self.undo_actions.pop() {
                match action {
                    Action::AddThing(key) => {
                        if let Some(new_key) = self.remove_thing(key) {
                            self.redo_actions.push(Action::AddThing(new_key));
                            break;
                        }
                    }
                    Action::RemoveThing(key) => {
                        if let Some(new_key) = self.restore_thing(key) {
                            self.redo_actions.push(Action::RemoveThing(new_key));
                            break;
                        }
                    }
                    Action::MoveThings(thing_keys, move_diff) => {
                        // Undo by applying the inverse move
                        let inverse_diff = Vector2 {
                            x: -move_diff.x,
                            y: -move_diff.y,
                        };
                        self.apply_move_to_things(&thing_keys, inverse_diff);
                        self.redo_actions
                            .push(Action::MoveThings(thing_keys, move_diff));
                        break;
                    }
                    Action::ResizeThings(changes) => {
                        self.apply_resize_changes(&changes, false);
                        self.redo_actions.push(Action::ResizeThings(changes));
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn redo(&mut self) {
        loop {
            if let Some(action) = self.redo_actions.pop() {
                match action {
                    Action::AddThing(key) => {
                        if let Some(new_key) = self.restore_thing(key) {
                            self.undo_actions.push(Action::AddThing(new_key));
                            break;
                        }
                    }
                    Action::RemoveThing(key) => {
                        if let Some(new_key) = self.remove_thing(key) {
                            self.undo_actions.push(Action::RemoveThing(new_key));
                            break;
                        }
                    }
                    Action::MoveThings(thing_keys, move_diff) => {
                        // Redo by applying the original move again
                        self.apply_move_to_things(&thing_keys, move_diff);
                        self.undo_actions
                            .push(Action::MoveThings(thing_keys, move_diff));
                        break;
                    }
                    Action::ResizeThings(changes) => {
                        self.apply_resize_changes(&changes, true);
                        self.undo_actions.push(Action::ResizeThings(changes));
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn strokes_within_point(&self, mouse_point: Vector2, brush_size: f32) -> Vec<ThingKey> {
        let mut strokes = vec![];
        for (k, thing) in &self.things {
            match &thing.kind {
                Renderable::Stroke(stroke) => {
                    for point in &stroke.points {
                        if check_collision_circles(
                            Vector2 {
                                x: point.x,
                                y: point.y,
                            },
                            stroke.brush_size / 2.0,
                            mouse_point,
                            brush_size / 2.0,
                        ) {
                            strokes.push(k);
                            break;
                        }
                    }
                }
                _ => continue,
            }
        }
        strokes
    }

    pub fn delete_strokes(&mut self, stroke_keys: Vec<ThingKey>) {
        for key in stroke_keys {
            if let Some(new_key) = self.remove_thing(key) {
                self.undo_actions.push(Action::RemoveThing(new_key));
            }
        }
    }

    pub fn using_text_tool_or_typing(&self) -> bool {
        return self.mode == Mode::UsingTool(Tool::Text) || self.mode == Mode::TypingText;
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Camera2D")]
/// Exists so we can serialize the raylib camera
struct Camera2DDef {
    offset: Vector2,
    target: Vector2,
    rotation: f32,
    zoom: f32,
}

#[cfg(test)]
mod tests {
    use raylib::prelude::Color;

    use crate::{
        app::{CanvasImage, Renderable, Stroke, Text, Thing},
        state::{TextColor, TextSize},
    };

    use super::State;

    #[test]
    fn it_undoes_and_redoes_strokes_and_text() {
        let mut state = State::default();
        let stroke = Stroke {
            points: vec![],
            color: Color::BLACK,
            brush_size: 10.0,
        };

        let stroke = Thing {
            kind: crate::app::Renderable::Stroke(stroke),
        };
        let text = Text {
            content: "Stuff".to_string(),
            position: None,
            size: TextSize(20),
            color: TextColor(Color::BLACK),
        };

        let text = Thing {
            kind: crate::app::Renderable::Text(text),
        };
        state.add_thing_with_undo(stroke);
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.things.len(), 0);
        assert_eq!(state.things_graveyard.len(), 1);

        state.add_thing_with_undo(text);

        state.redo();
        assert_eq!(state.things.len(), 2);
        assert_eq!(state.things_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 1);
        assert!(matches!(
            state.things.values().next().unwrap().kind,
            Renderable::Text(_)
        ));
        assert!(matches!(
            state.things_graveyard.values().next().unwrap().kind,
            Renderable::Stroke(_)
        ));
    }

    #[test]
    fn it_undoes_and_redoes_strokes() {
        let mut state = State::default();
        let stroke = Stroke {
            points: vec![],
            color: Color::BLACK,
            brush_size: 10.0,
        };

        let stroke = Thing {
            kind: crate::app::Renderable::Stroke(stroke),
        };
        state.add_thing_with_undo(stroke);
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.things.len(), 0);
        assert_eq!(state.things_graveyard.len(), 1);

        state.redo();
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 0);
    }

    #[test]
    fn it_resizes_text_and_images_with_undo() {
        let mut state = State::default();
        let text_key = state.add_thing(Thing {
            kind: Renderable::Text(Text {
                content: "Stuff".to_string(),
                position: Some(raylib::math::rvec2(10, 20)),
                size: TextSize(20),
                color: TextColor(Color::BLACK),
            }),
        });
        let image_key = state.add_thing(Thing {
            kind: Renderable::Image(CanvasImage {
                rect: raylib::math::rrect(30, 40, 100, 50),
                png_data: vec![],
            }),
        });

        let mut changes = state.capture_resize_changes(&[text_key, image_key]);
        state.preview_resize(&mut changes, raylib::math::rvec2(0, 0), 2.0);
        state.commit_resize_with_undo(changes);

        let Renderable::Text(text) = &state.things[text_key].kind else {
            panic!("expected text")
        };
        assert_eq!(text.position, Some(raylib::math::rvec2(20, 40)));
        assert_eq!(text.size, TextSize(40));
        let Renderable::Image(image) = &state.things[image_key].kind else {
            panic!("expected image")
        };
        assert_eq!(image.rect, raylib::math::rrect(60, 80, 200, 100));

        state.undo();
        let Renderable::Text(text) = &state.things[text_key].kind else {
            panic!("expected text")
        };
        assert_eq!(text.position, Some(raylib::math::rvec2(10, 20)));
        assert_eq!(text.size, TextSize(20));

        state.redo();
        let Renderable::Image(image) = &state.things[image_key].kind else {
            panic!("expected image")
        };
        assert_eq!(image.rect, raylib::math::rrect(60, 80, 200, 100));
    }

    #[test]
    fn it_undoes_and_redoes_text() {
        let mut state = State::default();
        let text = Text {
            content: "Stuff".to_string(),
            position: None,
            size: TextSize(20),
            color: TextColor(Color::BLACK),
        };

        let text = Thing {
            kind: crate::app::Renderable::Text(text),
        };
        state.add_thing_with_undo(text);
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.things.len(), 0);
        assert_eq!(state.things_graveyard.len(), 1);

        state.redo();
        assert_eq!(state.things.len(), 1);
        assert_eq!(state.things_graveyard.len(), 0);
    }
}
