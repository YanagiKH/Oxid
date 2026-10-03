use crate::state::Batch;
use crate::state::commit;
pub fn dispatch(state: &mut Batch, job: i32) -> () {
    commit(&mut *state, job);
    return;
}
