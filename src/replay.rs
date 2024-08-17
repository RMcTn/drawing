use std::path::Path;

use log::{debug, error, info};
use raylib::{
    automation::{AutomationEvent, AutomationEventList},
    RaylibHandle,
};

use crate::state::State;

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
