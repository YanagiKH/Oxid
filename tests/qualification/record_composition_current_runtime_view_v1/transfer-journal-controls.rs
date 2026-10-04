#[cfg(test)]
pub(super) fn record_transfer_control_start() {
    JOURNAL.with_borrow_mut(|j| *j = Journal { enabled: true, ..Journal::default() });
}
#[cfg(test)]
pub(super) fn record_transfer_control_rows() -> Vec<(&'static str, String)> {
    JOURNAL.with_borrow(|j| j.rows.iter().map(|row| (row.kind, row.payload.clone())).collect())
}
#[cfg(test)]
pub(super) fn record_transfer_control_end() {
    JOURNAL.with_borrow_mut(|j| *j = Journal::default());
}
