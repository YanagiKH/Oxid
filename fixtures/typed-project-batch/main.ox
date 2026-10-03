mod state;
mod jobs;
use crate::state::make;
use crate::state::retry;
use crate::state::done;
use crate::state::next_job;
use crate::state::is_active;
use crate::state::stop;
use crate::state::relay;
use crate::state::finish;
use crate::jobs::dispatch;
fn main() -> i32 {
    let mut state = make();
    while is_active(&state) {
        let mut attempt = 0;
        while attempt < 3 {
            attempt = attempt + 1;
            if attempt < 2 {
                retry(&mut state);
                continue;
            }
            let job = next_job(&state);
            dispatch(&mut state, job);
            break;
        }
        if done(&state) {
            stop(&mut state);
            break;
        }
        continue;
    }
    let completed = relay(state);
    return finish(completed);
}
