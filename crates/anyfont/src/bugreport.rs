//! Help ▸ Report a Bug... / Submit Ideas and Feedback... (owned by the services UI developer). STUB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Bug, Idea }

pub struct BugReport;

impl BugReport {
    pub fn new() -> Self { BugReport }
    pub fn open(&mut self, _kind: Kind) {}
    /// `crash_report`: text of a crash report found at start-up, offered as the description.
    pub fn open_with_crash(&mut self, _crash_report: String) {}
    pub fn ui(&mut self, _ctx: &egui::Context) {}
}
