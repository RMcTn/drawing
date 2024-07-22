use std::path::PathBuf;

use raylib::check_collision_circles;
use raylib::math::Vector2;
use raylib::{camera::Camera2D, color::Color};
use serde::{Deserialize, Serialize};
use slotmap::{DefaultKey, SlotMap};

use crate::{Action, Mode, Stroke, Strokes, Text, TextKey};

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct TextColor(pub Color);

impl Default for TextColor {
    fn default() -> Self {
        Self(Color::BLACK)
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct State {
    pub strokes: Strokes,
    pub undo_actions: Vec<Action>,
    pub redo_actions: Vec<Action>,
    pub stroke_graveyard: Strokes,
    pub text: SlotMap<TextKey, Text>,
    pub text_graveyard: SlotMap<TextKey, Text>,
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
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct TextSize(pub u32);
impl Default for TextSize {
    fn default() -> Self {
        Self(50)
    }
}

impl State {
    pub fn add_stroke_with_undo(&mut self, stroke: Stroke) {
        let key = self.add_stroke(stroke);
        self.undo_actions.push(Action::AddStroke(key));
    }

    pub fn add_stroke(&mut self, stroke: Stroke) -> DefaultKey {
        self.strokes.insert(stroke)
    }

    pub fn remove_stroke(&mut self, key: DefaultKey) -> Option<DefaultKey> {
        if let Some(stroke) = self.strokes.remove(key) {
            return Some(self.add_stroke_to_graveyard(stroke));
        }
        dbg!(
            "Tried to remove stroke with key {} but it was already gone",
            key
        );

        None
    }

    pub fn add_stroke_to_graveyard(&mut self, stroke: Stroke) -> DefaultKey {
        self.stroke_graveyard.insert(stroke)
    }

    pub fn restore_stroke(&mut self, key: DefaultKey) -> Option<DefaultKey> {
        if let Some(stroke) = self.stroke_graveyard.remove(key) {
            return Some(self.add_stroke(stroke));
        }
        dbg!(
            "Tried to restore stroke with key {} but it couldn't find it",
            key
        );

        None
    }

    pub fn add_text_with_undo(&mut self, text: Text) {
        let key = self.add_text(text);
        self.undo_actions.push(Action::AddText(key));
    }

    pub fn add_text(&mut self, text: Text) -> TextKey {
        self.text.insert(text)
    }

    pub fn restore_text(&mut self, key: TextKey) -> Option<TextKey> {
        if let Some(text) = self.text_graveyard.remove(key) {
            return Some(self.add_text(text));
        }
        dbg!(
            "Tried to restore text with key {} but it couldn't find it",
            key
        );

        None
    }

    pub fn remove_text(&mut self, key: TextKey) -> Option<TextKey> {
        if let Some(text) = self.text.remove(key) {
            return Some(self.add_text_to_graveyard(text));
        }
        dbg!(
            "Tried to remove text with key {} but it was already gone",
            key
        );

        None
    }

    pub fn add_text_to_graveyard(&mut self, text: Text) -> TextKey {
        self.text_graveyard.insert(text)
    }

    pub fn undo(&mut self) {
        loop {
            if let Some(action) = self.undo_actions.pop() {
                match action {
                    Action::AddStroke(key) => {
                        if let Some(new_key) = self.remove_stroke(key) {
                            self.redo_actions.push(Action::AddStroke(new_key));
                            break;
                        }
                    }
                    Action::RemoveStroke(key) => {
                        if let Some(new_key) = self.restore_stroke(key) {
                            self.redo_actions.push(Action::RemoveStroke(new_key));
                            break;
                        }
                    }
                    Action::AddText(key) => {
                        if let Some(new_key) = self.remove_text(key) {
                            self.redo_actions.push(Action::AddText(new_key));
                            break;
                        }
                    }
                    Action::RemoveText(key) => {
                        if let Some(new_key) = self.restore_text(key) {
                            self.redo_actions.push(Action::RemoveText(new_key));
                            break;
                        }
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
                    Action::AddStroke(key) => {
                        if let Some(new_key) = self.restore_stroke(key) {
                            self.undo_actions.push(Action::AddStroke(new_key));
                            break;
                        }
                    }
                    Action::RemoveStroke(key) => {
                        if let Some(new_key) = self.remove_stroke(key) {
                            self.undo_actions.push(Action::RemoveStroke(new_key));
                            break;
                        }
                    }
                    Action::AddText(key) => {
                        if let Some(new_key) = self.restore_text(key) {
                            self.undo_actions.push(Action::AddText(new_key));
                            break;
                        }
                    }
                    Action::RemoveText(key) => {
                        if let Some(new_key) = self.remove_text(key) {
                            self.undo_actions.push(Action::RemoveText(new_key));
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn strokes_within_point(&self, mouse_point: Vector2, brush_size: f32) -> Vec<DefaultKey> {
        let mut strokes = vec![];
        for (k, stroke) in &self.strokes {
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
        strokes
    }

    pub fn delete_strokes(&mut self, stroke_keys: Vec<DefaultKey>) {
        for key in stroke_keys {
            if let Some(new_key) = self.remove_stroke(key) {
                self.undo_actions.push(Action::RemoveStroke(new_key));
            }
        }
    }

    pub fn using_text_tool_or_typing(&self) -> bool {
        return self.mode == Mode::UsingTool(crate::Tool::Text) || self.mode == Mode::TypingText;
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
        state::{TextColor, TextSize},
        Text,
    };

    use super::State;

    #[test]
    fn it_undoes_and_redoes_strokes() {
        let mut state = State::default();
        let stroke = crate::Stroke {
            points: vec![],
            color: Color::BLACK,
            brush_size: 10.0,
        };

        state.add_stroke_with_undo(stroke);
        assert_eq!(state.strokes.len(), 1);
        assert_eq!(state.stroke_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.strokes.len(), 0);
        assert_eq!(state.stroke_graveyard.len(), 1);

        state.redo();
        assert_eq!(state.strokes.len(), 1);
        assert_eq!(state.stroke_graveyard.len(), 0);
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

        state.add_text_with_undo(text);
        assert_eq!(state.text.len(), 1);
        assert_eq!(state.text_graveyard.len(), 0);

        state.undo();
        assert_eq!(state.text.len(), 0);
        assert_eq!(state.text_graveyard.len(), 1);

        state.redo();
        assert_eq!(state.text.len(), 1);
        assert_eq!(state.text_graveyard.len(), 0);
    }
}
