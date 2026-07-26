# Draw app
"Infinite" canvas drawing app for quick free hand diagramming with a bit of text.

Made for personal use so expect things to change often.


https://github.com/RMcTn/drawing/assets/18317099/b1c4285c-d16c-4665-9e13-d72e929d1f5f




## Limitations
#### Key bindings
Keys cannot be rebound yet until an interface is built for that.
#### Selection Tool
- Text and images can be resized using the handle at the bottom-right of a selection.
- TODO: Decide how a click selects one `Thing` when several overlap at that point.
- TODO: Allow editing an existing text element after selecting it.
- TODO: Allow changing the colour of selected things. A group colour change should apply to all
  supported selected types even when their current colours differ; images may remain unaffected.
- TODO: Decide how resizing should affect brush strokes (point scaling, brush-width scaling, or both).

#### Canvas and zoom
- TODO: Expand the zoom range in both directions so more canvas is usable. Verify coordinate
  precision, brush sizes, hit testing, selection handles, culling, and rendering at the new limits.

#### Text Tool
- Delete mode will only delete brush strokes at this time. Text cannot be removed.
- Text font cannot be changed.
- Text size cannot be changed.
- Text colour cannot be changed.
- Backspace cannot be held down to remove characters. Backspace must be pressed multiple times if needed.

### Save file
There is no stable save versioning at the moment. Save files can and will break with changes to the program until a stable save versioning system is implemented.

### Default Keys

Two types of key inputs:
- Press keys are actions that will trigger once and stop. Undo/Redo for example.
- Hold keys are actions that will repeatedly trigger whilst the key is held. Camera pan for example.

#### Press keys
| Key | Action |
|-----|--------|
| KEY_M | ToggleDebugging |
| KEY_S + Left Control | Save |
| KEY_S + Left Control + Left Alt | Save As |
| KEY_O + Left Control | Load |
| KEY_Z | Undo |
| KEY_R | Redo |
| KEY_E | ChangeBrushType to deleting |
| KEY_Q | ChangeBrushType to drawing |
| KEY_T | Change to Text Tool |
| KEY_B | Change background color |
| KEY_C | Change to Color Picker Tool |
| KEY_U | Record the current camera location |

#### Hold keys
| Key | Action |
|-----|--------|
| KEY_A | PanCameraHorizontal left |
| KEY_D | PanCameraHorizontal right |
| KEY_W | PanCameraVertical up |
| KEY_S | PanCameraVertical down |
| KEY_L | CameraZoom out |
| KEY_K | CameraZoom in |
| KEY_LEFT_BRACKET | ChangeBrushSize smaller in brush mode |
| KEY_RIGHT_BRACKET | ChangeBrushSize larger in brush mode |
| KEY_LEFT_BRACKET | ChangeTextSize smaller in text mode |
| KEY_RIGHT_BRACKET | ChangeTextSize larger in text mode |
| KEY_H | SpawnBrushStrokes |

#### Mouse inputs
| Input | Action |
|-----|--------|
| Left click | Draw |
| Right click | Brush tool Color palette |
| Right click | Text tool Color palette |
| Middle click | Pan |
| Mouse wheel | Zoom in/Zoom out |
| Ctrl + Mouse wheel | Change brush size in brush mode|
| Ctrl + Mouse wheel | Change text size in text or typing mode|

### Text Tool
With the Text Tool selected, hover the mouse where you want the text to begin and start to type. Press ENTER to finish.  
Press BACKSPACE to remove characters.

### Dependencies
TODO

### Build
TODO

### Testing

Run all unit tests and the input replay regression test with:

```sh
./run-test.sh
```

Each replay test lives in `tests/<name>/`, with `<name>.rae` as its input recording and
`expected-state.json` as its expected observable drawing state. `run-test.sh` discovers all such
directories. It intentionally does not compare application save files, so unrelated
persistence-format changes do not invalidate replay tests.

Raylib does not record typed characters. Replay files can use draw-app event type `24`, with a
Unicode code point in the first parameter, to replay text input deterministically.

To inspect a replay interactively, run it in debug mode:

```sh
cargo run -- test --replay-path tests/draw_with_color_change/draw_with_color_change.rae --debug-replay
```

The drawing window remains responsive and provides buttons for stepping one recorded frame, one
event, or one complete left-mouse stroke, and for continuing playback. The same controls are
available as `F6`, `F7`, `F8`, and `F5`; press `F5` again to pause continuous playback. A stroke
step also completes a stroke that was partially advanced with frame or event steps. While paused,
rendering continues but application input simulation and regular app controls do not mutate state.

Saving and loading are covered separately by a Rust round-trip unit test in
`src/persistence.rs`.
