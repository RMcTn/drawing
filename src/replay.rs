use std::path::Path;

use log::{debug, error, info};
use raylib::{
    automation::{AutomationEvent, AutomationEventList},
    RaylibHandle,
};

use crate::{app::TestSettings, persistence, state::State, test_snapshot};

/// App-specific automation event. `params[0]` contains a Unicode code point.
/// Raylib recordings only capture key states and cannot reproduce text character input.
const INPUT_TEXT_CODEPOINT: u32 = 24;

pub fn load_replay(
    replay_path: &Path,
    rl: &RaylibHandle,
    automation_events_list: &mut AutomationEventList,
    automation_events: &mut Vec<AutomationEvent>,
) -> Option<()> {
    debug!("Trying to load replay from {:?}", replay_path);
    let loaded_automated_events = rl.load_automation_event_list(Some(replay_path.into()));
    if loaded_automated_events.count() == 0 {
        // Load unsuccessful
        // TODO: Show failure on UI
        error!(
            "Couldn't load automated event list from {}, or it was empty",
            replay_path.display()
        );
        return None;
    } else {
        // TODO: Does this leak memory?
        *automation_events_list = loaded_automated_events;
        rl.set_automation_event_list(automation_events_list);
        rl.set_automation_event_base_frame(0);

        *automation_events = automation_events_list.events();

        // TODO: Show success on UI
        info!(
            "Successfully loaded automated event list from {}",
            replay_path.display(),
        );
        return Some(());
    }
}

pub fn play_replay(state: &mut State) {
    state.is_playing_inputs = true;
    // TODO: Reset camera state etc
    state.current_play_frame = 0;
    state.play_frame_counter = 0;
}

pub fn stop_replay(state: &mut State) {
    state.is_playing_inputs = false;
    state.current_play_frame = 0;
    state.play_frame_counter = 0;
}

/// Returns true if the program should exit
pub fn replay_inputs(
    state: &mut State,
    test_options: &Option<TestSettings>,
    automation_events: &Vec<AutomationEvent>,
) -> bool {
    // NOTE: Multiple events could be executed in a single frame
    while state.play_frame_counter == automation_events[state.current_play_frame].frame() as usize {
        let event = &automation_events[state.current_play_frame];
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

        if state.current_play_frame == automation_events.len() {
            stop_replay(state);
            info!("Finished playing replay");
            if let Some(ref test_options) = test_options {
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
            break;
        }
    }
    state.play_frame_counter += 1;
    return false;
}
