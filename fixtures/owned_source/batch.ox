struct Batch { completed: i32, retries: i32, checksum: i32, active: bool }
fn retry(state: &mut Batch) -> () {
    state.retries = state.retries + 1;
    return;
}
fn commit(state: &mut Batch, job: i32) -> () {
    state.completed = state.completed + 1;
    state.checksum = state.checksum + job * 10;
    return;
}
fn dispatch(state: &mut Batch, job: i32) -> () {
    commit(&mut *state, job);
    return;
}
fn done(state: &Batch) -> bool { return state.completed >= 6; }
fn relay(state: Batch) -> Batch { return state; }
fn finish(state: Batch) -> i32 {
    return state.checksum + state.completed * 100 + state.retries;
}
fn main() -> i32 {
    let mut state = Batch { completed: 0, retries: 0, checksum: 0, active: true };
    while state.active {
        let mut attempt = 0;
        while attempt < 3 {
            attempt = attempt + 1;
            if attempt < 2 {
                retry(&mut state);
                continue;
            }
            let job = state.completed + 1;
            dispatch(&mut state, job);
            break;
        }
        if done(&state) {
            state.active = false;
            break;
        }
        continue;
    }
    let completed = relay(state);
    return finish(completed);
}
