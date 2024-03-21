use std::path::PathBuf;

use raylib::camera::Camera2D;
use raylib::check_collision_circles;
use raylib::math::Vector2;
use serde::{Deserialize, Serialize};
use slotmap::{DefaultKey, SlotMap};

use crate::{Action, Stroke, Strokes, Text, TextKey};

#[derive(Deserialize, Serialize)]
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
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Camera2D")]
struct Camera2DDef {
    offset: Vector2,
    target: Vector2,
    rotation: f32,
    zoom: f32,
}
