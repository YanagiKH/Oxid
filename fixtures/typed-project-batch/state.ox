pub struct Batch { completed: i32, retries: i32, checksum: i32, active: bool }
pub fn make() -> Batch {
    return Batch { completed: 0, retries: 0, checksum: 0, active: true };
}
pub fn retry(state: &mut Batch) -> () {
    state.retries = state.retries + 1;
    return;
}
pub fn commit(state: &mut Batch, job: i32) -> () {
    state.completed = state.completed + 1;
    state.checksum = state.checksum + job * 10;
    return;
}
pub fn done(state: &Batch) -> bool { return state.completed >= 6; }
pub fn next_job(state: &Batch) -> i32 { return state.completed + 1; }
pub fn is_active(state: &Batch) -> bool { return state.active; }
pub fn stop(state: &mut Batch) -> () { state.active = false; return; }
pub fn relay(state: Batch) -> Batch { return state; }
pub fn finish(state: Batch) -> i32 {
    return state.checksum + state.completed * 100 + state.retries;
}
