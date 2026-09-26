//! Shared test fixture: a [`JobCanceller`] that records every id it was asked
//! to cancel, instead of touching a real job tracker. Used by every topic
//! test file that exercises `cancel`/`cancel_all`.

use std::cell::RefCell;

use super::super::JobCanceller;

#[derive(Default)]
pub(super) struct RecordingCanceller {
    pub(super) cancelled: RefCell<Vec<String>>,
}

impl JobCanceller for RecordingCanceller {
    fn cancel_job(&self, job_id: &str) {
        self.cancelled.borrow_mut().push(job_id.to_string());
    }
}
