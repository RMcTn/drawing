# Draw app
"Infinite" canvas drawing app for quick free hand diagramming with a bit of text.

Made for personal use so expect things to change often.


https://github.com/RMcTn/drawing/assets/18317099/b1c4285c-d16c-4665-9e13-d72e929d1f5f




## Limitations
#### Key bindings
Keys cannot be rebound yet until an interface is built for that.
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
