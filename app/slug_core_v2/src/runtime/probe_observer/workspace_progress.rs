//! Fixed test-only timing at the actual native workspace dispatch boundaries.
use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Preflight,
    ReserveAndInitialize,
    SourceSnapshot,
    Archive,
    Execute,
    Filesystem,
}

impl Stage {
    pub(crate) fn operation(operation: &RepositoryWorkspaceOperation) -> Self {
        match operation {
            RepositoryWorkspaceOperation::Archive(_) => Self::Archive,
            RepositoryWorkspaceOperation::Execute(_) => Self::Execute,
            RepositoryWorkspaceOperation::File { .. }
            | RepositoryWorkspaceOperation::Template { .. }
            | RepositoryWorkspaceOperation::Symlink { .. }
            | RepositoryWorkspaceOperation::Read(_)
            | RepositoryWorkspaceOperation::Exists(_)
            | RepositoryWorkspaceOperation::Delete(_) => Self::Filesystem,
        }
    }
}

const LABELS: [&str; 6] = [
    "preflight",
    "reserve_and_initialize",
    "source_snapshot",
    "archive",
    "execute",
    "filesystem",
];

#[derive(Default)]
pub(super) struct Totals {
    // Two independent outcomes per stage. Column pairs are calls and wall ns.
    rows: [[u64; 4]; LABELS.len()],
    invalid: bool,
}

impl Totals {
    fn record(&mut self, stage: Stage, completed: bool, wall_ns: Option<u64>) {
        let row = &mut self.rows[stage as usize];
        let column = if completed { 0 } else { 2 };
        let sum = wall_ns.and_then(|ns| row[column + 1].checked_add(ns));
        match row[column].checked_add(1).zip(sum) {
            Some((calls, wall_ns)) => {
                row[column] = calls;
                row[column + 1] = wall_ns;
            }
            None => self.invalid = true,
        }
    }

    pub(super) fn report(&self) -> Value {
        json!({
            "stages": LABELS,
            "columns": ["completed", "completed_wall_ns", "aborted", "aborted_wall_ns"],
            "rows": self.rows,
            "invalid_clock_or_overflow": self.invalid,
            "completed_includes_replay": true,
            "overlaps_outer_progress_wall": true,
        })
    }
}

/// Holds a sampler reference only. The mutex is acquired after work returns,
/// including early returns, and never spans native IO or child execution.
#[must_use]
pub(crate) struct Span<'a> {
    totals: &'a Mutex<Totals>,
    stage: Stage,
    start: Instant,
    completed: bool,
}

impl<'a> Span<'a> {
    pub(super) fn start(totals: &'a Mutex<Totals>, stage: Stage) -> Self {
        Self {
            totals,
            stage,
            start: Instant::now(),
            completed: false,
        }
    }

    pub(crate) fn finish(mut self) {
        self.completed = true;
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        let wall_ns = Instant::now()
            .checked_duration_since(self.start)
            .and_then(|duration| u64::try_from(duration.as_nanos()).ok());
        self.totals
            .lock()
            .expect("workspace progress sampler poisoned")
            .record(self.stage, self.completed, wall_ns);
    }
}

#[test]
fn workspace_progress_spans_distinguish_completion_early_drop_and_invalid_totals() {
    let totals = Mutex::new(Totals::default());
    Span::start(&totals, Stage::Archive).finish();
    drop(Span::start(&totals, Stage::Execute));
    let mut totals = totals.into_inner().unwrap();
    assert_eq!(totals.rows[Stage::Archive as usize][0], 1);
    assert_eq!(totals.rows[Stage::Archive as usize][2], 0);
    assert_eq!(totals.rows[Stage::Execute as usize][0], 0);
    assert_eq!(totals.rows[Stage::Execute as usize][2], 1);

    let before = totals.rows;
    totals.record(Stage::Preflight, true, None);
    assert!(totals.invalid);
    assert_eq!(totals.rows, before);
    totals.invalid = false;
    totals.rows[Stage::Preflight as usize][1] = u64::MAX;
    let before = totals.rows;
    totals.record(Stage::Preflight, true, Some(1));
    assert!(totals.invalid);
    assert_eq!(totals.rows, before);
}

#[test]
fn workspace_progress_report_is_bounded_at_maximum_counts() {
    let totals = Totals {
        rows: [[u64::MAX; 4]; LABELS.len()],
        invalid: true,
    };
    assert!(
        json!({"workspace_progress": totals.report()})
            .to_string()
            .len()
            < 1024
    );
}
