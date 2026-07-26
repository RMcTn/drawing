use std::{
    io::{self, Write},
    path::Path,
};

use log::{debug, error, info};
use raylib::{
    automation::{AutomationEvent, AutomationEventList},
    RaylibHandle,
};

use crate::{app::TestSettings, persistence, state::State, test_snapshot};

/// App-specific automation event. `params[0]` contains a Unicode code point.
/// Raylib recordings only capture key states and cannot reproduce text character input.
const INPUT_TEXT_CODEPOINT: u32 = 24;
const INPUT_MOUSE_BUTTON_UP: u32 = 5;
const INPUT_MOUSE_BUTTON_DOWN: u32 = 6;
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
            info!("Attempting to save since replay has finished");
            match persistence::save(state, save_path) {
                Ok(_) => info!("Successfully saved to {}", save_path.display()),
                Err(e) => error!("Failed to save to {}: {}", save_path.display(), e),
            }
        }

        if let Some(snapshot_path) = &test_options.snapshot_path {
            info!(
                "Writing replay state snapshot to {}",
                snapshot_path.display()
            );
            if let Err(e) = test_snapshot::save(state, snapshot_path) {
                error!(
                    "Failed to write replay state snapshot to {}: {}",
                    snapshot_path.display(),
                    e
                );
            }
        }

        if test_options.quit_after_replay {
            info!("Quitting - Quit after replay is enabled");
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

#[derive(Debug, Clone, Copy)]
enum DebugStep {
    Frame,
    Event,
    MouseStroke,
    Continue,
}

/// Terminal-driven replay debugger. The terminal blocks between steps, so the drawing window can
/// display the completed step without repeatedly processing a held automation input.
pub struct ReplayDebugger {
    step: Option<DebugStep>,
    waiting_for_step_result: bool,
    mouse_stroke_started: bool,
    left_mouse_down: bool,
    pending_finish: bool,
    finished: bool,
}

impl ReplayDebugger {
    pub fn new() -> Self {
        Self {
            step: None,
            waiting_for_step_result: false,
            mouse_stroke_started: false,
            left_mouse_down: false,
            pending_finish: false,
            finished: false,
        }
    }

    fn prompt(&mut self, state: &State, events: &[AutomationEvent]) -> bool {
        let next = events.get(state.current_play_frame);
        if self.finished {
            println!("\nReplay finished: {} events", events.len());
        } else if let Some(event) = next {
            println!(
                "\nReplay paused: frame {}, event {}/{} (recorded frame {}, type {}, params {:?})",
                state.play_frame_counter,
                state.current_play_frame + 1,
                events.len(),
                event.frame(),
                event.get_type(),
                event.params(),
            );
        }
        print!("[f]rame  [e]vent  mouse [s]troke  [c]ontinue  [q]uit > ");
        let _ = io::stdout().flush();

        loop {
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                return true;
            }
            match input.trim().to_ascii_lowercase().as_str() {
                "f" | "frame" if !self.finished => {
                    self.step = Some(DebugStep::Frame);
                    return false;
                }
                "e" | "event" if !self.finished => {
                    self.step = Some(DebugStep::Event);
                    return false;
                }
                "s" | "stroke" if !self.finished => {
                    self.step = Some(DebugStep::MouseStroke);
                    self.mouse_stroke_started = self.left_mouse_down;
                    return false;
                }
                "c" | "continue" if !self.finished => {
                    self.step = Some(DebugStep::Continue);
                    return false;
                }
                "q" | "quit" => return true,
                _ => {
                    print!("Enter f, e, s, c, or q > ");
                    let _ = io::stdout().flush();
                }
            }
        }
    }

    fn observe_event(&mut self, event: &AutomationEvent) {
        if event.params()[0] == MOUSE_BUTTON_LEFT {
            if event.get_type() == INPUT_MOUSE_BUTTON_DOWN {
                self.left_mouse_down = true;
            } else if event.get_type() == INPUT_MOUSE_BUTTON_UP {
                self.left_mouse_down = false;
            }
        }
    }
}

/// Called after an application frame has been rendered. Events scheduled here are consumed by the
/// next application frame; when a step completes, that resulting frame is rendered before the next
/// terminal prompt appears.
pub fn debug_replay_after_frame(
    debugger: &mut ReplayDebugger,
    state: &mut State,
    test_options: &Option<TestSettings>,
    events: &[AutomationEvent],
) -> bool {
    if debugger.waiting_for_step_result {
        debugger.waiting_for_step_result = false;
        debugger.step = None;
        if debugger.pending_finish {
            debugger.pending_finish = false;
            debugger.finished = true;
            if finish_replay(state, test_options) {
                return true;
            }
        }
    }

    if debugger.finished {
        return debugger.prompt(state, events);
    }

    if debugger.step.is_none() && debugger.prompt(state, events) {
        return true;
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
            debugger.waiting_for_step_result = true;
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
                debugger.waiting_for_step_result = true;
            }
        }
        DebugStep::MouseStroke | DebugStep::Continue => {
            while state.current_play_frame < events.len()
                && events[state.current_play_frame].frame() as usize == state.play_frame_counter
            {
                let event = &events[state.current_play_frame];
                if step_matches_mouse_stroke(step)
                    && event.get_type() == INPUT_MOUSE_BUTTON_DOWN
                    && event.params()[0] == MOUSE_BUTTON_LEFT
                {
                    debugger.mouse_stroke_started = true;
                }
                let stroke_ended = step_matches_mouse_stroke(step)
                    && debugger.mouse_stroke_started
                    && event.get_type() == INPUT_MOUSE_BUTTON_UP
                    && event.params()[0] == MOUSE_BUTTON_LEFT;
                debugger.observe_event(event);
                play_event(state, event);
                if stroke_ended {
                    debugger.waiting_for_step_result = true;
                }
            }
            state.play_frame_counter += 1;
        }
    }

    if state.current_play_frame == events.len() {
        // Wait one rendered application frame so the last scheduled event is observed before save,
        // snapshot, or the final debugger prompt.
        debugger.pending_finish = true;
        debugger.waiting_for_step_result = true;
    }

    false
}

fn step_matches_mouse_stroke(step: DebugStep) -> bool {
    matches!(step, DebugStep::MouseStroke)
}
