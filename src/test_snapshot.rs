use std::{fs::File, io::Write, path::Path};

use raylib::{
    color::Color,
    math::{Rectangle, Vector2},
};
use serde::Serialize;

use crate::{
    app::{CanvasImage, Point, Renderable, Stroke, Text},
    state::{State, TextColor, TextSize},
};

/// The observable drawing state used by replay tests.
///
/// This is deliberately separate from the persistence format. Internal details such as slot-map
/// keys, undo action variants, file paths, and transient input state must not invalidate a replay
/// test when they change.
#[derive(Debug, Serialize, PartialEq)]
pub struct StateSnapshot<'a> {
    things: Vec<SnapshotThing<'a>>,
    undo_depth: usize,
    redo_depth: usize,
    graveyard_size: usize,
    camera: SnapshotCamera,
    background_color: Color,
    foreground_color: Color,
    text_size: TextSize,
    text_color: TextColor,
}

#[derive(Debug, Serialize, PartialEq)]
enum SnapshotThing<'a> {
    Stroke(SnapshotStroke<'a>),
    Text(SnapshotText<'a>),
    Image(SnapshotImage<'a>),
}

#[derive(Debug, Serialize, PartialEq)]
struct SnapshotStroke<'a> {
    points: &'a [Point],
    color: Color,
    brush_size: f32,
}

impl<'a> From<&'a Stroke> for SnapshotStroke<'a> {
    fn from(stroke: &'a Stroke) -> Self {
        Self {
            points: &stroke.points,
            color: stroke.color,
            brush_size: stroke.brush_size,
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct SnapshotText<'a> {
    content: &'a str,
    position: Option<Vector2>,
    size: TextSize,
    color: TextColor,
}

impl<'a> From<&'a Text> for SnapshotText<'a> {
    fn from(text: &'a Text) -> Self {
        Self {
            content: &text.content,
            position: text.position,
            size: text.size,
            color: text.color,
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct SnapshotImage<'a> {
    rect: Rectangle,
    png_data: &'a [u8],
}

impl<'a> From<&'a CanvasImage> for SnapshotImage<'a> {
    fn from(image: &'a CanvasImage) -> Self {
        Self {
            rect: image.rect,
            png_data: &image.png_data,
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct SnapshotCamera {
    offset: Vector2,
    target: Vector2,
    rotation: f32,
    zoom: f32,
}

impl<'a> From<&'a State> for StateSnapshot<'a> {
    fn from(state: &'a State) -> Self {
        let things = state
            .things
            .values()
            .map(|thing| match &thing.kind {
                Renderable::Stroke(stroke) => SnapshotThing::Stroke(stroke.into()),
                Renderable::Text(text) => SnapshotThing::Text(text.into()),
                Renderable::Image(image) => SnapshotThing::Image(image.into()),
            })
            .collect();

        Self {
            things,
            undo_depth: state.undo_actions.len(),
            redo_depth: state.redo_actions.len(),
            graveyard_size: state.things_graveyard.len(),
            camera: SnapshotCamera {
                offset: state.camera.offset,
                target: state.camera.target,
                rotation: state.camera.rotation,
                zoom: state.camera.zoom,
            },
            background_color: state.background_color.0,
            foreground_color: state.foreground_color.0,
            text_size: state.text_size,
            text_color: state.text_color,
        }
    }
}

pub fn save(state: &State, path: &Path) -> Result<(), std::io::Error> {
    let output = serde_json::to_string_pretty(&StateSnapshot::from(state))?;
    let mut file = File::create(path)?;
    file.write_all(output.as_bytes())?;
    file.write_all(b"\n")
}
