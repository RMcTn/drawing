use std::path::Path;

use log::{debug, error, info};
use raylib::{
    automation::{AutomationEvent, AutomationEventList},
    ffi, RaylibHandle,
};

use crate::{app::TestSettings, persistence, state::State, test_snapshot};

/// App-specific automation event. `params[0]` contains a Unicode code point.
/// Raylib recordings only capture key states and cannot reproduce text character input.
const INPUT_KEY_UP: u32 = 1;
const INPUT_KEY_DOWN: u32 = 2;
const INPUT_TEXT_CODEPOINT: u32 = 24;
const INPUT_MOUSE_BUTTON_UP: u32 = 5;
const INPUT_MOUSE_BUTTON_DOWN: u32 = 6;
const INPUT_MOUSE_POSITION: u32 = 7;
const MOUSE_BUTTON_LEFT: i32 = 0;

pub fn load_replay(
    replay_path: &Path,
    rl: &RaylibHandle,
    automation_events_list: &mut AutomationEventList,
    automation_events: &mut Vec<AutomationEvent>,
) -> Option<()> {
    debug!("Trying to load replay from {:?}", replay_path);
    let loaded_automated_events = rl.load_automation_event_list(Some(replay_path.into()));
    if loaded_automated_events.count() == 0 {
        error!(
            "Couldn't load automated event list from {}, or it was empty",
            replay_path.display()
        );
        return None;
    }

    *automation_events_list = loaded_automated_events;
    rl.set_automation_event_list(automation_events_list);
    rl.set_automation_event_base_frame(0);
    *automation_events = automation_events_list.events();

    info!(
        "Successfully loaded automated event list from {}",
        replay_path.display(),
    );
    Some(())
}

pub fn play_replay(state: &mut State) {
    state.is_playing_inputs = true;
    state.current_play_frame = 0;
    state.play_frame_counter = 0;
}

pub fn stop_replay(state: &mut State) {
    state.is_playing_inputs = false;
    state.current_play_frame = 0;
    state.play_frame_counter = 0;
}

fn play_event(state: &mut State, event: &AutomationEvent) {
    debug!(
        "Event {:?}: type {:?}",
        state.current_play_frame,
        event.get_type()
    );
    if event.get_type() == INPUT_TEXT_CODEPOINT {
        state.replay_text_input.push(event.params()[0] as u32);
    } else {
        event.play();
    }
    state.current_play_frame += 1;
}

fn finish_replay(state: &mut State, test_options: &Option<TestSettings>) -> bool {
    stop_replay(state);
    info!("Finished playing replay");

    if let Some(test_options) = test_options {
        if test_options.save_after_replay {
            let save_path = test_options
                .save_path
                .as_deref()
                .expect("clap requires --save-path with --save-after-replay");
            match persistence::save(state, save_path) {
                Ok(_) => info!("Successfully saved to {}", save_path.display()),
                Err(e) => error!("Failed to save to {}: {}", save_path.display(), e),
            }
        }

        if let Some(snapshot_path) = &test_options.snapshot_path {
            if let Err(e) = test_snapshot::save(state, snapshot_path) {
                error!(
                    "Failed to write replay state snapshot to {}: {}",
                    snapshot_path.display(),
                    e
                );
            }
        }

        if test_options.quit_after_replay {
            return true;
        }
    }
    false
}

/// Runs replay at its recorded frame boundaries. Returns true if the program should exit.
pub fn replay_inputs(
    state: &mut State,
    test_options: &Option<TestSettings>,
    automation_events: &[AutomationEvent],
) -> bool {
    while state.current_play_frame < automation_events.len()
        && state.play_frame_counter == automation_events[state.current_play_frame].frame() as usize
    {
        let event = &automation_events[state.current_play_frame];
        play_event(state, event);
        if state.current_play_frame == automation_events.len() {
            return finish_replay(state, test_options);
        }
    }
    state.play_frame_counter += 1;
    false
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DebugStep {
    Frame,
    Event,
    MouseStroke,
    Continue,
}

/// Replay scheduling state for the in-window debugger. Rendering continues while this is paused;
/// `simulation_needed` permits exactly one application update for each scheduled replay frame.
pub struct ReplayDebugger {
    step: Option<DebugStep>,
    simulation_needed: bool,
    pause_after_simulation: bool,
    mouse_stroke_started: bool,
    left_mouse_down: bool,
    held_keys: [bool; 512],
    held_mouse_buttons: [bool; 8],
    mouse_position: (f32, f32),
    pending_finish: bool,
    waiting_for_ui_mouse_release: bool,
    finished: bool,
}

impl ReplayDebugger {
    pub fn new() -> Self {
        Self {
            step: None,
            simulation_needed: false,
            pause_after_simulation: false,
            mouse_stroke_started: false,
            left_mouse_down: false,
            held_keys: [false; 512],
            held_mouse_buttons: [false; 8],
            mouse_position: (0.0, 0.0),
            pending_finish: false,
            waiting_for_ui_mouse_release: false,
            finished: false,
        }
    }

    pub fn should_simulate(&self) -> bool {
        self.simulation_needed
    }

    pub fn is_running(&self) -> bool {
        self.step == Some(DebugStep::Continue)
    }

    pub fn is_paused(&self) -> bool {
        self.step.is_none() && !self.finished
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse_position
    }

    /// Rendering polls physical input even while replay simulation is paused. Restore replay-held
    /// states after that poll so the next simulation update sees the recorded input, not the
    /// developer's keyboard and mouse state.
    pub fn restore_held_input(&self) {
        for (key, held) in self.held_keys.iter().enumerate() {
            if *held {
                play_raw_event(INPUT_KEY_DOWN, key as i32);
            }
        }
        for (button, held) in self.held_mouse_buttons.iter().enumerate() {
            let event_type = if *held {
                INPUT_MOUSE_BUTTON_DOWN
            } else {
                INPUT_MOUSE_BUTTON_UP
            };
            play_raw_event(event_type, button as i32);
        }
    }

    pub fn step_frame(&mut self) {
        if !self.finished && self.step.is_none() {
            self.step = Some(DebugStep::Frame);
        }
    }

    pub fn step_event(&mut self) {
        if !self.finished && self.step.is_none() {
            self.step = Some(DebugStep::Event);
        }
    }

    pub fn step_mouse_stroke(&mut self) {
        if !self.finished && self.step.is_none() {
            self.step = Some(DebugStep::MouseStroke);
            self.mouse_stroke_started = self.left_mouse_down;
        }
    }

    pub fn toggle_continue(&mut self) {
        if self.finished {
            return;
        }
        if self.step == Some(DebugStep::Continue) {
            self.step = None;
        } else if self.step.is_none() {
            self.step = Some(DebugStep::Continue);
        }
    }

    pub fn step_frame_from_ui(&mut self) {
        self.step_frame();
        self.wait_for_ui_mouse_release();
    }

    pub fn step_event_from_ui(&mut self) {
        self.step_event();
        self.wait_for_ui_mouse_release();
    }

    pub fn step_mouse_stroke_from_ui(&mut self) {
        self.step_mouse_stroke();
        self.wait_for_ui_mouse_release();
    }

    pub fn toggle_continue_from_ui(&mut self) {
        self.toggle_continue();
        self.wait_for_ui_mouse_release();
    }

    fn wait_for_ui_mouse_release(&mut self) {
        if self.step.is_some() {
            self.waiting_for_ui_mouse_release = true;
        }
    }

    pub fn status(&self, state: &State, events: &[AutomationEvent]) -> String {
        if self.finished {
            return format!("Replay finished ({} events)", events.len());
        }
        let mode = match self.step {
            None => "Paused",
            Some(DebugStep::Frame) => "Stepping frame",
            Some(DebugStep::Event) => "Stepping event",
            Some(DebugStep::MouseStroke) => "Stepping mouse stroke",
            Some(DebugStep::Continue) => "Running",
        };
        match events.get(state.current_play_frame) {
            Some(event) => format!(
                "{} | frame {} | event {}/{} | next: frame {}, type {}, {:?}",
                mode,
                state.play_frame_counter,
                state.current_play_frame + 1,
                events.len(),
                event.frame(),
                event.get_type(),
                event.params()
            ),
            None => format!("{} | event {}/{}", mode, events.len(), events.len()),
        }
    }

    fn observe_event(&mut self, event: &AutomationEvent) {
        self.observe_input(event.get_type(), event.params());
    }

    fn observe_input(&mut self, event_type: u32, params: [i32; 4]) {
        let input = params[0];
        match event_type {
            INPUT_KEY_UP | INPUT_KEY_DOWN if input >= 0 => {
                if let Some(held) = self.held_keys.get_mut(input as usize) {
                    *held = event_type == INPUT_KEY_DOWN;
                }
            }
            INPUT_MOUSE_BUTTON_UP | INPUT_MOUSE_BUTTON_DOWN if input >= 0 => {
                if let Some(held) = self.held_mouse_buttons.get_mut(input as usize) {
                    *held = event_type == INPUT_MOUSE_BUTTON_DOWN;
                }
                if input == MOUSE_BUTTON_LEFT {
                    self.left_mouse_down = event_type == INPUT_MOUSE_BUTTON_DOWN;
                }
            }
            INPUT_MOUSE_POSITION => {
                self.mouse_position = (input as f32, params[1] as f32);
            }
            _ => {}
        }
    }
}

fn play_raw_event(event_type: u32, input: i32) {
    let event = ffi::AutomationEvent {
        frame: 0,
        type_: event_type,
        params: [input, 0, 0, 0],
    };
    unsafe { ffi::PlayAutomationEvent(event) };
}

/// Schedule debugger events after rendering. They are consumed by exactly one application update
/// on the next window frame. `did_simulate` distinguishes that update from paused render-only frames.
pub fn debug_replay_after_frame(
    debugger: &mut ReplayDebugger,
    did_simulate: bool,
    ui_mouse_button_down: bool,
    state: &mut State,
    test_options: &Option<TestSettings>,
    events: &[AutomationEvent],
) -> bool {
    if did_simulate {
        debugger.simulation_needed = false;
        if debugger.pause_after_simulation {
            debugger.pause_after_simulation = false;
            debugger.step = None;
        }
        if debugger.pending_finish {
            debugger.pending_finish = false;
            debugger.finished = true;
            if finish_replay(state, test_options) {
                return true;
            }
        }
    }

    if debugger.finished || debugger.simulation_needed {
        return false;
    }

    if debugger.waiting_for_ui_mouse_release {
        if !ui_mouse_button_down {
            // Do not schedule on the release frame. Waiting for the next render-only frame keeps
            // the UI click out of Raylib's input state when replay simulation resumes.
            debugger.waiting_for_ui_mouse_release = false;
        }
        return false;
    }

    let Some(step) = debugger.step else {
        return false;
    };

    match step {
        DebugStep::Frame => {
            while state.current_play_frame < events.len()
                && events[state.current_play_frame].frame() as usize == state.play_frame_counter
            {
                let event = &events[state.current_play_frame];
                debugger.observe_event(event);
                play_event(state, event);
            }
            state.play_frame_counter += 1;
            debugger.pause_after_simulation = true;
        }
        DebugStep::Event => {
            if let Some(event) = events.get(state.current_play_frame) {
                let event_frame = event.frame() as usize;
                state.play_frame_counter = event_frame;
                debugger.observe_event(event);
                play_event(state, event);
                if events
                    .get(state.current_play_frame)
                    .is_none_or(|next| next.frame() as usize > event_frame)
                {
                    state.play_frame_counter += 1;
                }
                debugger.pause_after_simulation = true;
            }
        }
        DebugStep::MouseStroke | DebugStep::Continue => {
            while state.current_play_frame < events.len()
                && events[state.current_play_frame].frame() as usize == state.play_frame_counter
            {
                let event = &events[state.current_play_frame];
                if step == DebugStep::MouseStroke
                    && event.get_type() == INPUT_MOUSE_BUTTON_DOWN
                    && event.params()[0] == MOUSE_BUTTON_LEFT
                {
                    debugger.mouse_stroke_started = true;
                }
                let stroke_ended = step == DebugStep::MouseStroke
                    && debugger.mouse_stroke_started
                    && event.get_type() == INPUT_MOUSE_BUTTON_UP
                    && event.params()[0] == MOUSE_BUTTON_LEFT;
                debugger.observe_event(event);
                play_event(state, event);
                if stroke_ended {
                    debugger.pause_after_simulation = true;
                }
            }
            state.play_frame_counter += 1;
        }
    }

    // Even an empty recorded frame needs one simulation update: held inputs are meaningful.
    debugger.simulation_needed = true;
    if state.current_play_frame == events.len() {
        debugger.pending_finish = true;
        debugger.pause_after_simulation = true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        debug_replay_after_frame, ReplayDebugger, INPUT_KEY_DOWN, INPUT_KEY_UP,
        INPUT_MOUSE_BUTTON_DOWN, INPUT_MOUSE_BUTTON_UP, INPUT_MOUSE_POSITION,
    };
    use crate::state::State;

    #[test]
    fn ui_steps_wait_for_mouse_release_and_a_clear_frame() {
        let mut debugger = ReplayDebugger::new();
        let mut state = State::default();
        let events = vec![];

        debugger.step_frame_from_ui();
        debug_replay_after_frame(&mut debugger, false, true, &mut state, &None, &events);
        assert!(!debugger.should_simulate());

        debug_replay_after_frame(&mut debugger, false, false, &mut state, &None, &events);
        assert!(!debugger.should_simulate());

        debug_replay_after_frame(&mut debugger, false, false, &mut state, &None, &events);
        assert!(debugger.should_simulate());
    }

    #[test]
    fn recorded_input_state_survives_paused_render_frames() {
        let mut debugger = ReplayDebugger::new();

        debugger.observe_input(INPUT_MOUSE_POSITION, [123, 456, 0, 0]);
        debugger.observe_input(INPUT_MOUSE_BUTTON_DOWN, [0, 0, 0, 0]);
        debugger.observe_input(INPUT_KEY_DOWN, [65, 0, 0, 0]);

        assert_eq!(debugger.mouse_position(), (123.0, 456.0));
        assert!(debugger.left_mouse_down);
        assert!(debugger.held_mouse_buttons[0]);
        assert!(debugger.held_keys[65]);

        debugger.observe_input(INPUT_MOUSE_BUTTON_UP, [0, 0, 0, 0]);
        debugger.observe_input(INPUT_KEY_UP, [65, 0, 0, 0]);

        assert!(!debugger.left_mouse_down);
        assert!(!debugger.held_mouse_buttons[0]);
        assert!(!debugger.held_keys[65]);
    }

    #[test]
    fn keyboard_steps_schedule_without_ui_mouse_delay() {
        let mut debugger = ReplayDebugger::new();
        let mut state = State::default();
        let events = vec![];

        debugger.step_frame();
        debug_replay_after_frame(&mut debugger, false, false, &mut state, &None, &events);

        assert!(debugger.should_simulate());
    }
}
